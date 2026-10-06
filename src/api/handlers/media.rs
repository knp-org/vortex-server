use axum::{
    extract::{Path, State},
    Extension, Json,
};
use sqlx::SqlitePool;
use serde_json::json;
use crate::error::AppError;
use crate::api::middleware::AuthUser;
use crate::models::metadata::MetadataPatch;
use crate::services::{media_service, catalog_service::CatalogService};
use crate::services::library_service::LibraryService;
use crate::api::dtos::responses::{Card, AlbumDetail, ArtistDetail};
use crate::api::dtos::requests::{SearchQuery, IdentifyRequest};

#[derive(serde::Deserialize)]
pub struct LibraryScopedQuery {
    pub library_id: Option<i64>,
}

pub async fn get_library_media(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<Card>>, AppError> {
    let library = LibraryService::new(pool.clone()).get_by_id(id).await?;
    let cards = media_service::MediaService::new(pool.clone()).list_library(id, &library.library_type).await?;
    Ok(Json(cards))
}

pub async fn get_recently_added(State(pool): State<SqlitePool>) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).recently_added().await?))
}

/// Resolve the next locally available episode after an episode item.
pub async fn get_next_episode(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Option<crate::api::dtos::responses::EpisodeDto>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool).next_episode(id).await?))
}

#[derive(serde::Deserialize)]
pub struct LyricsQuery {
    pub force: Option<bool>,
}

/// Lyrics for a track: sidecar `.lrc`/`.txt`, embedded tags, then lrclib.net.
/// Always 200 with a (possibly empty) result so the client can show a clean
/// "no lyrics" state rather than handling a 404.
pub async fn get_track_lyrics(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    axum::extract::Query(query): axum::extract::Query<LyricsQuery>,
) -> Result<Json<crate::services::lyrics_service::Lyrics>, AppError> {
    Ok(Json(crate::services::lyrics_service::LyricsService::new(pool).for_track(id, query.force.unwrap_or(false)).await?))
}

/// Detail view, dispatched by the item's type.
pub async fn get_media_details(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let item_type = media_service::MediaService::new(pool.clone()).item_type(id).await?;

    let mut value = match item_type.as_str() {
        "book" => json!(media_service::MediaService::new(pool.clone()).book_detail(id).await?),
        "music_video" => json!(media_service::MediaService::new(pool.clone()).music_video_detail(id).await?),
        "episode" => json!(media_service::MediaService::new(pool.clone()).episode_detail(id).await?),
        "image" => json!(media_service::MediaService::new(pool.clone()).image_detail(id).await?),
        "track" => {
            // A track's "detail" is its album (so the client can show the track list).
            match media_service::MediaService::new(pool.clone()).track_album_id(id).await? {
                Some(aid) => json!(media_service::MediaService::new(pool.clone()).album_detail(aid).await?),
                None => json!({ "id": id }),
            }
        }
        _ => json!(media_service::MediaService::new(pool.clone()).movie_detail(id).await?),
    };
    // Add a type discriminator so the client knows which detail shape it received.
    if let Some(obj) = value.as_object_mut() {
        obj.insert("kind".to_string(), serde_json::Value::String(item_type.clone()));
    }
    Ok(Json(value))
}

/// Refresh a movie's metadata from the configured provider chain.
pub async fn refresh_media_metadata(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::services::metadata_service::MetadataService;

    // A hand-edited item is pinned; refreshing it would silently discard the edit.
    if CatalogService::new(pool.clone()).movie_locked(id).await? {
        return Err(AppError::BadRequest(
            "This item's metadata is locked by a manual edit. Unlock it (or re-identify) to refresh.".into(),
        ));
    }

    let svc = MetadataService::new(pool.clone());
    let (title, provider_ids) = media_service::MediaService::new(pool.clone()).movie_provider_lookup(id).await?;

    // Resolve a provider id from the stored provider_ids JSON, if any.
    let mut provider_name = svc.get_default_provider().await;
    let mut provider_id = None;
    if let Some(json_str) = provider_ids.as_ref() {
        if let Ok(ids) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(json_str) {
            if let Some(v) = ids.get(&provider_name) {
                provider_id = v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(|s| s.to_string()));
            } else if let Some((p, v)) = ids.iter().next() {
                provider_name = p.clone();
                provider_id = v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(|s| s.to_string()));
            }
        }
    }

    let meta = if let Some(pid) = provider_id {
        svc.fetch_by_id(&pid, Some("movie"), Some(&provider_name)).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata by ID: {}", e)))?
    } else {
        let term = title.unwrap_or_else(|| "".to_string());
        svc.fetch_metadata(&term, Some("movie")).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?
    };

    CatalogService::new(pool.clone()).apply_movie_metadata(id, &meta).await?;
    get_media_details(State(pool), Path(id)).await
}

/// Save a hand-authored metadata edit, dispatched by the item's type.
///
/// The body is a sparse patch: absent fields are left alone, `null` clears a field.
/// Saving pins the item (`metadata_locked`) unless the caller says otherwise, so a
/// later refresh or rescan can't overwrite the edit.
pub async fn update_media_metadata(
    Extension(auth_user): Extension<AuthUser>,
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(mut patch): Json<MetadataPatch>,
) -> Result<Json<serde_json::Value>, AppError> {
    auth_user.require_admin()?;
    if patch.is_empty() {
        return Err(AppError::BadRequest("No metadata fields to update".into()));
    }
    patch.metadata_locked.get_or_insert(true);

    let catalog = CatalogService::new(pool.clone());
    let item_type = media_service::MediaService::new(pool.clone()).item_type(id).await?;
    match item_type.as_str() {
        "book" => catalog.patch_book(id, &patch).await?,
        "episode" => catalog.patch_episode(id, &patch).await?,
        "music_video" => catalog.patch_music_video(id, &patch).await?,
        "image" => catalog.patch_image(id, &patch).await?,
        "track" => return Err(AppError::BadRequest(
            "Track metadata is read from the file's tags and can't be edited here".into(),
        )),
        _ => catalog.patch_movie(id, &patch).await?,
    }
    get_media_details(State(pool), Path(id)).await
}

pub async fn identify_media(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::services::metadata_service::MetadataService;

    // Identifying is an explicit "use this entry instead" — it overrides a lock.
    CatalogService::new(pool.clone()).unlock_movie(id).await?;

    let media_type = payload.media_type.as_deref().or(Some("movie"));
    let meta = MetadataService::new(pool.clone())
        .fetch_by_id(&payload.provider_id, media_type, payload.provider_name.as_deref()).await?;
    CatalogService::new(pool.clone()).apply_movie_metadata(id, &meta).await?;
    get_media_details(State(pool), Path(id)).await
}

pub async fn search_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<crate::models::metadata::NormalizedMetadata>>, AppError> {
    use crate::services::metadata_service::MetadataService;

    let svc = MetadataService::new(pool.clone());
    let media_type = params.media_type.as_deref();
    if let Ok(id) = params.query.trim().parse::<i64>() {
        let meta = svc.fetch_by_id(&id.to_string(), media_type, None).await?;
        Ok(Json(vec![meta]))
    } else {
        let results = svc.search(&params.query, params.year.clone(), media_type).await?;
        Ok(Json(results))
    }
}

pub async fn search_library(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).search(&params.query).await?))
}

// ── Music browse ───────────────────────────────────────────────────────────

pub async fn get_artists(
    State(pool): State<SqlitePool>,
    axum::extract::Query(q): axum::extract::Query<LibraryScopedQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).artist_cards(q.library_id).await?))
}

pub async fn get_library_tracks(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<crate::api::dtos::responses::TrackDto>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).library_tracks(id).await?))
}

pub async fn get_artist_detail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<ArtistDetail>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).artist_detail(id).await?))
}

pub async fn get_album_detail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<AlbumDetail>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).album_detail(id).await?))
}

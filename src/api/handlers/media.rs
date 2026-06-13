use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use serde_json::json;
use crate::error::AppError;
use crate::services::{media_service, catalog};
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
    let cards = media_service::list_library(&pool, id, &library.library_type).await?;
    Ok(Json(cards))
}

pub async fn get_recently_added(State(pool): State<SqlitePool>) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::recently_added(&pool).await?))
}

/// Detail view, dispatched by the item's type.
pub async fn get_media_details(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let item_type: Option<(String,)> = sqlx::query_as("SELECT item_type FROM media_items WHERE id = ?")
        .bind(id).fetch_optional(&pool).await?;
    let item_type = item_type
        .ok_or_else(|| AppError::NotFound(format!("Media {} not found", id)))?.0;

    let mut value = match item_type.as_str() {
        "book" => json!(media_service::book_detail(&pool, id).await?),
        "episode" => json!(media_service::episode_detail(&pool, id).await?),
        "track" => {
            // A track's "detail" is its album (so the client can show the track list).
            let album_id: Option<(Option<i64>,)> = sqlx::query_as("SELECT album_id FROM tracks WHERE item_id = ?")
                .bind(id).fetch_optional(&pool).await?;
            match album_id.and_then(|r| r.0) {
                Some(aid) => json!(media_service::album_detail(&pool, aid).await?),
                None => json!({ "id": id }),
            }
        }
        _ => json!(media_service::movie_detail(&pool, id).await?),
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
    use crate::services::metadata::{fetch_metadata, fetch_by_id, get_default_provider};

    let (title, provider_ids) = media_service::movie_provider_lookup(&pool, id).await?;

    // Resolve a provider id from the stored provider_ids JSON, if any.
    let mut provider_name = get_default_provider(&pool).await;
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
        fetch_by_id(&pid, Some("movie"), &pool, Some(&provider_name)).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata by ID: {}", e)))?
    } else {
        let term = title.unwrap_or_else(|| "".to_string());
        fetch_metadata(&term, Some("movie"), &pool).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?
    };

    catalog::apply_movie_metadata(&pool, id, &meta).await?;
    get_media_details(State(pool), Path(id)).await
}

pub async fn identify_media(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::services::metadata::fetch_by_id;

    let media_type = payload.media_type.as_deref().or(Some("movie"));
    let meta = fetch_by_id(&payload.provider_id, media_type, &pool, payload.provider_name.as_deref()).await?;
    catalog::apply_movie_metadata(&pool, id, &meta).await?;
    get_media_details(State(pool), Path(id)).await
}

pub async fn search_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<crate::models::metadata::NormalizedMetadata>>, AppError> {
    use crate::services::metadata::{search, fetch_by_id};

    let media_type = params.media_type.as_deref();
    if let Ok(id) = params.query.trim().parse::<i64>() {
        let meta = fetch_by_id(&id.to_string(), media_type, &pool, None).await?;
        Ok(Json(vec![meta]))
    } else {
        let results = search(&params.query, params.year.clone(), media_type, &pool).await?;
        Ok(Json(results))
    }
}

pub async fn search_library(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::search(&pool, &params.query).await?))
}

// ── Music browse ───────────────────────────────────────────────────────────

pub async fn get_artists(
    State(pool): State<SqlitePool>,
    axum::extract::Query(q): axum::extract::Query<LibraryScopedQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::artist_cards(&pool, q.library_id).await?))
}

pub async fn get_artist_detail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<ArtistDetail>, AppError> {
    Ok(Json(media_service::artist_detail(&pool, id).await?))
}

pub async fn get_album_detail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<AlbumDetail>, AppError> {
    Ok(Json(media_service::album_detail(&pool, id).await?))
}

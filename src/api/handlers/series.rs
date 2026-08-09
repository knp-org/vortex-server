use axum::{
    extract::{Path, State},
    Extension, Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::services::{media_service, catalog_service::CatalogService};
use crate::api::dtos::requests::IdentifyRequest;
use crate::api::middleware::AuthUser;
use crate::models::metadata::MetadataPatch;
use crate::api::dtos::responses::{Card, SeasonDto, SeriesDetail, EpisodeDto};

#[derive(serde::Deserialize)]
pub struct SeriesQuery {
    /// When present, only series in this library are returned.
    pub library_id: Option<i64>,
}

pub async fn get_all_series(
    State(pool): State<SqlitePool>,
    axum::extract::Query(query): axum::extract::Query<SeriesQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).series_cards(query.library_id).await?))
}

pub async fn get_series_seasons(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<SeasonDto>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).series_seasons(series_id).await?))
}

pub async fn get_season_episodes(
    Path((series_id, season_number)): Path<(i64, i64)>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<EpisodeDto>>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).season_episodes(series_id, season_number).await?))
}

pub async fn get_series_detail(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetail>, AppError> {
    Ok(Json(media_service::MediaService::new(pool.clone()).series_detail(series_id).await?))
}

/// Resolve a provider id from a series' stored `provider_ids` JSON.
fn resolve_provider_id(provider_ids: Option<&String>, provider_name: &str) -> (String, Option<String>) {
    let mut name = provider_name.to_string();
    let mut id = None;
    if let Some(json_str) = provider_ids {
        if let Ok(ids) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(json_str) {
            if let Some(v) = ids.get(&name) {
                id = v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(|s| s.to_string()));
            } else if let Some((p, v)) = ids.iter().next() {
                name = p.clone();
                id = v.as_i64().map(|i| i.to_string()).or_else(|| v.as_str().map(|s| s.to_string()));
            }
        }
    }
    (name, id)
}

/// Fill per-episode titles/plots/stills for every scanned season of a series.
async fn refresh_episode_details(pool: &SqlitePool, series_id: i64, provider_id: &str, provider_name: Option<&str>) -> Result<(), AppError> {
    use crate::services::metadata_service::MetadataService;

    let meta = MetadataService::new(pool.clone());
    let seasons = media_service::MediaService::new(pool.clone()).series_seasons(series_id).await?;
    for season in seasons {
        if let Ok(episodes) = meta.fetch_episodes(provider_id, season.season_number as i32, provider_name).await {
            for ep in episodes {
                let item_id = media_service::MediaService::new(pool.clone())
                    .episode_item_id(series_id, season.season_number, ep.episode_number as i64)
                    .await?;

                if let Some(item_id) = item_id {
                    let _ = CatalogService::new(pool.clone()).apply_episode_details(item_id, &ep.name, &ep.overview, ep.still_path.clone()).await;
                }
            }
        }
    }
    Ok(())
}

/// Save a hand-authored edit to a series. Sparse patch semantics: absent fields are
/// left alone, `null` clears one. Saving pins the series against auto-refresh unless
/// the caller explicitly sets `metadata_locked: false`.
pub async fn update_series_metadata(
    Extension(auth_user): Extension<AuthUser>,
    State(pool): State<SqlitePool>,
    Path(series_id): Path<i64>,
    Json(mut patch): Json<MetadataPatch>,
) -> Result<Json<SeriesDetail>, AppError> {
    auth_user.require_admin()?;
    if patch.is_empty() {
        return Err(AppError::BadRequest("No metadata fields to update".into()));
    }
    patch.metadata_locked.get_or_insert(true);

    CatalogService::new(pool.clone()).patch_series(series_id, &patch).await?;
    Ok(Json(media_service::MediaService::new(pool.clone()).series_detail(series_id).await?))
}

pub async fn refresh_series_metadata(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetail>, AppError> {
    use crate::services::metadata_service::MetadataService;

    // A hand-edited series is pinned; refreshing it would silently discard the edit.
    if CatalogService::new(pool.clone()).series_locked(series_id).await? {
        return Err(AppError::BadRequest(
            "This series' metadata is locked by a manual edit. Unlock it (or re-identify) to refresh.".into(),
        ));
    }

    let svc = MetadataService::new(pool.clone());
    let (name, provider_ids) = media_service::MediaService::new(pool.clone()).series_provider_lookup(series_id).await?;

    let (provider_name, provider_id) = resolve_provider_id(provider_ids.as_ref(), &svc.get_default_provider().await);

    let meta = if let Some(pid) = &provider_id {
        svc.fetch_by_id(pid, Some("series"), Some(&provider_name)).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata by ID: {}", e)))?
    } else {
        svc.fetch_metadata(&name, Some("series")).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?
    };

    CatalogService::new(pool.clone()).apply_series_metadata(series_id, &meta).await?;

    let (resolved_name, resolved_id) = resolve_provider_id(
        meta.provider_ids.as_ref().map(|v| v.to_string()).as_ref(),
        &provider_name,
    );
    if let Some(pid) = resolved_id {
        refresh_episode_details(&pool, series_id, &pid, Some(&resolved_name)).await?;
    }

    Ok(Json(media_service::MediaService::new(pool.clone()).series_detail(series_id).await?))
}

pub async fn identify_series(
    State(pool): State<SqlitePool>,
    Path(series_id): Path<i64>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<SeriesDetail>, AppError> {
    use crate::services::metadata_service::MetadataService;

    // Identifying is an explicit "use this show instead" — it overrides a lock.
    CatalogService::new(pool.clone()).unlock_series(series_id).await?;

    let media_type = payload.media_type.as_deref().or(Some("series"));
    let meta = MetadataService::new(pool.clone())
        .fetch_by_id(&payload.provider_id, media_type, payload.provider_name.as_deref()).await
        .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?;

    CatalogService::new(pool.clone()).apply_series_metadata(series_id, &meta).await?;
    refresh_episode_details(&pool, series_id, &payload.provider_id, payload.provider_name.as_deref()).await?;

    Ok(Json(media_service::MediaService::new(pool.clone()).series_detail(series_id).await?))
}

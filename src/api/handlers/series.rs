use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::services::{media_service, catalog};
use crate::api::dtos::requests::IdentifyRequest;
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
    Ok(Json(media_service::series_cards(&pool, query.library_id).await?))
}

pub async fn get_series_seasons(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<SeasonDto>>, AppError> {
    Ok(Json(media_service::series_seasons(&pool, series_id).await?))
}

pub async fn get_season_episodes(
    Path((series_id, season_number)): Path<(i64, i64)>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<EpisodeDto>>, AppError> {
    Ok(Json(media_service::season_episodes(&pool, series_id, season_number).await?))
}

pub async fn get_series_detail(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetail>, AppError> {
    Ok(Json(media_service::series_detail(&pool, series_id).await?))
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
    use crate::services::metadata::fetch_episodes;

    let seasons = media_service::series_seasons(pool, series_id).await?;
    for season in seasons {
        if let Ok(episodes) = fetch_episodes(provider_id, season.season_number as i32, pool, provider_name).await {
            for ep in episodes {
                let item_id: Option<(i64,)> = sqlx::query_as(
                    "SELECT e.item_id FROM episodes e JOIN seasons se ON se.id = e.season_id
                     WHERE se.series_id = ? AND se.season_number = ? AND e.episode_number = ?"
                )
                .bind(series_id).bind(season.season_number).bind(ep.episode_number as i64)
                .fetch_optional(pool).await?;

                if let Some((item_id,)) = item_id {
                    let _ = catalog::apply_episode_details(pool, item_id, &ep.name, &ep.overview, ep.still_path.clone()).await;
                }
            }
        }
    }
    Ok(())
}

pub async fn refresh_series_metadata(
    Path(series_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<SeriesDetail>, AppError> {
    use crate::services::metadata::{fetch_metadata, fetch_by_id, get_default_provider};

    let row: Option<(String, Option<String>)> = sqlx::query_as("SELECT name, provider_ids FROM series WHERE id = ?")
        .bind(series_id).fetch_optional(&pool).await?;
    let (name, provider_ids) = row.ok_or_else(|| AppError::NotFound(format!("Series {} not found", series_id)))?;

    let (provider_name, provider_id) = resolve_provider_id(provider_ids.as_ref(), &get_default_provider(&pool).await);

    let meta = if let Some(pid) = &provider_id {
        fetch_by_id(pid, Some("series"), &pool, Some(&provider_name)).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata by ID: {}", e)))?
    } else {
        fetch_metadata(&name, Some("series"), &pool).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?
    };

    catalog::apply_series_metadata(&pool, series_id, &meta).await?;

    let (resolved_name, resolved_id) = resolve_provider_id(
        meta.provider_ids.as_ref().map(|v| v.to_string()).as_ref(),
        &provider_name,
    );
    if let Some(pid) = resolved_id {
        refresh_episode_details(&pool, series_id, &pid, Some(&resolved_name)).await?;
    }

    Ok(Json(media_service::series_detail(&pool, series_id).await?))
}

pub async fn identify_series(
    State(pool): State<SqlitePool>,
    Path(series_id): Path<i64>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<SeriesDetail>, AppError> {
    use crate::services::metadata::fetch_by_id;

    let media_type = payload.media_type.as_deref().or(Some("series"));
    let meta = fetch_by_id(&payload.provider_id, media_type, &pool, payload.provider_name.as_deref()).await
        .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?;

    catalog::apply_series_metadata(&pool, series_id, &meta).await?;
    refresh_episode_details(&pool, series_id, &payload.provider_id, payload.provider_name.as_deref()).await?;

    Ok(Json(media_service::series_detail(&pool, series_id).await?))
}

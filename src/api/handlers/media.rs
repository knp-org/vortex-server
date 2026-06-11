use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::services::media_service;
use crate::models::db::media::Media;

pub async fn get_library_media(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<Media>>, AppError> {
    let media = media_service::get_by_library_id(&pool, id).await?;
    Ok(Json(media))
}

pub async fn get_recently_added(State(pool): State<SqlitePool>) -> Result<Json<Vec<Media>>, AppError> {
    let media = media_service::get_recently_added(&pool).await?;
    Ok(Json(media))
}

pub async fn get_media_details(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Media>, AppError> {
    let item = media_service::get_details(&pool, id).await?;
    
    tracing::info!("Fetched media details for id {}. Cast present: {}", id, item.cast.is_some());
    if let Some(c) = &item.cast {
        tracing::debug!("Cast data length: {}", c.len());
    }

    Ok(Json(item))
}

pub async fn refresh_media_metadata(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
) -> Result<Json<Media>, AppError> {
    use crate::services::metadata::{fetch_metadata, fetch_by_id, get_default_provider};

    let media = sqlx::query_as::<_, Media>("
        SELECT 
            m.id, m.library_id, m.file_path, m.title, m.year, m.poster_url, m.plot, m.media_type, 
            m.added_at, m.series_name, m.season_number, m.episode_number, m.provider_ids, 
            m.backdrop_url, m.still_url, m.runtime, m.genres, m.rating, m.cast, m.director, m.media_info,
            m.age_rating, m.studio, m.trailer_url, m.origin_country, m.collection_name, m.creator, m.tags,
            l.library_type, 
            ('/api/v1/stream/' || m.id) as stream_url 
        FROM media m 
        JOIN libraries l ON m.library_id = l.id 
        WHERE m.id = ?
    ")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Media with id {} not found", id)))?;

    // Try to get provider ID to fetch exact match
    let mut selected_provider_name = get_default_provider(&pool).await;
    let mut selected_provider_id = None;

    if let Some(json_str) = media.provider_ids.as_ref() {
        if let Ok(ids) = serde_json::from_str::<std::collections::HashMap<String, serde_json::Value>>(json_str) {
            if let Some(v) = ids.get(&selected_provider_name) {
                selected_provider_id = Some(v.clone());
            } else if let Some((p_name, v)) = ids.iter().next() {
                selected_provider_name = p_name.clone();
                selected_provider_id = Some(v.clone());
            }
        }
    }

    let provider_id = selected_provider_id.and_then(|v| {
        if let Some(s) = v.as_str() {
            Some(s.to_string())
        } else if let Some(i) = v.as_i64() {
            Some(i.to_string())
        } else {
            None
        }
    });

    let type_hint = if media.series_name.is_some() { Some("series") } else { Some("movie") };

    let meta = if let Some(id_str) = provider_id {
        tracing::info!("Refreshing metadata using ID: {} from provider: {}", id_str, selected_provider_name);
        fetch_by_id(&id_str, type_hint, &pool, Some(&selected_provider_name)).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata by ID: {}", e)))?
    } else {
        let title_to_search = if let Some(t) = &media.title {
            t.clone()
        } else {
             std::path::Path::new(&media.file_path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        };

        tracing::info!("Refreshing metadata by searching: {}", title_to_search);
        fetch_metadata(&title_to_search, type_hint, &pool).await
            .map_err(|e| AppError::External(format!("Failed to fetch metadata: {}", e)))?
    };

    media_service::update_media_metadata(&pool, id, &meta).await?;
    
    get_media_details(State(pool), Path(id)).await
}

use crate::api::dtos::requests::{SearchQuery, IdentifyRequest};

pub async fn search_handler(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<crate::models::metadata::NormalizedMetadata>>, AppError> {
    use crate::services::metadata::{search, fetch_by_id};

    let media_type = params.media_type.as_deref();
    
    // Check if query is a numeric ID
    if let Ok(id) = params.query.trim().parse::<i64>() {
        let meta = fetch_by_id(&id.to_string(), media_type, &pool, None).await?;
        Ok(Json(vec![meta]))
    } else {
        let results = search(&params.query, params.year.clone(), media_type, &pool).await?;
        Ok(Json(results))
    }
}


pub async fn identify_media(
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(payload): Json<IdentifyRequest>,
) -> Result<Json<Media>, AppError> {
    use crate::services::metadata::fetch_by_id;

    let media_type = payload.media_type.as_deref();
    let meta = fetch_by_id(&payload.provider_id, media_type, &pool, payload.provider_name.as_deref()).await?;

    media_service::update_media_metadata(&pool, id, &meta).await?;
    
    get_media_details(State(pool), Path(id)).await
}

pub async fn search_library(
    State(pool): State<SqlitePool>,
    axum::extract::Query(params): axum::extract::Query<SearchQuery>,
) -> Result<Json<Vec<Media>>, AppError> {
    let query_param = format!("%{}%", params.query);
    
    // Use separate queries for better readability and query plan caching
    let media = if let Some(media_type) = &params.media_type {
        sqlx::query_as::<_, Media>(
            "SELECT 
                m.id, m.library_id, m.file_path, 
                COALESCE(m.series_name, m.title) as title, 
                m.year, m.poster_url, m.plot, 
                (CASE WHEN m.series_name IS NOT NULL THEN 'series' ELSE m.media_type END) as media_type, 
                m.added_at, m.series_name, 
                NULL as season_number, NULL as episode_number, 
                m.provider_ids, m.backdrop_url, m.still_url, 
                m.runtime, m.genres, m.rating, m.cast, m.director, m.media_info,
                m.age_rating, m.studio, m.trailer_url, m.origin_country, m.collection_name, m.creator, m.tags,
                l.library_type,
                ('/api/v1/stream/' || m.id) as stream_url
             FROM media m
             JOIN libraries l ON m.library_id = l.id
             WHERE (m.title LIKE ? OR m.series_name LIKE ? OR m.plot LIKE ?)
               AND l.library_type != 'other'
             GROUP BY COALESCE(m.series_name, m.id)
             HAVING l.library_type = ?
             ORDER BY title ASC LIMIT 20"
        )
        .bind(&query_param)
        .bind(&query_param)
        .bind(&query_param)
        .bind(media_type)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query_as::<_, Media>(
            "SELECT 
                m.id, m.library_id, m.file_path, 
                COALESCE(m.series_name, m.title) as title, 
                m.year, m.poster_url, m.plot, 
                (CASE WHEN m.series_name IS NOT NULL THEN 'series' ELSE m.media_type END) as media_type, 
                m.added_at, m.series_name, 
                NULL as season_number, NULL as episode_number, 
                m.provider_ids, m.backdrop_url, m.still_url, 
                m.runtime, m.genres, m.rating, m.cast, m.director, m.media_info,
                m.age_rating, m.studio, m.trailer_url, m.origin_country, m.collection_name, m.creator, m.tags,
                l.library_type,
                ('/api/v1/stream/' || m.id) as stream_url
             FROM media m
             JOIN libraries l ON m.library_id = l.id
             WHERE (m.title LIKE ? OR m.series_name LIKE ? OR m.plot LIKE ?)
               AND l.library_type != 'other'
             GROUP BY COALESCE(m.series_name, m.id)
             ORDER BY title ASC LIMIT 20"
        )
        .bind(&query_param)
        .bind(&query_param)
        .bind(&query_param)
        .fetch_all(&pool)
        .await?
    };
    
    Ok(Json(media))
}

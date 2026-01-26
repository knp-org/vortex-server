//! Media Service - Common DB operations for media metadata management.
//! This module reduces code duplication between movie and TV handlers.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::metadata::NormalizedMetadata;


/// Update a single media item (movie or episode) with fetched metadata.
pub async fn update_media_metadata(
    pool: &SqlitePool,
    id: i64,
    meta: &NormalizedMetadata,
) -> Result<(), AppError> {
    let genres_str = meta.genres.as_ref().map(|g| g.join(", "));
    
    // Parse year safely
    let year = meta.year.as_ref()
        .and_then(|y| y.parse::<i64>().ok())
        .unwrap_or(0);

    sqlx::query(
        "UPDATE media SET title = ?, year = ?, poster_url = ?, backdrop_url = ?, plot = ?, media_type = ?, runtime = ?, genres = ?, provider_ids = ? WHERE id = ?"
    )
    .bind(&meta.title)
    .bind(year)
    .bind(&meta.poster_url)
    .bind(&meta.backdrop_url)
    .bind(&meta.plot)
    .bind(&meta.media_type)
    .bind(meta.runtime)
    .bind(genres_str)
    .bind(meta.provider_ids.as_ref().map(|v| v.to_string()))
    .bind(id)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// Update all episodes in a series with series-level metadata (poster, backdrop, plot, year, genres).
pub async fn update_series_metadata(
    pool: &SqlitePool,
    series_name: &str,
    meta: &NormalizedMetadata,
) -> Result<(), AppError> {
    let genres_str = meta.genres.as_ref().map(|g| g.join(", "));
    
    let year = meta.year.as_ref()
        .and_then(|y| y.parse::<i64>().ok())
        .unwrap_or(0);
    
    sqlx::query(
        "UPDATE media SET poster_url = ?, backdrop_url = ?, plot = ?, year = ?, genres = ?, provider_ids = ? WHERE series_name = ?"
    )
    .bind(&meta.poster_url)
    .bind(&meta.backdrop_url)
    .bind(&meta.plot)
    .bind(year)
    .bind(genres_str)
    .bind(meta.provider_ids.as_ref().map(|v| v.to_string()))
    .bind(series_name)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// Update a single episode with episode-specific details (title, plot, still image).
pub async fn update_episode_details(
    pool: &SqlitePool,
    series_name: &str,
    season_number: i32,
    episode_number: i32,
    title: &str,
    plot: &str,
    still_url: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE media SET title = ?, plot = ?, still_url = ? WHERE series_name = ? AND season_number = ? AND episode_number = ?"
    )
    .bind(title)
    .bind(plot)
    .bind(still_url)
    .bind(series_name)
    .bind(season_number)
    .bind(episode_number)
    .execute(pool)
    .await?;
    
    Ok(())
}

/// Get all distinct season numbers for a series.
pub async fn get_series_seasons(pool: &SqlitePool, series_name: &str) -> Result<Vec<i32>, AppError> {
    let seasons: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT season_number FROM media WHERE series_name = ? AND season_number IS NOT NULL"
    )
    .bind(series_name)
    .fetch_all(pool)
    .await?;
    
    Ok(seasons)
}

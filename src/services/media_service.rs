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

    let cast_json = meta.cast.as_ref().and_then(|c| serde_json::to_string(c).ok());

    let director_str = meta.director.as_ref().map(|d| d.join(", "));

    sqlx::query("
        UPDATE media 
        SET title = ?, year = ?, plot = ?, poster_url = ?, backdrop_url = ?, 
            media_type = ?, provider_ids = ?, genres = ?, runtime = ?, rating = ?, cast = ?, director = ?
        WHERE id = ?
    ")
    .bind(&meta.title)
    .bind(year) // Use the parsed year
    .bind(&meta.plot)
    .bind(&meta.poster_url)
    .bind(&meta.backdrop_url)
    .bind(&meta.media_type)
    .bind(meta.provider_ids.as_ref().map(|v| v.to_string())) // Keep original provider_ids binding
    .bind(genres_str)
    .bind(meta.runtime)
    .bind(meta.rating) // Add rating
    .bind(cast_json) // Add cast_json
    .bind(director_str) // Add director as comma-separated string
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
    
    let cast_json = meta.cast.as_ref().and_then(|c| serde_json::to_string(c).ok());

    let director_str = meta.director.as_ref().map(|d| d.join(", "));

    sqlx::query(
        "UPDATE media SET poster_url = ?, backdrop_url = ?, plot = ?, year = ?, genres = ?, provider_ids = ?, cast = ?, director = ?, rating = ? WHERE series_name = ?"
    )
    .bind(&meta.poster_url)
    .bind(&meta.backdrop_url)
    .bind(&meta.plot)
    .bind(year)
    .bind(genres_str)
    .bind(meta.provider_ids.as_ref().map(|v| v.to_string()))
    .bind(cast_json)
    .bind(director_str)
    .bind(meta.rating)
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

/// Get all media in a library
pub async fn get_by_library_id(pool: &SqlitePool, library_id: i64) -> Result<Vec<crate::models::db::media::Media>, AppError> {
    let media = sqlx::query_as::<_, crate::models::db::media::Media>("
        SELECT m.*, l.library_type, ('/api/v1/stream/' || m.id) as stream_url 
        FROM media m 
        JOIN libraries l ON m.library_id = l.id 
        WHERE m.library_id = ? 
        ORDER BY m.title ASC
    ")
        .bind(library_id)
        .fetch_all(pool)
        .await?;
    Ok(media)
}

/// Get recently added media
pub async fn get_recently_added(pool: &SqlitePool) -> Result<Vec<crate::models::db::media::Media>, AppError> {
    let query = "
        SELECT 
            MAX(media.id) as id,
            media.library_id,
            l.library_type,
            MAX(media.file_path) as file_path,
            COALESCE(media.series_name, media.title) as title,
            media.year,
            (CASE WHEN media.series_name IS NOT NULL THEN 
                (SELECT poster_url FROM media m2 WHERE m2.series_name = media.series_name AND m2.poster_url IS NOT NULL LIMIT 1)
             ELSE media.poster_url END) as poster_url,
            media.plot,
            (CASE WHEN media.series_name IS NOT NULL THEN 'series' ELSE 'movie' END) as media_type,
            MAX(media.added_at) as added_at,
            media.series_name,
            NULL as season_number,
            NULL as episode_number,
            NULL as provider_ids,
            (CASE WHEN media.series_name IS NOT NULL THEN 
                (SELECT backdrop_url FROM media m2 WHERE m2.series_name = media.series_name AND m2.backdrop_url IS NOT NULL LIMIT 1)
             ELSE media.backdrop_url END) as backdrop_url,
            NULL as still_url,
            media.runtime,
            media.genres,
            media.rating,
            media.cast,
            media.director,
            media.media_info
        FROM media
        JOIN libraries l ON media.library_id = l.id
        WHERE l.library_type != 'other'
        GROUP BY COALESCE(media.series_name, media.id)
        ORDER BY MAX(media.added_at) DESC
        LIMIT 20
    ";

    let media = sqlx::query_as::<_, crate::models::db::media::Media>(query)
        .fetch_all(pool)
        .await?;
    
    Ok(media)
}

/// Get media details by ID
pub async fn get_details(pool: &SqlitePool, id: i64) -> Result<crate::models::db::media::Media, AppError> {
    let item = sqlx::query_as::<_, crate::models::db::media::Media>("
        SELECT 
            m.id, m.library_id, m.file_path, m.title, m.year, m.poster_url, m.plot, m.media_type, 
            m.added_at, m.series_name, m.season_number, m.episode_number, m.provider_ids, 
            m.backdrop_url, m.still_url, m.runtime, m.genres, m.rating, m.cast, m.director, m.media_info,
            l.library_type, 
            ('/api/v1/stream/' || m.id) as stream_url 
        FROM media m 
        JOIN libraries l ON m.library_id = l.id 
        WHERE m.id = ?
    ")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Media with id {} not found", id)))?;
    
    Ok(item)
}

use sqlx::FromRow;
use serde::{Deserialize, Serialize};
use super::library::LibraryType;

#[allow(dead_code)]
#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct PlaybackProgress {
    pub media_id: i64,
    pub position: i64, // seconds
    pub total_duration: i64, // seconds
    pub last_watched: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Media {
    pub id: i64,
    pub library_id: i64,
    pub file_path: String,
    pub title: Option<String>,
    pub year: Option<i64>,
    pub poster_url: Option<String>,
    pub plot: Option<String>,
    pub media_type: Option<String>, // "movie" or "episode", kept for compatibility/metadata
    pub added_at: Option<chrono::NaiveDateTime>,
    // TV Show specific fields
    pub series_name: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub provider_ids: Option<String>,
    pub backdrop_url: Option<String>,
    pub still_url: Option<String>,
    pub runtime: Option<i32>,
    pub genres: Option<String>,
    pub library_type: Option<LibraryType>,
}


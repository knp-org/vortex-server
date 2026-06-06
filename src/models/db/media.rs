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
    pub rating: Option<f32>,
    pub cast: Option<String>, // JSON stored as string
    pub director: Option<String>, // JSON or comma separated
    pub library_type: Option<LibraryType>,
    #[sqlx(default)]
    pub stream_url: Option<String>,
    pub media_info: Option<String>, // JSON string of MediaInfo
    // Note: book-specific fields (page_count, reading_mode) live on the dedicated
    // `Book` model (src/models/db/book.rs), not here.
}

// Structures for detailed media info (stored as JSON in media_info)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaInfo {
    pub container: Option<String>,
    pub size: Option<i64>,
    pub bit_rate: Option<i64>,
    pub video: Option<VideoStream>,
    pub audio: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
    pub duration: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoStream {
    pub index: i32,
    pub codec: String,
    pub profile: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub aspect_ratio: Option<String>,
    pub bit_rate: Option<i64>,
    pub frame_rate: Option<String>,
    pub bit_depth: Option<i32>,
    pub pixel_format: Option<String>,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub ref_frames: Option<i32>,
    pub codec_tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioStream {
    pub index: i32,
    pub codec: String,
    pub channels: Option<i32>,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
    pub bit_rate: Option<i64>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleStream {
    pub index: i32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_external: bool,
    pub is_forced: bool,
    pub is_default: bool,
}

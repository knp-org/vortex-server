//! `music_videos` detail row (1:1 with a `media_items` spine row of type `music_video`).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MusicVideo {
    pub item_id: i64,
    pub title: Option<String>,
    pub artist_id: Option<i64>,
    pub artist_name: Option<String>,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub runtime: Option<i64>,
}

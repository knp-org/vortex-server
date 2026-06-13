//! `tracks` detail row (1:1 with a `media_items` spine row of type `track`).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Track {
    pub item_id: i64,
    pub album_id: Option<i64>,
    pub artist_id: Option<i64>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub title: Option<String>,
    pub duration: Option<i64>,
}

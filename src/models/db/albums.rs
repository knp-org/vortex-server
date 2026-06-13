//! `albums` table model.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Album {
    pub id: i64,
    pub artist_id: Option<i64>,
    pub library_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub cover_url: Option<String>,
    pub provider_ids: Option<String>,
}

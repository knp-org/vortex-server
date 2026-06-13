//! `artists` table model.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Artist {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub bio: Option<String>,
    pub image_url: Option<String>,
    pub provider_ids: Option<String>,
}

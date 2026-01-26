use sqlx::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type, PartialEq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LibraryType {
    Movies,
    TvShows,
    MusicVideos,
    Other,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub library_type: LibraryType,
}

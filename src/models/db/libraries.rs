use sqlx::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, sqlx::Type, PartialEq)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum LibraryType {
    Movies,
    TvShows,
    Music,
    MusicVideos,
    Books,
    Images,
    Other,
}

/// A library and all of the folder paths it scans.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub paths: Vec<String>,
    pub library_type: LibraryType,
    /// Default reading mode applied to books in this library when the book has no
    /// per-book override. Only meaningful for `Books` libraries.
    pub default_reading_mode: Option<String>,
}

/// Raw `libraries` table row, without the associated `library_paths`.
#[derive(Debug, FromRow, Clone)]
pub struct LibraryRow {
    pub id: i64,
    pub name: String,
    pub library_type: LibraryType,
    pub default_reading_mode: Option<String>,
}

use sqlx::FromRow;
use serde::{Deserialize, Serialize};

/// A book item (pdf / cbz / epub).
///
/// Books live in the shared `media` table (`media_type = 'book'`) alongside
/// videos, but are modelled as their own type so book logic never has to reach
/// through the video-centric [`Media`](super::media::Media) struct and its
/// movie/episode fields. This is the "typed model over a shared store" approach.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Book {
    pub id: i64,
    pub library_id: i64,
    pub file_path: String,
    pub title: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub added_at: Option<chrono::NaiveDateTime>,
    /// Total pages. Populated for CBZ at scan time; `None` for PDF/EPUB
    /// (the client determines those via pdf.js / epub.js).
    pub page_count: Option<i64>,
    /// Per-book reading-mode override. When unset the library default applies.
    pub reading_mode: Option<String>,
}

/// The set of columns selected to hydrate a [`Book`] from the `media` table.
/// Kept in one place so the query shape lives in a single location.
pub const BOOK_COLUMNS: &str =
    "id, library_id, file_path, title, plot, poster_url, added_at, page_count, reading_mode";

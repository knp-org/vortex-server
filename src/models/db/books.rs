//! `books` detail row (1:1 with a `media_items` spine row of type `book`).
//!
//! A book is hydrated by joining `media_items` (for `library_id`/`file_path`) with
//! `books` (book-only metadata). It carries no TV-shaped fields — the old shared-table
//! design that reused `series_name`/`season_number` for comics is gone.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Book {
    /// = `media_items.id`.
    pub item_id: i64,
    pub library_id: i64,
    pub file_path: String,
    pub title: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    /// Total pages. Populated for CBZ at scan time; `None` for PDF/EPUB
    /// (the client determines those via pdf.js / epub.js).
    pub page_count: Option<i64>,
    /// Per-book reading-mode override. When unset the library default applies.
    pub reading_mode: Option<String>,
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub isbn: Option<String>,
    pub book_series_id: Option<i64>,
    pub chapter_number: Option<f64>,
}

/// Columns selected to hydrate a [`Book`] by joining `media_items` (aliased `mi`)
/// with `books` (aliased `b`). Kept in one place so the query shape lives once.
pub const BOOK_SELECT: &str = "mi.id AS item_id, mi.library_id, mi.file_path, \
    b.title, b.plot, b.poster_url, b.page_count, b.reading_mode, \
    b.publisher, b.published_date, b.isbn, b.book_series_id, b.chapter_number";

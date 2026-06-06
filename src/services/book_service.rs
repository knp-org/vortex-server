//! Book repository / service.
//!
//! Owns all persistence for [`Book`] items in the shared `media` table, so book
//! handlers and the scanner never write raw book SQL inline. File/format
//! operations (zip extraction, page counting) live in [`crate::services::books`].

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::models::db::book::{Book, BOOK_COLUMNS};

pub const DEFAULT_READING_MODE: &str = "vertical";
pub const VALID_READING_MODES: &[&str] = &["vertical", "horizontal_ltr", "horizontal_rtl", "webtoon"];

pub fn is_valid_reading_mode(mode: &str) -> bool {
    VALID_READING_MODES.contains(&mode)
}

pub struct BookService {
    pool: SqlitePool,
}

impl BookService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Fetch a book by id, erroring if the item is missing or isn't a book.
    pub async fn get(&self, id: i64) -> Result<Book, AppError> {
        self.get_optional(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Book {} not found", id)))
    }

    /// Fetch a book by id, returning `None` for missing items or non-book media.
    pub async fn get_optional(&self, id: i64) -> Result<Option<Book>, AppError> {
        let query = format!(
            "SELECT {BOOK_COLUMNS} FROM media WHERE id = ? AND media_type = 'book'"
        );
        Ok(sqlx::query_as::<_, Book>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Resolve the effective reading mode: per-book override → library default →
    /// global default. Invalid stored values are ignored.
    pub async fn resolve_reading_mode(&self, book: &Book) -> Result<String, AppError> {
        if let Some(mode) = book.reading_mode.as_deref().filter(|m| is_valid_reading_mode(m)) {
            return Ok(mode.to_string());
        }

        let library_default: Option<String> = sqlx::query_scalar::<_, Option<String>>(
            "SELECT default_reading_mode FROM libraries WHERE id = ?",
        )
        .bind(book.library_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        Ok(library_default
            .filter(|m| is_valid_reading_mode(m))
            .unwrap_or_else(|| DEFAULT_READING_MODE.to_string()))
    }

    /// Set a book's per-book reading-mode override.
    pub async fn set_reading_mode(&self, id: i64, mode: &str) -> Result<(), AppError> {
        if !is_valid_reading_mode(mode) {
            return Err(AppError::BadRequest(format!("Invalid reading mode: {}", mode)));
        }
        let result = sqlx::query("UPDATE media SET reading_mode = ? WHERE id = ? AND media_type = 'book'")
            .bind(mode)
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Book {} not found", id)));
        }
        Ok(())
    }

    /// Persist a computed page count (e.g. a CBZ backfill).
    pub async fn set_page_count(&self, id: i64, count: i64) -> Result<(), AppError> {
        sqlx::query("UPDATE media SET page_count = ? WHERE id = ?")
            .bind(count)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert a freshly scanned book, or refresh the library link / page count
    /// of an existing row. Title and reading-mode overrides are preserved.
    pub async fn upsert_scanned(
        &self,
        file_path: &str,
        title: &str,
        library_id: i64,
        page_count: Option<i64>,
    ) -> Result<(), AppError> {
        let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM media WHERE file_path = ?")
            .bind(file_path)
            .fetch_optional(&self.pool)
            .await?;

        if let Some((id,)) = existing {
            sqlx::query(
                "UPDATE media SET library_id = ?, media_type = 'book', page_count = COALESCE(?, page_count) WHERE id = ?",
            )
            .bind(library_id)
            .bind(page_count)
            .bind(id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO media (file_path, title, library_id, media_type, page_count) VALUES (?, ?, ?, 'book', ?)",
            )
            .bind(file_path)
            .bind(title)
            .bind(library_id)
            .bind(page_count)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}

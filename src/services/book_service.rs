//! Book repository / service.
//!
//! Owns reads/updates for [`Book`] items over `media_items` + `books`. Scanning goes
//! through [`crate::services::catalog::upsert_book`]; file/format operations (zip
//! extraction, page counting) live in [`crate::services::books`].

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::models::db::books::{Book, BOOK_SELECT};

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

    /// Fetch a book by item id, erroring if missing or not a book.
    pub async fn get(&self, id: i64) -> Result<Book, AppError> {
        self.get_optional(id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Book {} not found", id)))
    }

    /// Fetch a book by item id, returning `None` for missing or non-book items.
    pub async fn get_optional(&self, id: i64) -> Result<Option<Book>, AppError> {
        let query = format!(
            "SELECT {BOOK_SELECT} FROM media_items mi JOIN books b ON b.item_id = mi.id \
             WHERE mi.id = ? AND mi.item_type = 'book'"
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
        let result = sqlx::query("UPDATE books SET reading_mode = ? WHERE item_id = ?")
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
        sqlx::query("UPDATE books SET page_count = ? WHERE item_id = ?")
            .bind(count)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

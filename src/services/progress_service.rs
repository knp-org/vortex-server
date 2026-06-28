//! Progress Service
//!
//! Owns reads/writes for per-user playback/reading progress
//! (`user_media_progress`), including the "continue watching" rail.

use sqlx::SqlitePool;
use crate::error::AppError;

/// A row backing the "continue watching" rail. The handler shapes this into the
/// public response (adding the stream URL).
#[derive(sqlx::FromRow)]
pub struct ContinueWatchingRow {
    pub id: i64,
    pub item_type: String,
    pub title: Option<String>,
    pub poster_url: Option<String>,
    pub position: i64,
    pub total_duration: i64,
    pub reading_style: Option<String>,
}

pub struct ProgressService {
    pool: SqlitePool,
}

impl ProgressService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// In-progress items for the user: started but not yet effectively finished,
    /// excluding `other`-type libraries, most-recent first.
    pub async fn continue_watching(&self, user_id: i64) -> Result<Vec<ContinueWatchingRow>, AppError> {
        Ok(sqlx::query_as::<_, ContinueWatchingRow>(
            "SELECT mi.id, mi.item_type,
                    COALESCE(mv.title, e.title, mvd.title) AS title,
                    COALESCE(mv.poster_url, e.still_url, mvd.poster_url) AS poster_url,
                    p.position, p.total_duration, p.reading_style
             FROM user_media_progress p
             JOIN media_items mi ON mi.id = p.item_id
             JOIN libraries l ON l.id = mi.library_id AND l.library_type != 'other'
             LEFT JOIN movies mv ON mv.item_id = mi.id
             LEFT JOIN episodes e ON e.item_id = mi.id
             LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
             WHERE p.user_id = ? AND p.position > 10 AND p.position < (p.total_duration * 0.95)
             ORDER BY p.last_watched DESC LIMIT 10"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Upsert a user's progress for an item. A `None` reading_style preserves any
    /// previously stored value.
    pub async fn update(&self, user_id: i64, item_id: i64, position: i64, total_duration: i64, reading_style: Option<&str>) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_media_progress (user_id, item_id, position, total_duration, reading_style, last_watched)
             VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(user_id, item_id) DO UPDATE SET
                position = excluded.position,
                total_duration = excluded.total_duration,
                reading_style = COALESCE(excluded.reading_style, user_media_progress.reading_style),
                last_watched = CURRENT_TIMESTAMP"
        )
        .bind(user_id)
        .bind(item_id)
        .bind(position)
        .bind(total_duration)
        .bind(reading_style)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// A user's stored `(position, reading_style)` for an item, defaulting to
    /// `(0, None)` when none exists.
    pub async fn get(&self, user_id: i64, item_id: i64) -> Result<(i64, Option<String>), AppError> {
        let row: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT position, reading_style FROM user_media_progress WHERE user_id = ? AND item_id = ?"
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or((0, None)))
    }
}

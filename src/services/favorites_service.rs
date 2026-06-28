//! Favorites Service
//!
//! Owns reads/writes for the per-user `user_favorites` table.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::api::dtos::responses::Card;

pub struct FavoritesService {
    pool: SqlitePool,
}

impl FavoritesService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Favorites can point at any file-backed item; surface a card per type.
    pub async fn list(&self, user_id: i64) -> Result<Vec<Card>, AppError> {
        Ok(sqlx::query_as::<_, Card>(
            "SELECT mi.id,
                    mi.item_type AS kind,
                    COALESCE(mv.title, e.title, b.title, mvd.title) AS title,
                    COALESCE(mv.poster_url, e.still_url, b.poster_url, mvd.poster_url) AS poster_url,
                    COALESCE(mv.year, mvd.year) AS year,
                    CASE WHEN mi.item_type IN ('movie','episode','music_video')
                         THEN ('/api/v1/stream/' || mi.id) END AS stream_url
             FROM user_favorites f
             JOIN media_items mi ON mi.id = f.item_id
             LEFT JOIN movies mv ON mv.item_id = mi.id
             LEFT JOIN episodes e ON e.item_id = mi.id
             LEFT JOIN books b ON b.item_id = mi.id
             LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
             WHERE f.user_id = ?
             ORDER BY f.created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn add(&self, user_id: i64, item_id: i64) -> Result<(), AppError> {
        sqlx::query("INSERT OR IGNORE INTO user_favorites (user_id, item_id) VALUES (?, ?)")
            .bind(user_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn remove(&self, user_id: i64, item_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM user_favorites WHERE user_id = ? AND item_id = ?")
            .bind(user_id)
            .bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

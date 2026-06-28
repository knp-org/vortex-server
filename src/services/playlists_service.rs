//! Playlists Service
//!
//! Owns reads/writes for per-user music playlists (`playlists`,
//! `playlist_tracks`), including ownership checks.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::api::dtos::responses::{PlaylistDto, PlaylistDetail};
use crate::services::media_service;

pub struct PlaylistsService {
    pool: SqlitePool,
}

impl PlaylistsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Ensure the playlist exists and belongs to the caller.
    pub async fn assert_owner(&self, playlist_id: i64, user_id: i64) -> Result<(), AppError> {
        let owner: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .fetch_optional(&self.pool)
            .await?;
        match owner {
            Some((uid,)) if uid == user_id => Ok(()),
            Some(_) => Err(AppError::Forbidden("Not your playlist".to_string())),
            None => Err(AppError::NotFound("Playlist not found".to_string())),
        }
    }

    pub async fn list(&self, user_id: i64) -> Result<Vec<PlaylistDto>, AppError> {
        Ok(sqlx::query_as::<_, PlaylistDto>(
            "SELECT p.id, p.name,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count,
                    p.created_at
             FROM playlists p WHERE p.user_id = ? ORDER BY p.created_at DESC"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create(&self, user_id: i64, name: &str) -> Result<PlaylistDto, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Playlist name is required".to_string()));
        }
        let id = sqlx::query("INSERT INTO playlists (user_id, name) VALUES (?, ?)")
            .bind(user_id)
            .bind(name)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();

        Ok(sqlx::query_as::<_, PlaylistDto>(
            "SELECT id, name, 0 AS track_count, created_at FROM playlists WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Full playlist with its ordered tracks. Caller must have verified ownership.
    pub async fn detail(&self, playlist_id: i64) -> Result<PlaylistDetail, AppError> {
        let name: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
            .bind(playlist_id).fetch_one(&self.pool).await?;
        Ok(PlaylistDetail {
            id: playlist_id,
            name: name.0,
            tracks: media_service::MediaService::new(self.pool.clone()).playlist_tracks(playlist_id).await?,
        })
    }

    pub async fn add_track(&self, playlist_id: i64, item_id: i64) -> Result<(), AppError> {
        sqlx::query(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, item_id, position)
             VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?))"
        )
        .bind(playlist_id).bind(item_id).bind(playlist_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_track(&self, playlist_id: i64, item_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND item_id = ?")
            .bind(playlist_id).bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, playlist_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

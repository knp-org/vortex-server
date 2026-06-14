//! Per-user music playlists.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use sqlx::SqlitePool;
use serde::Deserialize;
use crate::error::AppError;
use crate::api::middleware::AuthUser;
use crate::api::dtos::responses::{PlaylistDto, PlaylistDetail};
use crate::services::media_service;

#[derive(Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddTrackRequest {
    pub item_id: i64,
}

/// Ensure the playlist exists and belongs to the caller.
async fn assert_owner(pool: &SqlitePool, playlist_id: i64, user_id: i64) -> Result<(), AppError> {
    let owner: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM playlists WHERE id = ?")
        .bind(playlist_id)
        .fetch_optional(pool)
        .await?;
    match owner {
        Some((uid,)) if uid == user_id => Ok(()),
        Some(_) => Err(AppError::Forbidden("Not your playlist".to_string())),
        None => Err(AppError::NotFound("Playlist not found".to_string())),
    }
}

pub async fn list_playlists(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<PlaylistDto>>, AppError> {
    let playlists = sqlx::query_as::<_, PlaylistDto>(
        "SELECT p.id, p.name,
                (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count,
                p.created_at
         FROM playlists p WHERE p.user_id = ? ORDER BY p.created_at DESC"
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(playlists))
}

pub async fn create_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<CreatePlaylistRequest>,
) -> Result<Json<PlaylistDto>, AppError> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Playlist name is required".to_string()));
    }
    let id = sqlx::query("INSERT INTO playlists (user_id, name) VALUES (?, ?)")
        .bind(user.id)
        .bind(name)
        .execute(&pool)
        .await?
        .last_insert_rowid();

    let dto = sqlx::query_as::<_, PlaylistDto>(
        "SELECT id, name, 0 AS track_count, created_at FROM playlists WHERE id = ?"
    )
    .bind(id)
    .fetch_one(&pool)
    .await?;
    Ok(Json(dto))
}

pub async fn get_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<PlaylistDetail>, AppError> {
    assert_owner(&pool, id, user.id).await?;
    let name: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
        .bind(id).fetch_one(&pool).await?;

    Ok(Json(PlaylistDetail {
        id,
        name: name.0,
        tracks: media_service::playlist_tracks(&pool, id).await?,
    }))
}

pub async fn add_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
    Json(payload): Json<AddTrackRequest>,
) -> Result<StatusCode, AppError> {
    assert_owner(&pool, id, user.id).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, item_id, position)
         VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?))"
    )
    .bind(id).bind(payload.item_id).bind(id)
    .execute(&pool)
    .await?;
    Ok(StatusCode::OK)
}

pub async fn remove_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path((id, item_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    assert_owner(&pool, id, user.id).await?;
    sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND item_id = ?")
        .bind(id).bind(item_id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    assert_owner(&pool, id, user.id).await?;
    sqlx::query("DELETE FROM playlists WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

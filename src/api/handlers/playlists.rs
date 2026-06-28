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
use crate::services::playlists_service::PlaylistsService;

#[derive(Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct AddTrackRequest {
    pub item_id: i64,
}

pub async fn list_playlists(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<PlaylistDto>>, AppError> {
    Ok(Json(PlaylistsService::new(pool).list(user.id).await?))
}

pub async fn create_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<CreatePlaylistRequest>,
) -> Result<Json<PlaylistDto>, AppError> {
    Ok(Json(PlaylistsService::new(pool).create(user.id, &payload.name).await?))
}

pub async fn get_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<Json<PlaylistDetail>, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_owner(id, user.id).await?;
    Ok(Json(service.detail(id).await?))
}

pub async fn add_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
    Json(payload): Json<AddTrackRequest>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_owner(id, user.id).await?;
    service.add_track(id, payload.item_id).await?;
    Ok(StatusCode::OK)
}

pub async fn remove_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path((id, item_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_owner(id, user.id).await?;
    service.remove_track(id, item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_owner(id, user.id).await?;
    service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

//! Per-user playlists.
//!
//! Playlists come in kinds (`music`, `movie`, `tvshow`, `music_video`, `other`);
//! `other` is hidden and only becomes visible once the caller exchanges their
//! PIN for a short-lived unlock token and sends it back in the
//! [`UNLOCK_HEADER`](crate::api::middleware::UNLOCK_HEADER).

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Extension,
    Json,
};
use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use crate::api::middleware::{hidden_unlocked, make_unlock_token, AuthUser};
use crate::api::dtos::responses::{PlaylistDto, PlaylistDetail};
use crate::services::playlists_service::{normalize_kind, PlaylistsService};

#[derive(Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
    /// `music` (default) | `movie` | `tvshow` | `music_video` | `other`.
    pub kind: Option<String>,
}

#[derive(Deserialize)]
pub struct AddTrackRequest {
    pub item_id: i64,
}

#[derive(Deserialize)]
pub struct SetPinRequest {
    /// Required only when replacing an existing PIN.
    pub current_pin: Option<String>,
    pub pin: String,
}

#[derive(Deserialize)]
pub struct UnlockRequest {
    pub pin: String,
}

#[derive(Serialize)]
pub struct PinStatus {
    pub is_set: bool,
}

#[derive(Serialize)]
pub struct UnlockResponse {
    /// Send back as the `X-Vortex-Unlock` header to see hidden playlists.
    pub token: String,
    pub expires_in: i64,
}

pub async fn list_playlists(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<Vec<PlaylistDto>>, AppError> {
    let unlocked = hidden_unlocked(&headers, user.id);
    Ok(Json(PlaylistsService::new(pool).list(user.id, unlocked).await?))
}

pub async fn create_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    Json(payload): Json<CreatePlaylistRequest>,
) -> Result<Json<PlaylistDto>, AppError> {
    let kind = normalize_kind(payload.kind.as_deref())?;
    let unlocked = hidden_unlocked(&headers, user.id);
    Ok(Json(PlaylistsService::new(pool).create(user.id, &payload.name, &kind, unlocked).await?))
}

pub async fn get_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<PlaylistDetail>, AppError> {
    let service = PlaylistsService::new(pool);
    let kind = service.assert_access(id, user.id, hidden_unlocked(&headers, user.id)).await?;
    Ok(Json(service.detail(id, &kind).await?))
}

pub async fn add_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<AddTrackRequest>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    let kind = service.assert_access(id, user.id, hidden_unlocked(&headers, user.id)).await?;
    service.add_track(id, &kind, payload.item_id).await?;
    Ok(StatusCode::OK)
}

pub async fn remove_track(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    Path((id, item_id)): Path<(i64, i64)>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_access(id, user.id, hidden_unlocked(&headers, user.id)).await?;
    service.remove_track(id, item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_playlist(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let service = PlaylistsService::new(pool);
    service.assert_access(id, user.id, hidden_unlocked(&headers, user.id)).await?;
    service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Hidden-playlist PIN
// ---------------------------------------------------------------------------

/// Whether the caller has a PIN configured. Never returns the PIN itself.
pub async fn get_pin_status(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<PinStatus>, AppError> {
    let is_set = PlaylistsService::new(pool).pin_is_set(user.id).await?;
    Ok(Json(PinStatus { is_set }))
}

pub async fn set_pin(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<SetPinRequest>,
) -> Result<StatusCode, AppError> {
    PlaylistsService::new(pool)
        .set_pin(user.id, payload.current_pin.as_deref(), &payload.pin)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Exchange the PIN for a short-lived unlock token.
pub async fn unlock(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<UnlockRequest>,
) -> Result<Json<UnlockResponse>, AppError> {
    PlaylistsService::new(pool).verify_pin(user.id, &payload.pin).await?;
    let (token, expires_in) = make_unlock_token(user.id)?;
    Ok(Json(UnlockResponse { token, expires_in }))
}

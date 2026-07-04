//! Photo gallery (album) endpoints for Images libraries.
//!
//! Galleries group photos into albums, parallel to how series group episodes.
//! Reads are served from [`crate::services::media_service`]; mutations go through
//! [`crate::services::gallery_service`].

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::api::dtos::responses::{Card, GalleryDetail, ImageDto};
use crate::error::AppError;
use crate::services::gallery_service::GalleryService;
use crate::services::media_service::MediaService;

#[derive(Deserialize)]
pub struct LibraryScopedQuery {
    pub library_id: Option<i64>,
}

/// List galleries, optionally scoped to one library (`?library_id=`).
pub async fn list_galleries(
    State(pool): State<SqlitePool>,
    axum::extract::Query(q): axum::extract::Query<LibraryScopedQuery>,
) -> Result<Json<Vec<Card>>, AppError> {
    Ok(Json(MediaService::new(pool).gallery_cards(q.library_id).await?))
}

/// Every photo in an Images library (across all albums), for the "Add Photos"
/// album picker.
pub async fn list_library_images(
    Path(library_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<ImageDto>>, AppError> {
    Ok(Json(MediaService::new(pool).library_images(library_id).await?))
}

/// Gallery detail: album fields plus the photos it contains.
pub async fn get_gallery(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<GalleryDetail>, AppError> {
    Ok(Json(MediaService::new(pool).gallery_detail(id).await?))
}

#[derive(Deserialize)]
pub struct CreateGalleryRequest {
    pub library_id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

pub async fn create_gallery(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateGalleryRequest>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    let id = GalleryService::new(pool)
        .create(payload.library_id, &payload.name, payload.description.as_deref())
        .await?;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdateGalleryRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub cover_url: Option<String>,
}

pub async fn update_gallery(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateGalleryRequest>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool)
        .update(id, payload.name.as_deref(), payload.description.as_deref(), payload.cover_url.as_deref())
        .await?;
    Ok(StatusCode::OK)
}

pub async fn delete_gallery(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool).delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AddImagesRequest {
    pub item_ids: Vec<i64>,
}

/// Add (move) a set of photos into this gallery.
pub async fn add_images(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<AddImagesRequest>,
) -> Result<Json<Value>, AppError> {
    let moved = GalleryService::new(pool).add_images(id, &payload.item_ids).await?;
    Ok(Json(json!({ "moved": moved })))
}

/// Remove a photo from this gallery into the recycle bin (the photo is kept and
/// can be restored; see the trash endpoints below).
pub async fn remove_image(
    Path((id, item_id)): Path<(i64, i64)>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool).remove_image(id, item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Recycle bin: photos removed from their album in this Images library but not
/// yet permanently deleted. Each photo's `gallery_id` is the album it will be
/// restored to (its former home, or null if that album was deleted).
pub async fn list_trash(
    Path(library_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<ImageDto>>, AppError> {
    Ok(Json(MediaService::new(pool).trashed_images(library_id).await?))
}

/// Restore a photo out of the recycle bin, back into the album it came from.
pub async fn restore_image(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool).restore_image(item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Permanently delete a single photo from the recycle bin.
pub async fn purge_image(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool).purge_image(item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Empty the recycle bin for an Images library (permanently delete all trashed
/// photos). Returns how many were removed.
pub async fn empty_trash(
    Path(library_id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Value>, AppError> {
    let purged = GalleryService::new(pool).empty_trash(library_id).await?;
    Ok(Json(json!({ "purged": purged })))
}

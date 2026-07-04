use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::services::gallery_service::GalleryService;


/// Serve a cached image
pub async fn get_image(
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let cfg = crate::infrastructure::config::config();
    let images_dir = cfg.data_dir.join("thumbnails");
    let file_path = images_dir.join(&filename);

    if !file_path.exists() {
        return Err(AppError::NotFound("Image not found".to_string()));
    }

    let content = tokio::fs::read(&file_path).await
        .map_err(|e| AppError::Internal(format!("Failed to read image: {}", e)))?;

    let mut headers = HeaderMap::new();
    // Assuming JPEG for TMDB images, but could be PNG. 
    // TMDB usually sends .jpg
    if filename.ends_with(".png") {
        headers.insert(header::CONTENT_TYPE, "image/png".parse().unwrap());
    } else {
        headers.insert(header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
    }
    headers.insert(header::CACHE_CONTROL, "public, max-age=31536000, immutable".parse().unwrap());

    Ok((headers, content))
}

/// Serve the full-resolution original photo backing an image item (`media_items.id`).
/// Distinct from the cached-metadata `get_image` above, which serves by filename.
pub async fn get_image_file(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    let file_path: Option<String> = sqlx::query_scalar(
        "SELECT mi.file_path FROM media_items mi
         JOIN images i ON i.item_id = mi.id
         WHERE mi.id = ? AND mi.item_type = 'image'"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?;

    let file_path = file_path.ok_or_else(|| AppError::NotFound(format!("Image {} not found", id)))?;

    let content = tokio::fs::read(&file_path).await
        .map_err(|_| AppError::NotFound("Image file not found".to_string()))?;

    let mime = mime_guess::from_path(&file_path).first_or_octet_stream();

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.as_ref().parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "private, max-age=86400".parse().unwrap());

    Ok((headers, content))
}

#[derive(Deserialize)]
pub struct UpdateImageRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub taken_at: Option<String>,
    /// Move the photo into another gallery (album).
    #[serde(default)]
    pub gallery_id: Option<i64>,
}

/// Edit a photo's options (title, capture date, album membership).
pub async fn update_image(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateImageRequest>,
) -> Result<StatusCode, AppError> {
    GalleryService::new(pool)
        .update_image(id, payload.title.as_deref(), payload.taken_at.as_deref(), payload.gallery_id)
        .await?;
    Ok(StatusCode::OK)
}



use axum::{
    extract::Path,
    http::{header, HeaderMap},
    response::IntoResponse,
};
use std::path::PathBuf;
use crate::error::AppError;


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



use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use sqlx::SqlitePool;

use std::path::{Component, Path as StdPath, PathBuf};

use crate::api::middleware::AuthUser;
use crate::error::AppError;
use crate::services::gallery_service::GalleryService;

/// Resolve an untrusted file name against the thumbnail cache directory.
///
/// The name must be a single ordinary path component — no separators, no `..`, no
/// root or prefix — which rules out traversal before any filesystem call. A cached
/// thumbnail that is a symlink out of the cache is rejected too, so a poisoned
/// cache directory can't be used to read arbitrary files.
fn resolve_cached_image(images_dir: &StdPath, filename: &str) -> Result<PathBuf, AppError> {
    let invalid = || AppError::BadRequest("Invalid image name".to_string());

    let mut components = StdPath::new(filename).components();
    let name = match (components.next(), components.next()) {
        (Some(Component::Normal(name)), None) => name,
        _ => return Err(invalid()),
    };

    let file_path = images_dir.join(name);

    // Only meaningful once the file exists; a miss falls through to the caller's 404.
    if let (Ok(resolved), Ok(root)) = (file_path.canonicalize(), images_dir.canonicalize()) {
        if !resolved.starts_with(&root) {
            return Err(invalid());
        }
    }

    Ok(file_path)
}

/// Serve a cached image.
///
/// This route is public, so `filename` is untrusted: it is a bare file name within
/// the thumbnail cache and nothing else. Axum percent-decodes path params after
/// routing, so a `%2F` would otherwise smuggle a separator through and let the
/// `join` below escape the cache directory.
pub async fn get_image(
    Path(filename): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let cfg = crate::infrastructure::config::config();
    let images_dir = cfg.data_dir.join("thumbnails");
    let file_path = resolve_cached_image(&images_dir, &filename)?;

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

/// Edit a photo's options (title, capture date, album membership). Admin-only:
/// this mutates shared catalog content, not per-user state.
pub async fn update_image(
    Extension(auth_user): Extension<AuthUser>,
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateImageRequest>,
) -> Result<StatusCode, AppError> {
    auth_user.require_admin()?;
    GalleryService::new(pool)
        .update_image(id, payload.title.as_deref(), payload.taken_at.as_deref(), payload.gallery_id)
        .await?;
    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::resolve_cached_image;
    use std::path::Path;

    /// The thumbnail cache is keyed by bare file names, so anything with structure
    /// in it is a traversal attempt. `..%2Fjwt_secret.key` on the wire arrives here
    /// already decoded as `../jwt_secret.key`.
    #[test]
    fn rejects_traversal_out_of_the_cache() {
        let root = Path::new("/var/lib/vortex/thumbnails");
        for name in [
            "../jwt_secret.key",
            "../../etc/passwd",
            "sub/dir.jpg",
            "/etc/passwd",
            "..",
            "",
        ] {
            assert!(
                resolve_cached_image(root, name).is_err(),
                "expected {name:?} to be rejected"
            );
        }
    }

    #[test]
    fn accepts_a_plain_cache_filename() {
        let root = Path::new("/var/lib/vortex/thumbnails");
        assert_eq!(
            resolve_cached_image(root, "12.jpg").unwrap(),
            root.join("12.jpg")
        );
    }
}

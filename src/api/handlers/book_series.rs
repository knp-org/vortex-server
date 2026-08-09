use axum::{
    extract::{Path, State},
    Extension, Json,
};
use sqlx::SqlitePool;

use crate::api::dtos::responses::{BookDetail, BookSeriesDetail};
use crate::api::middleware::AuthUser;
use crate::error::AppError;
use crate::models::metadata::MetadataPatch;
use crate::services::catalog_service::CatalogService;
use crate::services::media_service::MediaService;

pub async fn get_book_series_detail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<BookSeriesDetail>, AppError> {
    let service = MediaService::new(pool);
    let detail = service.book_series_detail(id).await?;
    Ok(Json(detail))
}

pub async fn get_book_series_chapters(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<BookDetail>>, AppError> {
    let service = MediaService::new(pool);
    let chapters = service.book_series_chapters(id).await?;
    Ok(Json(chapters))
}

/// Save a hand-authored edit to a book series (name/plot/artwork/rating).
pub async fn update_book_series_metadata(
    Extension(auth_user): Extension<AuthUser>,
    State(pool): State<SqlitePool>,
    Path(id): Path<i64>,
    Json(patch): Json<MetadataPatch>,
) -> Result<Json<BookSeriesDetail>, AppError> {
    auth_user.require_admin()?;
    if patch.is_empty() {
        return Err(AppError::BadRequest("No metadata fields to update".into()));
    }
    CatalogService::new(pool.clone()).patch_book_series(id, &patch).await?;
    Ok(Json(MediaService::new(pool).book_series_detail(id).await?))
}

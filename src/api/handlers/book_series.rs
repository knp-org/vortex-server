use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;

use crate::api::dtos::responses::{BookDetail, BookSeriesDetail};
use crate::error::AppError;
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

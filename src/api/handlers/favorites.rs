//! Per-user favorites over the `user_favorites` table.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::api::middleware::AuthUser;
use crate::api::dtos::responses::Card;
use crate::services::favorites_service::FavoritesService;

pub async fn list_favorites(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Card>>, AppError> {
    let cards = FavoritesService::new(pool).list(user.id).await?;
    Ok(Json(cards))
}

pub async fn add_favorite(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    FavoritesService::new(pool).add(user.id, item_id).await?;
    Ok(StatusCode::OK)
}

pub async fn remove_favorite(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    FavoritesService::new(pool).remove(user.id, item_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

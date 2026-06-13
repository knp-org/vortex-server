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

pub async fn list_favorites(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Card>>, AppError> {
    // Favorites can point at any file-backed item; surface a card per type.
    let cards = sqlx::query_as::<_, Card>(
        "SELECT mi.id,
                mi.item_type AS kind,
                COALESCE(mv.title, e.title, b.title, mvd.title) AS title,
                COALESCE(mv.poster_url, e.still_url, b.poster_url, mvd.poster_url) AS poster_url,
                COALESCE(mv.year, mvd.year) AS year,
                CASE WHEN mi.item_type IN ('movie','episode','music_video')
                     THEN ('/api/v1/stream/' || mi.id) END AS stream_url
         FROM user_favorites f
         JOIN media_items mi ON mi.id = f.item_id
         LEFT JOIN movies mv ON mv.item_id = mi.id
         LEFT JOIN episodes e ON e.item_id = mi.id
         LEFT JOIN books b ON b.item_id = mi.id
         LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
         WHERE f.user_id = ?
         ORDER BY f.created_at DESC"
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await?;
    Ok(Json(cards))
}

pub async fn add_favorite(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    sqlx::query("INSERT OR IGNORE INTO user_favorites (user_id, item_id) VALUES (?, ?)")
        .bind(user.id)
        .bind(item_id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::OK)
}

pub async fn remove_favorite(
    Path(item_id): Path<i64>,
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM user_favorites WHERE user_id = ? AND item_id = ?")
        .bind(user.id)
        .bind(item_id)
        .execute(&pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

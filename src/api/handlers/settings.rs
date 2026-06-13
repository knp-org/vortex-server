use axum::{
    extract::State,
    http::StatusCode,
    Extension,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::db::models::Setting;
use crate::api::middleware::AuthUser;

#[derive(serde::Deserialize)]
pub struct UpdateSettingRequest {
    key: String,
    value: String,
}

// ---------------------------------------------------------------------------
// Global, server-wide settings (provider keys, scan config). Admin-facing.
// ---------------------------------------------------------------------------

pub async fn get_settings(State(pool): State<SqlitePool>) -> Result<Json<Vec<Setting>>, AppError> {
    let settings = sqlx::query_as::<_, Setting>("SELECT * FROM settings")
        .fetch_all(&pool)
        .await?;
    Ok(Json(settings))
}

pub async fn update_setting(
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateSettingRequest>,
) -> Result<StatusCode, AppError> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(&payload.key)
        .bind(&payload.value)
        .execute(&pool)
        .await?;

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Per-user settings (theme, playback/reading preferences). Scoped to the caller.
// ---------------------------------------------------------------------------

pub async fn get_user_settings(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Setting>>, AppError> {
    let settings = sqlx::query_as::<_, Setting>("SELECT key, value FROM user_settings WHERE user_id = ?")
        .bind(user.id)
        .fetch_all(&pool)
        .await?;
    Ok(Json(settings))
}

pub async fn update_user_setting(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<UpdateSettingRequest>,
) -> Result<StatusCode, AppError> {
    sqlx::query(
        "INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?)
         ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value"
    )
        .bind(user.id)
        .bind(&payload.key)
        .bind(&payload.value)
        .execute(&pool)
        .await?;

    Ok(StatusCode::OK)
}

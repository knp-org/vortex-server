use axum::{
    extract::State,
    http::StatusCode,
    Extension,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::settings::Setting;
use crate::api::middleware::AuthUser;
use crate::services::settings_service::SettingsService;

#[derive(serde::Deserialize)]
pub struct UpdateSettingRequest {
    key: String,
    value: String,
}

// ---------------------------------------------------------------------------
// Global, server-wide settings (provider keys, scan config). Admin-facing.
// ---------------------------------------------------------------------------

pub async fn get_settings(State(pool): State<SqlitePool>) -> Result<Json<Vec<Setting>>, AppError> {
    Ok(Json(SettingsService::new(pool).list_global().await?))
}

pub async fn update_setting(
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateSettingRequest>,
) -> Result<StatusCode, AppError> {
    SettingsService::new(pool).upsert_global(&payload.key, &payload.value).await?;
    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// Per-user settings (theme, playback/reading preferences). Scoped to the caller.
// ---------------------------------------------------------------------------

pub async fn get_user_settings(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<Setting>>, AppError> {
    Ok(Json(SettingsService::new(pool).list_for_user(user.id).await?))
}

pub async fn update_user_setting(
    State(pool): State<SqlitePool>,
    Extension(user): Extension<AuthUser>,
    Json(payload): Json<UpdateSettingRequest>,
) -> Result<StatusCode, AppError> {
    SettingsService::new(pool).upsert_for_user(user.id, &payload.key, &payload.value).await?;
    Ok(StatusCode::OK)
}

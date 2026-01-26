use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::db::models::Setting;

#[derive(serde::Deserialize)]
pub struct UpdateSettingRequest {
    key: String,
    value: String,
}

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
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = ?")
        .bind(&payload.key)
        .bind(&payload.value)
        .bind(&payload.value)
        .execute(&pool)
        .await?;

    Ok(StatusCode::OK)
}

pub async fn reset_database(State(pool): State<SqlitePool>) -> Result<StatusCode, AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM playback_progress").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM media").execute(&mut *tx).await?;
    sqlx::query("DELETE FROM libraries").execute(&mut *tx).await?;
    tx.commit().await?;
    
    Ok(StatusCode::OK)
}

//! Settings Service
//!
//! Owns reads/writes for global server settings (`settings`) and per-user
//! settings (`user_settings`).

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::settings::Setting;

pub struct SettingsService {
    pool: SqlitePool,
}

impl SettingsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_global(&self) -> Result<Vec<Setting>, AppError> {
        Ok(sqlx::query_as::<_, Setting>("SELECT * FROM settings")
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn upsert_global(&self, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn list_for_user(&self, user_id: i64) -> Result<Vec<Setting>, AppError> {
        Ok(sqlx::query_as::<_, Setting>("SELECT key, value FROM user_settings WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn upsert_for_user(&self, user_id: i64, key: &str, value: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?)
             ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value"
        )
            .bind(user_id)
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

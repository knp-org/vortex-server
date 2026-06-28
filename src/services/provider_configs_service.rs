//! Provider Configs Service
//!
//! Owns reads/writes for the `provider_configs` table. Secret masking, config
//! merging and provider construction stay in the handler/registry layer.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::provider_configs::ProviderConfig;

pub struct ProviderConfigsService {
    pool: SqlitePool,
}

impl ProviderConfigsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// All stored provider configs (callers join these against the registry).
    pub async fn list_all(&self) -> Result<Vec<ProviderConfig>, AppError> {
        Ok(sqlx::query_as::<_, ProviderConfig>(
            "SELECT provider_id, enabled, priority, config_json FROM provider_configs"
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get(&self, provider_id: &str) -> Result<Option<ProviderConfig>, AppError> {
        Ok(sqlx::query_as::<_, ProviderConfig>(
            "SELECT provider_id, enabled, priority, config_json FROM provider_configs WHERE provider_id = ?"
        )
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Upsert the stored config JSON, leaving enabled/priority untouched on conflict.
    pub async fn upsert_config(&self, provider_id: &str, enabled: bool, priority: i32, config_json: &str) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, ?, ?, ?)
             ON CONFLICT(provider_id) DO UPDATE SET config_json = ?"
        )
        .bind(provider_id)
        .bind(enabled)
        .bind(priority)
        .bind(config_json)
        .bind(config_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Enable/disable a provider, creating a default row if none exists.
    pub async fn set_enabled(&self, provider_id: &str, enabled: bool) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, ?, 100, '{}')
             ON CONFLICT(provider_id) DO UPDATE SET enabled = ?"
        )
        .bind(provider_id)
        .bind(enabled)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Set a provider's priority, creating an enabled default row if none exists.
    pub async fn set_priority(&self, provider_id: &str, priority: i32) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, 1, ?, '{}')
             ON CONFLICT(provider_id) DO UPDATE SET priority = ?"
        )
        .bind(provider_id)
        .bind(priority)
        .bind(priority)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

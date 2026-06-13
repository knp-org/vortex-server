//! Providers API Handler
//!
//! Admin endpoints for listing, configuring, enabling/disabling, reordering,
//! and testing metadata providers.

use axum::{
    extract::{Path, State},
    Json,
};
use sqlx::SqlitePool;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use crate::metadata_providers::manifest::{FieldType, ProviderManifest};
use crate::metadata_providers::registry;
use crate::models::db::provider_configs::ProviderConfig;

// ── Response types ─────────────────────────────────────────────────────

/// A provider as returned by `GET /providers` — combines registry info with DB state.
#[derive(Serialize)]
pub struct ProviderInfo {
    /// Static manifest from the registry
    #[serde(flatten)]
    pub manifest: &'static ProviderManifest,
    /// Whether this provider is currently enabled
    pub enabled: bool,
    /// Priority (lower = tried first)
    pub priority: i32,
    /// Whether config has been saved (i.e. row exists in provider_configs)
    pub configured: bool,
}

/// The config response masks secret field values.
#[derive(Serialize)]
pub struct ProviderConfigResponse {
    pub provider_id: String,
    pub enabled: bool,
    pub priority: i32,
    pub config: serde_json::Value,
}

/// Result of a provider test.
#[derive(Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
}

// ── Request types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub config: serde_json::Value,
}

#[derive(Deserialize)]
pub struct ToggleRequest {
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct ReorderRequest {
    /// Ordered list of provider IDs — first gets priority 10, second 20, etc.
    pub order: Vec<String>,
}

// ── Handlers ───────────────────────────────────────────────────────────

/// `GET /api/v1/providers`
/// List all registry providers with their manifest + current enabled/priority.
pub async fn list_providers(
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<ProviderInfo>>, AppError> {
    let configs: Vec<ProviderConfig> = sqlx::query_as(
        "SELECT provider_id, enabled, priority, config_json FROM provider_configs"
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let mut result = Vec::new();

    for entry in registry::registry() {
        let db_config = configs.iter().find(|c| c.provider_id == entry.manifest.id);

        result.push(ProviderInfo {
            manifest: &entry.manifest,
            enabled: db_config.map(|c| c.enabled).unwrap_or(false),
            priority: db_config.map(|c| c.priority).unwrap_or(100),
            configured: db_config.is_some(),
        });
    }

    // Sort by priority
    result.sort_by_key(|p| p.priority);

    Ok(Json(result))
}

/// `GET /api/v1/providers/:id/config`
/// Get current config for a provider. Secret fields are masked.
pub async fn get_provider_config(
    Path(id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<ProviderConfigResponse>, AppError> {
    let manifest = registry::manifest(&id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown provider: {}", id)))?;

    let config: Option<ProviderConfig> = sqlx::query_as(
        "SELECT provider_id, enabled, priority, config_json FROM provider_configs WHERE provider_id = ?"
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await?;

    let (enabled, priority, config_json) = match config {
        Some(c) => {
            let json: serde_json::Value = serde_json::from_str(&c.config_json)
                .unwrap_or(serde_json::json!({}));
            (c.enabled, c.priority, json)
        }
        None => (false, 100, serde_json::json!({})),
    };

    // Mask secret fields
    let mut masked = config_json.clone();
    if let Some(obj) = masked.as_object_mut() {
        for field in &manifest.config_schema {
            if field.field_type == FieldType::Secret {
                if let Some(val) = obj.get(field.key) {
                    if let Some(s) = val.as_str() {
                        if !s.is_empty() {
                            obj.insert(field.key.to_string(), serde_json::json!("••••••••"));
                        }
                    }
                }
            }
        }
    }

    Ok(Json(ProviderConfigResponse {
        provider_id: id,
        enabled,
        priority,
        config: masked,
    }))
}

/// `PUT /api/v1/providers/:id/config`
/// Update config for a provider. Merge semantics: masked/empty secrets are preserved.
pub async fn update_provider_config(
    Path(id): Path<String>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateConfigRequest>,
) -> Result<Json<ProviderConfigResponse>, AppError> {
    let manifest = registry::manifest(&id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown provider: {}", id)))?;

    // Load existing config to merge secrets
    let existing: Option<ProviderConfig> = sqlx::query_as(
        "SELECT provider_id, enabled, priority, config_json FROM provider_configs WHERE provider_id = ?"
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await?;

    let existing_json: serde_json::Value = existing.as_ref()
        .and_then(|c| serde_json::from_str(&c.config_json).ok())
        .unwrap_or(serde_json::json!({}));

    // Merge: for secret fields, if the new value is the mask or empty, keep the old value
    let mut merged = payload.config.clone();
    if let Some(obj) = merged.as_object_mut() {
        for field in &manifest.config_schema {
            if field.field_type == FieldType::Secret {
                let new_val = obj.get(field.key).and_then(|v| v.as_str()).unwrap_or("");
                if new_val.is_empty() || new_val == "••••••••" {
                    // Preserve existing secret
                    if let Some(old) = existing_json.get(field.key) {
                        obj.insert(field.key.to_string(), old.clone());
                    }
                }
            }
        }
    }

    let config_str = serde_json::to_string(&merged)
        .map_err(|e| AppError::BadRequest(format!("Invalid config JSON: {}", e)))?;

    let priority = existing.as_ref().map(|c| c.priority).unwrap_or(100);
    let enabled = existing.as_ref().map(|c| c.enabled).unwrap_or(true);

    sqlx::query(
        "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, ?, ?, ?)
         ON CONFLICT(provider_id) DO UPDATE SET config_json = ?"
    )
    .bind(&id)
    .bind(enabled)
    .bind(priority)
    .bind(&config_str)
    .bind(&config_str)
    .execute(&pool)
    .await?;

    // Return the masked version
    get_provider_config(Path(id), State(pool)).await
}

/// `POST /api/v1/providers/:id/toggle`
/// Enable or disable a provider.
pub async fn toggle_provider(
    Path(id): Path<String>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<ToggleRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let _ = registry::manifest(&id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown provider: {}", id)))?;

    // Ensure the provider has a row in the DB
    sqlx::query(
        "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, ?, 100, '{}')
         ON CONFLICT(provider_id) DO UPDATE SET enabled = ?"
    )
    .bind(&id)
    .bind(payload.enabled)
    .bind(payload.enabled)
    .execute(&pool)
    .await?;

    Ok(Json(serde_json::json!({
        "provider_id": id,
        "enabled": payload.enabled,
    })))
}

/// `PUT /api/v1/providers/order`
/// Reorder provider priorities. First in the list gets priority 10, second 20, etc.
pub async fn reorder_providers(
    State(pool): State<SqlitePool>,
    Json(payload): Json<ReorderRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    for (i, provider_id) in payload.order.iter().enumerate() {
        let priority = ((i + 1) * 10) as i32;

        sqlx::query(
            "INSERT INTO provider_configs (provider_id, enabled, priority, config_json) VALUES (?, 1, ?, '{}')
             ON CONFLICT(provider_id) DO UPDATE SET priority = ?"
        )
        .bind(provider_id)
        .bind(priority)
        .bind(priority)
        .execute(&pool)
        .await?;
    }

    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// `POST /api/v1/providers/:id/test`
/// Build the provider from its stored config and run health_check().
pub async fn test_provider(
    Path(id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<TestResult>, AppError> {
    let _ = registry::manifest(&id)
        .ok_or_else(|| AppError::NotFound(format!("Unknown provider: {}", id)))?;

    let factory = registry::factory(&id)
        .ok_or_else(|| AppError::NotFound(format!("No factory for provider: {}", id)))?;

    // Load config from DB
    let config: Option<ProviderConfig> = sqlx::query_as(
        "SELECT provider_id, enabled, priority, config_json FROM provider_configs WHERE provider_id = ?"
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await?;

    let config_json: serde_json::Value = config
        .and_then(|c| serde_json::from_str(&c.config_json).ok())
        .unwrap_or(serde_json::json!({}));

    // Build the provider
    let provider = match factory(&config_json) {
        Ok(p) => p,
        Err(e) => {
            return Ok(Json(TestResult {
                success: false,
                message: format!("Failed to initialize provider: {}", e),
            }));
        }
    };

    // Run health check
    match provider.health_check().await {
        Ok(()) => Ok(Json(TestResult {
            success: true,
            message: "Connection successful!".to_string(),
        })),
        Err(e) => Ok(Json(TestResult {
            success: false,
            message: format!("{}", e),
        })),
    }
}

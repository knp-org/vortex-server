//! Provider configuration DB models
//!
//! Maps to the `provider_configs` and `library_providers` tables.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A row in the `provider_configs` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProviderConfig {
    pub provider_id: String,
    pub enabled: bool,
    pub priority: i32,
    pub config_json: String,
}

/// A row in the `library_providers` table (per-library override).
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryProvider {
    pub library_id: i64,
    pub provider_id: String,
    pub priority: i32,
    pub enabled: bool,
}

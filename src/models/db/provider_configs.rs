//! `provider_configs` table model.

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

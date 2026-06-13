//! `library_providers` table model (per-library provider override).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A row in the `library_providers` table.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LibraryProvider {
    pub library_id: i64,
    pub provider_id: String,
    pub priority: i32,
    pub enabled: bool,
}

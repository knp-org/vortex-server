use sqlx::FromRow;
use serde::{Deserialize, Serialize};

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

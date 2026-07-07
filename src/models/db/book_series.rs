use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BookSeries {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub rating: Option<f64>,
}

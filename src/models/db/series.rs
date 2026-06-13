//! `series` grouping entity (a TV show; not a file, so not in `media_items`).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Series {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
    pub studio_id: Option<i64>,
    pub trailer_url: Option<String>,
    pub collection_name: Option<String>,
    pub origin_country: Option<String>,
    pub creator: Option<String>,
    pub provider_ids: Option<String>,
}

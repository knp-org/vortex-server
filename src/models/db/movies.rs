//! `movies` detail row (1:1 with a `media_items` spine row of type `movie`).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Movie {
    pub item_id: i64,
    pub title: Option<String>,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i64>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
    pub studio_id: Option<i64>,
    pub collection_name: Option<String>,
    pub origin_country: Option<String>,
    pub creator: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub trailer_url: Option<String>,
    pub provider_ids: Option<String>,
}

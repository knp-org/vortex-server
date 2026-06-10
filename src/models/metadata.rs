use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormalizedMetadata {
    pub title: String,
    pub year: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub media_type: Option<String>, // "movie", "series"
    pub provider_ids: Option<serde_json::Value>,
    pub genres: Option<Vec<String>>,
    pub runtime: Option<i32>,
    pub rating: Option<f32>,
    pub cast: Option<Vec<CastMember>>,
    pub director: Option<Vec<String>>,
    pub tagline: Option<String>,
    pub status: Option<String>,
    pub original_language: Option<String>,
    pub popularity: Option<f32>,
    pub budget: Option<i64>,
    pub revenue: Option<i64>,
    pub homepage: Option<String>,
    pub imdb_id: Option<String>,
    pub age_rating: Option<String>,
    pub studio: Option<String>,
    pub trailer_url: Option<String>,
    pub origin_country: Option<String>,
    pub collection_name: Option<String>,
    pub creator: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CastMember {
    pub name: String,
    pub character: String,
    pub role: String, // "actor" or "job" (Director/etc)
    pub profile_url: Option<String>,
    pub order: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpisodeMetadata {
    pub id: String, // Provider specific ID (or generic ID string)
    pub episode_number: i32,
    pub season_number: i32,
    pub name: String,
    pub overview: String,
    pub still_path: Option<String>,
    pub air_date: Option<String>,
}

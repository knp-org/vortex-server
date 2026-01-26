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

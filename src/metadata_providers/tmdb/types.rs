//! TMDB API Response Types
//!
//! These types are specific to the TMDB API response format.

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct TmdbCredits {
    pub cast: Vec<TmdbCast>,
    pub crew: Vec<TmdbCrew>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TmdbCrew {
    pub name: String,
    pub job: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TmdbCast {
    pub name: String,
    pub character: Option<String>,
    pub profile_path: Option<String>,
    pub order: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbFullResponse {
    pub runtime: Option<i32>,
    pub episode_run_time: Option<Vec<i32>>,
    pub genres: Option<Vec<TmdbGenre>>,
    pub vote_average: Option<f32>,
    pub tagline: Option<String>,
    pub status: Option<String>,
    pub original_language: Option<String>,
    pub popularity: Option<f32>,
    pub budget: Option<i64>,
    pub revenue: Option<i64>,
    pub homepage: Option<String>,
    pub imdb_id: Option<String>,
    pub created_by: Option<Vec<TmdbCreator>>,
    pub credits: Option<TmdbCredits>,
    pub aggregate_credits: Option<TmdbAggregateCredits>, // For TV shows
}

#[derive(Deserialize, Debug)]
pub struct TmdbCreator {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbAggregateCredits {
    pub cast: Vec<TmdbCast>,
    pub crew: Vec<TmdbAggregateCrew>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbAggregateCrew {
    pub name: String,
    pub jobs: Vec<TmdbJob>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbJob {
    pub job: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbGenre {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbResponse {
    pub results: Vec<TmdbResult>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbResult {
    pub id: i64,
    #[serde(alias = "title", alias = "name")]
    pub title: String,
    #[serde(alias = "overview")]
    pub overview: Option<String>,
    #[serde(alias = "poster_path")]
    pub poster_path: Option<String>,
    #[serde(alias = "backdrop_path")]
    pub backdrop_path: Option<String>,
    #[serde(alias = "release_date", alias = "first_air_date")]
    pub date: Option<String>,
    pub vote_average: Option<f32>,
    pub media_type: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbSeasonResponse {
    pub episodes: Vec<TmdbEpisode>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbEpisode {
    pub episode_number: i32,
    pub name: String,
    pub overview: String,
    pub still_path: Option<String>,
}

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
    pub production_companies: Option<Vec<TmdbCompany>>,
    pub networks: Option<Vec<TmdbCompany>>,
    pub belongs_to_collection: Option<TmdbCollection>,
    pub origin_country: Option<Vec<String>>,
    pub content_ratings: Option<TmdbContentRatings>,
    pub release_dates: Option<TmdbReleaseDates>,
    pub videos: Option<TmdbVideos>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbCompany {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbCollection {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbVideos {
    pub results: Vec<TmdbVideo>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbVideo {
    #[serde(alias = "type")]
    pub video_type: String,
    pub key: String,
    pub site: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbContentRatings {
    pub results: Vec<TmdbContentRatingResult>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbContentRatingResult {
    pub iso_3166_1: String,
    pub rating: String,
}

#[derive(Deserialize, Debug)]
pub struct TmdbReleaseDates {
    pub results: Vec<TmdbReleaseDateResult>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbReleaseDateResult {
    pub iso_3166_1: String,
    pub release_dates: Vec<TmdbReleaseDateItem>,
}

#[derive(Deserialize, Debug)]
pub struct TmdbReleaseDateItem {
    pub certification: String,
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

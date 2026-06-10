use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SeriesDto {
    pub name: String,
    pub poster_url: Option<String>,
    pub season_count: i32,
}

#[derive(Debug, Serialize, Clone)]
pub struct SeasonDto {
    pub season_number: i32,
    pub episode_count: i32,
    pub poster_url: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct EpisodeDto {
    pub id: i64,
    pub title: Option<String>,
    pub episode_number: i32,
    pub poster_url: Option<String>,
    pub file_path: String,
    pub plot: Option<String>,
    pub runtime: Option<i32>,
    pub rating: Option<f32>,
    pub cast: Option<String>,
    pub director: Option<String>,
    pub age_rating: Option<String>,
    pub studio: Option<String>,
    pub trailer_url: Option<String>,
    pub origin_country: Option<String>,
    pub collection_name: Option<String>,
    pub creator: Option<String>,
    pub tags: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SeriesDetailDto {
    pub name: String,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub plot: Option<String>,
    pub year: Option<i64>,
    pub genres: Option<String>,
    pub cast: Option<String>,
    pub director: Option<String>,
    pub age_rating: Option<String>,
    pub studio: Option<String>,
    pub trailer_url: Option<String>,
    pub origin_country: Option<String>,
    pub collection_name: Option<String>,
    pub creator: Option<String>,
    pub tags: Option<String>,
    pub seasons: Vec<SeasonDto>,
}

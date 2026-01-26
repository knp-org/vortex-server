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
}

#[derive(Debug, Serialize, Clone)]
pub struct SeriesDetailDto {
    pub name: String,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub plot: Option<String>,
    pub year: Option<i64>,
    pub genres: Option<String>,
    pub seasons: Vec<SeasonDto>,
}

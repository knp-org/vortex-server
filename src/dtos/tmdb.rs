use serde::Deserialize;


#[derive(Deserialize, Debug)]
pub struct TmdbFullResponse {
    pub runtime: Option<i32>,
    pub episode_run_time: Option<Vec<i32>>,
    pub genres: Option<Vec<TmdbGenre>>,
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
}

// Public DTO for API responses

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

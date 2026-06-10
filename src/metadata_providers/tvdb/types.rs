use serde::Deserialize;

/// Response from /series/{id}/translations/{lang} or /movies/{id}/translations/{lang}
#[derive(Debug, Deserialize)]
pub struct TvdbTranslationResponse {
    pub data: Option<TvdbTranslationData>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbTranslationData {
    pub name: Option<String>,
    pub overview: Option<String>,
    #[allow(dead_code)]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbLoginResponse {
    pub data: TvdbLoginData,
}

#[derive(Debug, Deserialize)]
pub struct TvdbLoginData {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbSearchResponse {
    pub data: Option<Vec<TvdbSearchResult>>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbSearchResult {
    pub tvdb_id: String,
    pub name: Option<String>,
    pub year: Option<String>,
    pub image_url: Option<String>,
    pub overview: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbSeriesResponse {
    pub data: TvdbSeriesData,
}

#[derive(Debug, Deserialize)]
pub struct TvdbSeriesData {
    pub id: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "firstAired")]
    pub first_aired: Option<String>,
    pub characters: Option<Vec<TvdbCharacter>>,
    pub artworks: Option<Vec<TvdbArtwork>>,
    #[serde(rename = "contentRatings")]
    pub content_ratings: Option<Vec<TvdbContentRating>>,
    pub companies: Option<Vec<TvdbCompany>>,
    pub trailers: Option<Vec<TvdbTrailer>>,
    pub tags: Option<Vec<TvdbTag>>,
    pub genres: Option<Vec<TvdbGenre>>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbGenre {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbContentRating {
    pub name: String,
    pub country: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbCompany {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbTrailer {
    pub name: String,
    pub url: String,
    pub language: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbTag {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct TvdbArtwork {
    #[allow(dead_code)]
    pub id: i64,
    pub image: Option<String>,
    #[serde(rename = "type")]
    pub artwork_type: i32,
}

#[derive(Debug, Deserialize)]
pub struct TvdbCharacter {
    #[allow(dead_code)]
    pub id: i64,
    pub name: Option<String>,
    #[serde(alias = "peopleName", alias = "personName")]
    pub people_name: Option<String>,
    pub image: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbEpisodesResponse {
    pub data: TvdbEpisodesData,
}

#[derive(Debug, Deserialize)]
pub struct TvdbEpisodesData {
    pub episodes: Option<Vec<TvdbEpisode>>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbEpisode {
    pub id: i64,
    #[allow(dead_code)]
    #[serde(rename = "seriesId")]
    pub series_id: i64,
    pub name: Option<String>,
    pub aired: Option<String>,
    #[allow(dead_code)]
    pub runtime: Option<i32>,
    pub overview: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "seasonNumber")]
    pub season_number: i32,
    pub number: i32,
}

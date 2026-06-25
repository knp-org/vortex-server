use serde::{Deserialize, Deserializer};

/// Deserialize a field but tolerate a mismatched shape. TVDB's `movies/{id}/extended`
/// and `series/{id}/extended` responses differ (e.g. a field that's an array for one
/// is an object/null for the other); without this, one odd field would fail the whole
/// parse and 502 the request. On any mismatch we yield `None` instead of erroring.
fn lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(T::deserialize(v).ok())
}

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

// NOTE: every nested field below is tolerant (Option / default). The `movies/{id}/extended`
// and `series/{id}/extended` responses differ subtly, and TVDB frequently returns null or
// omits fields; a single strict field would abort the whole parse and 502 the request.
#[derive(Debug, Deserialize)]
pub struct TvdbSeriesData {
    pub id: i64,
    pub name: Option<String>,
    pub overview: Option<String>,
    pub image: Option<String>,
    #[serde(rename = "firstAired")]
    pub first_aired: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    pub characters: Option<Vec<TvdbCharacter>>,
    #[serde(default, deserialize_with = "lenient")]
    pub artworks: Option<Vec<TvdbArtwork>>,
    #[serde(rename = "contentRatings", default, deserialize_with = "lenient")]
    pub content_ratings: Option<Vec<TvdbContentRating>>,
    #[serde(default, deserialize_with = "lenient")]
    pub companies: Option<Vec<TvdbCompany>>,
    #[serde(default, deserialize_with = "lenient")]
    pub trailers: Option<Vec<TvdbTrailer>>,
    #[serde(default, deserialize_with = "lenient")]
    pub tags: Option<Vec<TvdbTag>>,
    #[serde(default, deserialize_with = "lenient")]
    pub genres: Option<Vec<TvdbGenre>>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbGenre {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbContentRating {
    pub name: Option<String>,
    pub country: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbCompany {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbTrailer {
    #[allow(dead_code)]
    pub name: Option<String>,
    pub url: Option<String>,
    pub language: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbTag {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TvdbArtwork {
    #[allow(dead_code)]
    pub id: i64,
    pub image: Option<String>,
    #[serde(rename = "type", default)]
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
    /// Distinguishes role type: "Actor", "Director", "Writer", "Producer", etc.
    #[serde(alias = "peopleType", default)]
    pub people_type: Option<String>,
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

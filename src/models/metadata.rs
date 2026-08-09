use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

// ── Manual (user) metadata edits ────────────────────────────────────────────

/// A field in a [`MetadataPatch`]: absent = leave unchanged, `null` = clear,
/// value = set. `Option<Option<T>>` distinguishes the first two, which a plain
/// `Option<T>` cannot (serde maps both a missing key and `null` to `None`).
pub type Patch<T> = Option<Option<T>>;

/// Deserializes any present value — including `null` — into `Some(...)`, so an
/// absent key stays `None` via `#[serde(default)]`.
fn present<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A user-authored metadata edit. Every field is a [`Patch`], so a client sends
/// only what it changed. One flat shape covers all entity types; each entity
/// applies the subset of fields it owns and ignores the rest.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MetadataPatch {
    // Shared
    #[serde(deserialize_with = "present")] pub title: Patch<String>,
    #[serde(deserialize_with = "present")] pub original_title: Patch<String>,
    #[serde(deserialize_with = "present")] pub year: Patch<i64>,
    #[serde(deserialize_with = "present")] pub plot: Patch<String>,
    #[serde(deserialize_with = "present")] pub tagline: Patch<String>,
    #[serde(deserialize_with = "present")] pub runtime: Patch<i64>,
    #[serde(deserialize_with = "present")] pub rating: Patch<f64>,
    #[serde(deserialize_with = "present")] pub age_rating: Patch<String>,
    /// Studio *name*; resolved to a `studios` row on write.
    #[serde(deserialize_with = "present")] pub studio: Patch<String>,
    #[serde(deserialize_with = "present")] pub collection_name: Patch<String>,
    #[serde(deserialize_with = "present")] pub origin_country: Patch<String>,
    /// Comma-separated creators/directors, as stored.
    #[serde(deserialize_with = "present")] pub creator: Patch<String>,
    #[serde(deserialize_with = "present")] pub poster_url: Patch<String>,
    #[serde(deserialize_with = "present")] pub backdrop_url: Patch<String>,
    #[serde(deserialize_with = "present")] pub trailer_url: Patch<String>,

    /// Full replacement lists for the normalized link tables.
    pub genres: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,

    // Books
    #[serde(deserialize_with = "present")] pub publisher: Patch<String>,
    #[serde(deserialize_with = "present")] pub published_date: Patch<String>,
    #[serde(deserialize_with = "present")] pub isbn: Patch<String>,
    #[serde(deserialize_with = "present")] pub page_count: Patch<i64>,

    // Episodes
    #[serde(deserialize_with = "present")] pub episode_number: Patch<i64>,
    #[serde(deserialize_with = "present")] pub air_date: Patch<String>,
    #[serde(deserialize_with = "present")] pub still_url: Patch<String>,

    /// When set, flips the "protect from automatic refresh" flag.
    pub metadata_locked: Option<bool>,
}

impl MetadataPatch {
    /// True when the patch carries nothing to write.
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.original_title.is_none() && self.year.is_none()
            && self.plot.is_none() && self.tagline.is_none() && self.runtime.is_none()
            && self.rating.is_none() && self.age_rating.is_none() && self.studio.is_none()
            && self.collection_name.is_none() && self.origin_country.is_none()
            && self.creator.is_none() && self.poster_url.is_none() && self.backdrop_url.is_none()
            && self.trailer_url.is_none() && self.genres.is_none() && self.tags.is_none()
            && self.publisher.is_none() && self.published_date.is_none() && self.isbn.is_none()
            && self.page_count.is_none() && self.episode_number.is_none() && self.air_date.is_none()
            && self.still_url.is_none() && self.metadata_locked.is_none()
    }
}

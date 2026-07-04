//! Response DTOs for the redesigned, per-type catalog API.

use serde::Serialize;
use sqlx::FromRow;

/// A lightweight card for grid/listing/search/recent views. `kind` tells the client
/// which detail endpoint to call and how to interpret `id`:
/// - `movie` / `episode` / `book` / `music_video`: `id` is a `media_items.id`.
/// - `series`: `id` is a `series.id`.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Card {
    pub id: i64,
    pub kind: String,
    pub title: Option<String>,
    pub poster_url: Option<String>,
    pub year: Option<i64>,
    #[sqlx(default)]
    pub stream_url: Option<String>,
    /// Up to a few thumbnail URLs used to render a mosaic/collage card
    /// (currently only populated for gallery cards). Empty for other kinds.
    #[sqlx(skip)]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thumbs: Vec<String>,
}

/// A person credited on an item or series.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CreditDto {
    pub name: String,
    pub character: Option<String>,
    pub role: Option<String>,
    pub profile_url: Option<String>,
    pub ord: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MovieDetail {
    pub id: i64,
    pub title: Option<String>,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub tagline: Option<String>,
    pub runtime: Option<i64>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
    pub studio: Option<String>,
    pub collection_name: Option<String>,
    pub origin_country: Option<String>,
    pub creator: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub trailer_url: Option<String>,
    pub provider_ids: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<CreditDto>,
    pub stream_url: String,
    pub file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeasonDto {
    pub id: i64,
    pub season_number: i64,
    pub episode_count: i64,
    pub poster_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesDetail {
    pub id: i64,
    pub name: String,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
    pub studio: Option<String>,
    pub trailer_url: Option<String>,
    pub collection_name: Option<String>,
    pub origin_country: Option<String>,
    pub creator: Option<String>,
    pub provider_ids: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub cast: Vec<CreditDto>,
    pub seasons: Vec<SeasonDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpisodeDto {
    pub id: i64,
    pub series_id: Option<i64>,
    pub series_name: Option<String>,
    pub season_number: Option<i64>,
    pub episode_number: Option<i64>,
    pub title: Option<String>,
    pub plot: Option<String>,
    pub still_url: Option<String>,
    pub runtime: Option<i64>,
    pub air_date: Option<String>,
    pub stream_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrackDto {
    pub id: i64, // media_items.id
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub duration: Option<i64>,
    pub stream_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MusicVideoDetail {
    pub id: i64,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub year: Option<i64>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub runtime: Option<i64>,
    pub genres: Vec<String>,
    pub stream_url: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PlaylistDto {
    pub id: i64,
    pub name: String,
    pub track_count: i64,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistDetail {
    pub id: i64,
    pub name: String,
    pub tracks: Vec<TrackDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlbumDetail {
    pub id: i64,
    pub title: String,
    pub artist_id: Option<i64>,
    pub artist: Option<String>,
    pub year: Option<i64>,
    pub cover_url: Option<String>,
    pub tracks: Vec<TrackDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtistDetail {
    pub id: i64,
    pub name: String,
    pub bio: Option<String>,
    pub image_url: Option<String>,
    pub albums: Vec<Card>,
}

/// A photo in grid/lightbox views. `url` is the full-resolution original;
/// `thumb_url` is the server-scaled thumbnail.
#[derive(Debug, Clone, Serialize)]
pub struct ImageDto {
    pub id: i64, // media_items.id
    pub gallery_id: Option<i64>,
    pub title: Option<String>,
    pub taken_at: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub url: String,
    pub thumb_url: String,
}

/// Full metadata for a single photo (EXIF + camera/GPS), plus its serving URLs.
#[derive(Debug, Clone, Serialize)]
pub struct ImageDetail {
    pub id: i64,
    pub gallery_id: Option<i64>,
    pub title: Option<String>,
    pub taken_at: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<i64>,
    pub focal_length: Option<f64>,
    pub aperture: Option<f64>,
    pub gps_lat: Option<f64>,
    pub gps_lon: Option<f64>,
    pub orientation: Option<i64>,
    pub url: String,
    pub thumb_url: String,
    pub file_name: Option<String>,
}

/// A photo album (gallery) with its photos, for the gallery detail view.
#[derive(Debug, Clone, Serialize)]
pub struct GalleryDetail {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub cover_url: Option<String>,
    pub taken_at: Option<String>,
    pub image_count: i64,
    pub images: Vec<ImageDto>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookDetail {
    pub id: i64,
    pub title: Option<String>,
    pub plot: Option<String>,
    pub poster_url: Option<String>,
    pub page_count: Option<i64>,
    pub reading_mode: Option<String>,
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub isbn: Option<String>,
}

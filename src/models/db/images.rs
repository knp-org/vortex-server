//! `images` table model (photo detail rows for an Images library).
//!
//! One row per photo file, joined 1:1 to its `media_items` spine row on `item_id`,
//! parallel to `movies`/`episodes`/`tracks`. Grouping into albums is via
//! `gallery_id` (see [`super::galleries`]).

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Column list for `SELECT`ing an [`Image`] joined to its spine row. Mirrors the
/// pattern used by books' `BOOK_SELECT`, so `library_id`/`file_path` come along.
pub const IMAGE_SELECT: &str = "mi.id AS item_id, mi.library_id, mi.file_path, \
    i.gallery_id, i.title, i.taken_at, i.width, i.height, i.camera_make, \
    i.camera_model, i.lens, i.iso, i.focal_length, i.aperture, i.gps_lat, \
    i.gps_lon, i.orientation";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Image {
    pub item_id: i64,
    pub library_id: i64,
    pub file_path: String,
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
}

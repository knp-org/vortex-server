//! `galleries` table model.
//!
//! A gallery is the grouping entity for an Images (photo) library — the album a
//! set of photos belongs to (typically one folder of images). It is not a
//! file-backed item, so it lives in its own table and is referenced by
//! `images.gallery_id`, parallel to `series`/`albums`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Gallery {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub description: Option<String>,
    /// Cover photo URL — usually the first/representative image's thumbnail.
    pub cover_url: Option<String>,
    /// Earliest photo capture date, used for sorting galleries.
    pub taken_at: Option<String>,
}

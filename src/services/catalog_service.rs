//! Catalog repository.
//!
//! Owns all writes to the identity spine (`media_items`), the per-type detail tables
//! (`movies`, `series`/`seasons`/`episodes`, ...) and the normalized lookup/join
//! tables (`genres`, `people`, `studios`, `item_genres`, `credits`, ...). The scanner
//! and metadata-refresh handlers go through here instead of writing raw SQL inline.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::metadata::NormalizedMetadata;

pub struct CatalogService {
    pool: SqlitePool,
}

impl CatalogService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a spine row for `file_path` if absent, otherwise refresh its library/type.
    /// Returns the `media_items.id` to use as the item id everywhere downstream.
    pub async fn upsert_item(
        &self,
        library_id: i64,
        item_type: &str,
        file_path: &str,
    ) -> Result<i64, AppError> {
        let pool = &self.pool;
        if let Some((id,)) = sqlx::query_as::<_, (i64,)>("SELECT id FROM media_items WHERE file_path = ?")
            .bind(file_path)
            .fetch_optional(pool)
            .await?
        {
            sqlx::query("UPDATE media_items SET library_id = ?, item_type = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(library_id)
                .bind(item_type)
                .bind(id)
                .execute(pool)
                .await?;
            Ok(id)
        } else {
            let id = sqlx::query("INSERT INTO media_items (library_id, item_type, file_path) VALUES (?, ?, ?)")
                .bind(library_id)
                .bind(item_type)
                .bind(file_path)
                .execute(pool)
                .await?
                .last_insert_rowid();
            Ok(id)
        }
    }

    /// Upsert the `movies` detail row for `item_id` from fetched metadata, and refresh
    /// its normalized genres/tags/credits.
    pub async fn apply_movie_metadata(&self, item_id: i64, meta: &NormalizedMetadata) -> Result<(), AppError> {
        let pool = &self.pool;
        let studio_id = match &meta.studio {
            Some(s) if !s.is_empty() => Some(get_or_create_studio(pool, s).await?),
            _ => None,
        };
        let provider_ids = meta.provider_ids.as_ref().map(|v| v.to_string());

        sqlx::query(
            "INSERT INTO movies (item_id, title, year, plot, tagline, runtime, rating, age_rating, studio_id, collection_name, origin_country, creator, poster_url, backdrop_url, trailer_url, provider_ids)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(item_id) DO UPDATE SET
                title=excluded.title, year=excluded.year, plot=excluded.plot, tagline=excluded.tagline,
                runtime=excluded.runtime, rating=excluded.rating, age_rating=excluded.age_rating,
                studio_id=excluded.studio_id, collection_name=excluded.collection_name,
                origin_country=excluded.origin_country, creator=excluded.creator,
                poster_url=excluded.poster_url, backdrop_url=excluded.backdrop_url,
                trailer_url=excluded.trailer_url, provider_ids=excluded.provider_ids"
        )
        .bind(item_id)
        .bind(&meta.title)
        .bind(parse_year(meta))
        .bind(&meta.plot)
        .bind(&meta.tagline)
        .bind(meta.runtime)
        .bind(meta.rating)
        .bind(&meta.age_rating)
        .bind(studio_id)
        .bind(&meta.collection_name)
        .bind(&meta.origin_country)
        .bind(meta.creator.as_ref().map(|c| c.join(", ")))
        .bind(&meta.poster_url)
        .bind(&meta.backdrop_url)
        .bind(&meta.trailer_url)
        .bind(provider_ids)
        .execute(pool)
        .await?;

        if let Some(g) = &meta.genres { set_item_genres(pool, item_id, g).await?; }
        if let Some(t) = &meta.tags { set_item_tags(pool, item_id, t).await?; }
        set_credits(pool, Some(item_id), None, meta).await?;
        Ok(())
    }

    /// Ensure a bare `movies` row exists (title only) for items we couldn't enrich.
    pub async fn ensure_movie_stub(&self, item_id: i64, title: &str) -> Result<(), AppError> {
        sqlx::query("INSERT OR IGNORE INTO movies (item_id, title) VALUES (?, ?)")
            .bind(item_id).bind(title).execute(&self.pool).await?;
        Ok(())
    }

    /// Get-or-create a series by (library, name) and return its id.
    pub async fn get_or_create_series(&self, library_id: i64, name: &str) -> Result<i64, AppError> {
        let pool = &self.pool;
        sqlx::query("INSERT OR IGNORE INTO series (library_id, name) VALUES (?, ?)")
            .bind(library_id).bind(name).execute(pool).await?;
        let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM series WHERE library_id = ? AND name = ?")
            .bind(library_id).bind(name).fetch_one(pool).await?;
        Ok(id)
    }

    /// Apply series-level metadata + normalized genres/tags/credits.
    pub async fn apply_series_metadata(&self, series_id: i64, meta: &NormalizedMetadata) -> Result<(), AppError> {
        let pool = &self.pool;
        let provider_ids = meta.provider_ids.as_ref().map(|v| v.to_string());
        let studio_id = match &meta.studio {
            Some(s) if !s.is_empty() => Some(get_or_create_studio(pool, s).await?),
            _ => None,
        };
        sqlx::query(
            "UPDATE series SET year=?, plot=?, poster_url=?, backdrop_url=?, rating=?, age_rating=?,
                studio_id=?, trailer_url=?, collection_name=?, origin_country=?, creator=?, provider_ids=?
             WHERE id=?"
        )
        .bind(parse_year(meta))
        .bind(&meta.plot)
        .bind(&meta.poster_url)
        .bind(&meta.backdrop_url)
        .bind(meta.rating)
        .bind(&meta.age_rating)
        .bind(studio_id)
        .bind(&meta.trailer_url)
        .bind(&meta.collection_name)
        .bind(&meta.origin_country)
        .bind(meta.creator.as_ref().map(|c| c.join(", ")))
        .bind(provider_ids)
        .bind(series_id)
        .execute(pool)
        .await?;

        if let Some(g) = &meta.genres { set_series_genres(pool, series_id, g).await?; }
        if let Some(t) = &meta.tags { set_series_tags(pool, series_id, t).await?; }
        set_credits(pool, None, Some(series_id), meta).await?;
        Ok(())
    }

    /// Get-or-create a season under a series and return its id.
    pub async fn get_or_create_season(&self, series_id: i64, season_number: i64) -> Result<i64, AppError> {
        let pool = &self.pool;
        sqlx::query("INSERT OR IGNORE INTO seasons (series_id, season_number) VALUES (?, ?)")
            .bind(series_id).bind(season_number).execute(pool).await?;
        let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM seasons WHERE series_id = ? AND season_number = ?")
            .bind(series_id).bind(season_number).fetch_one(pool).await?;
        Ok(id)
    }

    /// Upsert an `episodes` row, preserving any title/plot already filled by metadata.
    pub async fn upsert_episode(
        &self,
        item_id: i64,
        season_id: i64,
        episode_number: i64,
        fallback_title: &str,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO episodes (item_id, season_id, episode_number, title)
             VALUES (?, ?, ?, ?)
             ON CONFLICT(item_id) DO UPDATE SET season_id=excluded.season_id, episode_number=excluded.episode_number"
        )
        .bind(item_id).bind(season_id).bind(episode_number).bind(fallback_title)
        .execute(&self.pool).await?;
        Ok(())
    }

    /// Fill episode-specific details fetched from a provider.
    pub async fn apply_episode_details(
        &self,
        item_id: i64,
        title: &str,
        plot: &str,
        still_url: Option<String>,
    ) -> Result<(), AppError> {
        sqlx::query("UPDATE episodes SET title=?, plot=?, still_url=COALESCE(?, still_url) WHERE item_id=?")
            .bind(title).bind(plot).bind(still_url).bind(item_id)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Upsert a `books` detail row. The spine row is created by [`upsert_item`] first.
    pub async fn upsert_book(
        &self,
        item_id: i64,
        title: &str,
        page_count: Option<i64>,
        book_series_id: Option<i64>,
        chapter_number: Option<f64>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO books (item_id, title, page_count, book_series_id, chapter_number)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(item_id) DO UPDATE SET 
             page_count = COALESCE(excluded.page_count, books.page_count),
             book_series_id = excluded.book_series_id,
             chapter_number = excluded.chapter_number"
        )
        .bind(item_id).bind(title).bind(page_count)
        .bind(book_series_id).bind(chapter_number)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get_or_create_book_series(pool: &SqlitePool, library_id: i64, name: &str) -> Result<i64, AppError> {
        sqlx::query("INSERT OR IGNORE INTO book_series (library_id, name) VALUES (?, ?)")
            .bind(library_id).bind(name)
            .execute(pool).await?;

        let id: i64 = sqlx::query_scalar("SELECT id FROM book_series WHERE library_id = ? AND name = ?")
            .bind(library_id).bind(name)
            .fetch_one(pool).await?;
        Ok(id)
    }

    /// Upsert a minimal `music_videos` row (filename title; no external metadata yet).
    pub async fn upsert_music_video(&self, item_id: i64, title: &str) -> Result<(), AppError> {
        sqlx::query("INSERT OR IGNORE INTO music_videos (item_id, title) VALUES (?, ?)")
            .bind(item_id).bind(title).execute(&self.pool).await?;
        Ok(())
    }

    // ── Music: artists → albums → tracks ───────────────────────────────────────

    /// Get-or-create an artist by (library, name).
    pub async fn get_or_create_artist(&self, library_id: i64, name: &str) -> Result<i64, AppError> {
        let pool = &self.pool;
        sqlx::query("INSERT OR IGNORE INTO artists (library_id, name) VALUES (?, ?)")
            .bind(library_id).bind(name).execute(pool).await?;
        let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM artists WHERE library_id = ? AND name = ?")
            .bind(library_id).bind(name).fetch_one(pool).await?;
        Ok(id)
    }

    /// Get-or-create an album under an artist. Fills `year` on first creation.
    pub async fn get_or_create_album(
        &self,
        artist_id: i64,
        library_id: i64,
        title: &str,
        year: Option<i64>,
    ) -> Result<i64, AppError> {
        let pool = &self.pool;
        sqlx::query("INSERT OR IGNORE INTO albums (artist_id, library_id, title, year) VALUES (?, ?, ?, ?)")
            .bind(artist_id).bind(library_id).bind(title).bind(year).execute(pool).await?;
        let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM albums WHERE artist_id = ? AND title = ?")
            .bind(artist_id).bind(title).fetch_one(pool).await?;
        Ok(id)
    }

    /// Set an album's cover URL only if it doesn't already have one.
    pub async fn set_album_cover_if_empty(&self, album_id: i64, cover_url: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE albums SET cover_url = ? WHERE id = ? AND (cover_url IS NULL OR cover_url = '')")
            .bind(cover_url).bind(album_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Upsert a `tracks` detail row. The spine row is created by [`upsert_item`] first.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_track(
        &self,
        item_id: i64,
        album_id: i64,
        artist_id: i64,
        track_number: Option<i64>,
        disc_number: Option<i64>,
        title: &str,
        duration: Option<i64>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO tracks (item_id, album_id, artist_id, track_number, disc_number, title, duration)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(item_id) DO UPDATE SET
                album_id=excluded.album_id, artist_id=excluded.artist_id,
                track_number=excluded.track_number, disc_number=excluded.disc_number,
                title=excluded.title, duration=excluded.duration"
        )
        .bind(item_id).bind(album_id).bind(artist_id)
        .bind(track_number).bind(disc_number).bind(title).bind(duration)
        .execute(&self.pool).await?;
        Ok(())
    }

    // ── Images: galleries → images ─────────────────────────────────────────────

    /// Get-or-create a gallery (photo album) by (library, name) and return its id.
    pub async fn get_or_create_gallery(&self, library_id: i64, name: &str) -> Result<i64, AppError> {
        let pool = &self.pool;
        sqlx::query("INSERT OR IGNORE INTO galleries (library_id, name) VALUES (?, ?)")
            .bind(library_id).bind(name).execute(pool).await?;
        let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM galleries WHERE library_id = ? AND name = ?")
            .bind(library_id).bind(name).fetch_one(pool).await?;
        Ok(id)
    }

    /// Set a gallery's cover URL only if it doesn't already have one.
    pub async fn set_gallery_cover_if_empty(&self, gallery_id: i64, cover_url: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE galleries SET cover_url = ? WHERE id = ? AND (cover_url IS NULL OR cover_url = '')")
            .bind(cover_url).bind(gallery_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Advance a gallery's `taken_at` back to the earliest photo date seen, so the
    /// gallery sorts by its oldest photo. No-op when `taken_at` is `None`.
    pub async fn min_gallery_taken_at(&self, gallery_id: i64, taken_at: Option<&str>) -> Result<(), AppError> {
        if let Some(t) = taken_at {
            sqlx::query("UPDATE galleries SET taken_at = ? WHERE id = ? AND (taken_at IS NULL OR taken_at > ?)")
                .bind(t).bind(gallery_id).bind(t).execute(&self.pool).await?;
        }
        Ok(())
    }

    /// Upsert an `images` detail row from extracted EXIF. The spine row is created
    /// by [`upsert_item`] first. Preserves a user-edited `title`/`gallery_id`:
    /// on conflict it refreshes the machine-derived EXIF columns but leaves
    /// `title` and `gallery_id` alone (COALESCE keeps the existing value).
    pub async fn upsert_image(
        &self,
        item_id: i64,
        gallery_id: Option<i64>,
        title: &str,
        exif: &crate::services::images::ImageExif,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO images
                (item_id, gallery_id, title, taken_at, width, height, camera_make,
                 camera_model, lens, iso, focal_length, aperture, gps_lat, gps_lon, orientation)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(item_id) DO UPDATE SET
                gallery_id=COALESCE(images.gallery_id, excluded.gallery_id),
                taken_at=excluded.taken_at, width=excluded.width, height=excluded.height,
                camera_make=excluded.camera_make, camera_model=excluded.camera_model,
                lens=excluded.lens, iso=excluded.iso, focal_length=excluded.focal_length,
                aperture=excluded.aperture, gps_lat=excluded.gps_lat, gps_lon=excluded.gps_lon,
                orientation=excluded.orientation"
        )
        .bind(item_id).bind(gallery_id).bind(title)
        .bind(&exif.taken_at).bind(exif.width).bind(exif.height)
        .bind(&exif.camera_make).bind(&exif.camera_model).bind(&exif.lens)
        .bind(exif.iso).bind(exif.focal_length).bind(exif.aperture)
        .bind(exif.gps_lat).bind(exif.gps_lon).bind(exif.orientation)
        .execute(&self.pool).await?;
        Ok(())
    }

    /// Delete a spine row (cascades to its detail row, credits, genres and user state).
    pub async fn delete_item(&self, item_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM media_items WHERE id = ?").bind(item_id).execute(&self.pool).await?;
        Ok(())
    }
}

// ── Private helpers (normalized lookup/join tables) ─────────────────────────

/// Get-or-create a row in a simple `(id, name UNIQUE)` lookup table and return its id.
async fn get_or_create_named(pool: &SqlitePool, table: &str, name: &str) -> Result<i64, AppError> {
    // `table` is never user-supplied — only the fixed names below.
    sqlx::query(&format!("INSERT OR IGNORE INTO {table} (name) VALUES (?)"))
        .bind(name)
        .execute(pool)
        .await?;
    let (id,) = sqlx::query_as::<_, (i64,)>(&format!("SELECT id FROM {table} WHERE name = ?"))
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(id)
}

async fn get_or_create_studio(pool: &SqlitePool, name: &str) -> Result<i64, AppError> {
    get_or_create_named(pool, "studios", name).await
}

async fn get_or_create_person(pool: &SqlitePool, name: &str, profile_url: Option<&str>) -> Result<i64, AppError> {
    sqlx::query("INSERT OR IGNORE INTO people (name, profile_url) VALUES (?, ?)")
        .bind(name)
        .bind(profile_url)
        .execute(pool)
        .await?;
    let (id,) = sqlx::query_as::<_, (i64,)>("SELECT id FROM people WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(id)
}

/// Replace the genre links for an item with the given genre names.
async fn set_item_genres(pool: &SqlitePool, item_id: i64, genres: &[String]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM item_genres WHERE item_id = ?").bind(item_id).execute(pool).await?;
    for name in genres {
        let gid = get_or_create_named(pool, "genres", name).await?;
        sqlx::query("INSERT OR IGNORE INTO item_genres (item_id, genre_id) VALUES (?, ?)")
            .bind(item_id).bind(gid).execute(pool).await?;
    }
    Ok(())
}

/// Replace the genre links for a series with the given genre names.
async fn set_series_genres(pool: &SqlitePool, series_id: i64, genres: &[String]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM series_genres WHERE series_id = ?").bind(series_id).execute(pool).await?;
    for name in genres {
        let gid = get_or_create_named(pool, "genres", name).await?;
        sqlx::query("INSERT OR IGNORE INTO series_genres (series_id, genre_id) VALUES (?, ?)")
            .bind(series_id).bind(gid).execute(pool).await?;
    }
    Ok(())
}

/// Replace the tag links for an item with the given tag names.
async fn set_item_tags(pool: &SqlitePool, item_id: i64, tags: &[String]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM item_tags WHERE item_id = ?").bind(item_id).execute(pool).await?;
    for name in tags {
        let tid = get_or_create_named(pool, "tags", name).await?;
        sqlx::query("INSERT OR IGNORE INTO item_tags (item_id, tag_id) VALUES (?, ?)")
            .bind(item_id).bind(tid).execute(pool).await?;
    }
    Ok(())
}

/// Replace the tag links for a series with the given tag names.
async fn set_series_tags(pool: &SqlitePool, series_id: i64, tags: &[String]) -> Result<(), AppError> {
    sqlx::query("DELETE FROM series_tags WHERE series_id = ?").bind(series_id).execute(pool).await?;
    for name in tags {
        let tid = get_or_create_named(pool, "tags", name).await?;
        sqlx::query("INSERT OR IGNORE INTO series_tags (series_id, tag_id) VALUES (?, ?)")
            .bind(series_id).bind(tid).execute(pool).await?;
    }
    Ok(())
}

/// Replace the cast/crew credits, keyed by either `item_id` or `series_id`
/// (exactly one is `Some`). Cast comes from `meta.cast`; directors are added as
/// `Director` credits when present.
async fn set_credits(
    pool: &SqlitePool,
    item_id: Option<i64>,
    series_id: Option<i64>,
    meta: &NormalizedMetadata,
) -> Result<(), AppError> {
    match (item_id, series_id) {
        (Some(id), _) => { sqlx::query("DELETE FROM credits WHERE item_id = ?").bind(id).execute(pool).await?; }
        (_, Some(id)) => { sqlx::query("DELETE FROM credits WHERE series_id = ?").bind(id).execute(pool).await?; }
        _ => return Ok(()),
    }

    if let Some(cast) = &meta.cast {
        for member in cast {
            let pid = get_or_create_person(pool, &member.name, member.profile_url.as_deref()).await?;
            sqlx::query("INSERT INTO credits (person_id, item_id, series_id, role, character, ord) VALUES (?, ?, ?, ?, ?, ?)")
                .bind(pid).bind(item_id).bind(series_id)
                .bind(&member.role).bind(&member.character).bind(member.order)
                .execute(pool).await?;
        }
    }
    if let Some(directors) = &meta.director {
        for name in directors {
            let pid = get_or_create_person(pool, name, None).await?;
            sqlx::query("INSERT INTO credits (person_id, item_id, series_id, role, character, ord) VALUES (?, ?, ?, 'Director', NULL, 0)")
                .bind(pid).bind(item_id).bind(series_id)
                .execute(pool).await?;
        }
    }
    Ok(())
}

fn parse_year(meta: &NormalizedMetadata) -> Option<i64> {
    meta.year.as_ref().and_then(|y| y.get(0..4)).and_then(|y| y.parse::<i64>().ok())
}

//! Gallery / photo service.
//!
//! Owns the user-facing mutations for an Images (photo) library: creating and
//! editing albums (galleries), moving photos between albums, and editing a
//! photo's options. Scan-time ingest goes through
//! [`crate::services::catalog_service::CatalogService`] instead; EXIF/file parsing
//! lives in [`crate::services::images`]. Reads are served by
//! [`crate::services::media_service`].

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::models::db::galleries::Gallery;

pub struct GalleryService {
    pool: SqlitePool,
}

impl GalleryService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new (manual) gallery in an Images library. Errors if the library
    /// doesn't exist or isn't an Images library, or if the name is already taken.
    pub async fn create(&self, library_id: i64, name: &str, description: Option<&str>) -> Result<i64, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Gallery name cannot be empty".to_string()));
        }

        let lib_type: Option<String> = sqlx::query_scalar("SELECT library_type FROM libraries WHERE id = ?")
            .bind(library_id).fetch_optional(&self.pool).await?;
        match lib_type.as_deref() {
            None => return Err(AppError::LibraryNotFound(library_id)),
            Some("images") => {}
            Some(_) => return Err(AppError::BadRequest("Library is not an Images library".to_string())),
        }

        let existing: Option<i64> = sqlx::query_scalar("SELECT id FROM galleries WHERE library_id = ? AND name = ?")
            .bind(library_id).bind(name).fetch_optional(&self.pool).await?;
        if existing.is_some() {
            return Err(AppError::BadRequest(format!("A gallery named '{}' already exists", name)));
        }

        let id = sqlx::query("INSERT INTO galleries (library_id, name, description) VALUES (?, ?, ?)")
            .bind(library_id).bind(name).bind(description)
            .execute(&self.pool).await?
            .last_insert_rowid();
        Ok(id)
    }

    /// Fetch a gallery by id, erroring if missing.
    pub async fn get(&self, id: i64) -> Result<Gallery, AppError> {
        sqlx::query_as::<_, Gallery>("SELECT * FROM galleries WHERE id = ?")
            .bind(id).fetch_optional(&self.pool).await?
            .ok_or_else(|| AppError::NotFound(format!("Gallery {} not found", id)))
    }

    /// Edit a gallery's mutable fields. `None` fields are left unchanged.
    pub async fn update(
        &self,
        id: i64,
        name: Option<&str>,
        description: Option<&str>,
        cover_url: Option<&str>,
    ) -> Result<(), AppError> {
        // Ensure it exists (and give a clean 404 rather than a silent no-op).
        self.get(id).await?;

        if let Some(name) = name {
            let name = name.trim();
            if name.is_empty() {
                return Err(AppError::BadRequest("Gallery name cannot be empty".to_string()));
            }
        }

        let result = sqlx::query(
            "UPDATE galleries SET
                name = COALESCE(?, name),
                description = COALESCE(?, description),
                cover_url = COALESCE(?, cover_url)
             WHERE id = ?"
        )
        .bind(name.map(str::trim))
        .bind(description)
        .bind(cover_url)
        .bind(id)
        .execute(&self.pool).await;

        // Surface the UNIQUE(library_id, name) collision as a 400 instead of a 500.
        if let Err(sqlx::Error::Database(e)) = &result {
            if e.message().contains("UNIQUE") {
                return Err(AppError::BadRequest("A gallery with that name already exists".to_string()));
            }
        }
        result?;
        Ok(())
    }

    /// Delete a gallery. Its photos are kept but become ungrouped
    /// (`images.gallery_id` is set NULL by the FK's ON DELETE SET NULL).
    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM galleries WHERE id = ?")
            .bind(id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Gallery {} not found", id)));
        }
        Ok(())
    }

    /// Assign a set of photos to a gallery. Only rows that are actually images are
    /// affected. Returns the number of photos moved.
    pub async fn add_images(&self, gallery_id: i64, item_ids: &[i64]) -> Result<u64, AppError> {
        self.get(gallery_id).await?;
        if item_ids.is_empty() {
            return Ok(0);
        }

        // Assigning a photo to an album also lifts it out of the recycle bin.
        let placeholders = std::iter::repeat("?").take(item_ids.len()).collect::<Vec<_>>().join(",");
        let sql = format!(
            "UPDATE images SET gallery_id = ?, deleted_at = NULL, prev_gallery_id = NULL \
             WHERE item_id IN ({})",
            placeholders
        );
        let mut q = sqlx::query(&sql).bind(gallery_id);
        for id in item_ids {
            q = q.bind(id);
        }
        let result = q.execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// Remove a photo from a gallery into the recycle bin. The photo is kept but
    /// soft-deleted (`deleted_at` set); the album it came from is stashed in
    /// `prev_gallery_id` so [`Self::restore_image`] can put it back.
    pub async fn remove_image(&self, gallery_id: i64, item_id: i64) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE images
                SET prev_gallery_id = gallery_id,
                    gallery_id = NULL,
                    deleted_at = CURRENT_TIMESTAMP
             WHERE item_id = ? AND gallery_id = ?"
        )
        .bind(item_id).bind(gallery_id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Image {} not found in gallery {}", item_id, gallery_id)));
        }
        Ok(())
    }

    /// Restore a photo out of the recycle bin, back into the album it was removed
    /// from. If that album has since been deleted, `prev_gallery_id` will already
    /// be NULL (ON DELETE SET NULL) and the photo returns ungrouped.
    pub async fn restore_image(&self, item_id: i64) -> Result<(), AppError> {
        let result = sqlx::query(
            "UPDATE images
                SET gallery_id = prev_gallery_id,
                    prev_gallery_id = NULL,
                    deleted_at = NULL
             WHERE item_id = ? AND deleted_at IS NOT NULL"
        )
        .bind(item_id).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Image {} not found in the recycle bin", item_id)));
        }
        Ok(())
    }

    /// Permanently delete a trashed photo (removes its `media_items` spine row,
    /// which cascades to the `images` row). Only photos already in the recycle
    /// bin can be purged, so a live photo is never destroyed by this call.
    pub async fn purge_image(&self, item_id: i64) -> Result<(), AppError> {
        let trashed: Option<i64> = sqlx::query_scalar(
            "SELECT item_id FROM images WHERE item_id = ? AND deleted_at IS NOT NULL"
        ).bind(item_id).fetch_optional(&self.pool).await?;
        if trashed.is_none() {
            return Err(AppError::NotFound(format!("Image {} not found in the recycle bin", item_id)));
        }
        sqlx::query("DELETE FROM media_items WHERE id = ?")
            .bind(item_id).execute(&self.pool).await?;
        Ok(())
    }

    /// Empty the recycle bin for one Images library. Returns the number of photos
    /// permanently deleted.
    pub async fn empty_trash(&self, library_id: i64) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM media_items
             WHERE id IN (
                 SELECT i.item_id FROM images i
                 JOIN media_items mi ON mi.id = i.item_id
                 WHERE mi.library_id = ? AND i.deleted_at IS NOT NULL
             )"
        ).bind(library_id).execute(&self.pool).await?;
        Ok(result.rows_affected())
    }

    /// Edit a photo's options. `None` fields are left unchanged; passing a
    /// `gallery_id` moves the photo into that album.
    pub async fn update_image(
        &self,
        item_id: i64,
        title: Option<&str>,
        taken_at: Option<&str>,
        gallery_id: Option<i64>,
    ) -> Result<(), AppError> {
        // Moving the photo into an album (a non-NULL gallery_id) also lifts it out
        // of the recycle bin; otherwise the trash state is left untouched.
        let result = sqlx::query(
            "UPDATE images SET
                title = COALESCE(?, title),
                taken_at = COALESCE(?, taken_at),
                gallery_id = COALESCE(?, gallery_id),
                deleted_at = CASE WHEN ? IS NOT NULL THEN NULL ELSE deleted_at END,
                prev_gallery_id = CASE WHEN ? IS NOT NULL THEN NULL ELSE prev_gallery_id END
             WHERE item_id = ?"
        )
        .bind(title)
        .bind(taken_at)
        .bind(gallery_id)
        .bind(gallery_id)
        .bind(gallery_id)
        .bind(item_id)
        .execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Err(AppError::NotFound(format!("Image {} not found", item_id)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::media_service::MediaService;
    use crate::test_support::{seed_item, seed_library, test_pool};

    /// Seed an image spine row plus its `images` detail row in a gallery.
    async fn seed_image(pool: &SqlitePool, library_id: i64, gallery_id: i64, file_path: &str) -> i64 {
        let id = seed_item(pool, library_id, "image", file_path).await;
        sqlx::query("INSERT INTO images (item_id, gallery_id) VALUES (?, ?)")
            .bind(id).bind(gallery_id).execute(pool).await.unwrap();
        id
    }

    async fn state(pool: &SqlitePool, item_id: i64) -> (Option<i64>, Option<i64>, Option<String>) {
        sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<String>)>(
            "SELECT gallery_id, prev_gallery_id, deleted_at FROM images WHERE item_id = ?"
        ).bind(item_id).fetch_one(pool).await.unwrap()
    }

    #[tokio::test]
    async fn remove_trashes_and_restore_returns_to_album() {
        let pool = test_pool().await;
        let lib = seed_library(&pool, "Photos", "images").await;
        let svc = GalleryService::new(pool.clone());
        let gid = svc.create(lib, "Trip", None).await.unwrap();
        let img = seed_image(&pool, lib, gid, "/data/a.jpg").await;

        // Remove -> trashed, stashes the album, disappears from the picker.
        svc.remove_image(gid, img).await.unwrap();
        let (gallery_id, prev, deleted_at) = state(&pool, img).await;
        assert_eq!(gallery_id, None);
        assert_eq!(prev, Some(gid));
        assert!(deleted_at.is_some());

        let reader = MediaService::new(pool.clone());
        assert!(reader.library_images(lib).await.unwrap().is_empty());
        assert_eq!(reader.gallery_detail(gid).await.unwrap().image_count, 0);
        let trash = reader.trashed_images(lib).await.unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].gallery_id, Some(gid)); // shows where it'll be restored

        // Restore -> back in the album, out of the bin and back in the picker.
        svc.restore_image(img).await.unwrap();
        let (gallery_id, prev, deleted_at) = state(&pool, img).await;
        assert_eq!(gallery_id, Some(gid));
        assert_eq!(prev, None);
        assert!(deleted_at.is_none());
        assert!(reader.trashed_images(lib).await.unwrap().is_empty());
        assert_eq!(reader.gallery_detail(gid).await.unwrap().image_count, 1);
    }

    #[tokio::test]
    async fn restore_after_album_deleted_lands_ungrouped() {
        let pool = test_pool().await;
        let lib = seed_library(&pool, "Photos", "images").await;
        let svc = GalleryService::new(pool.clone());
        let gid = svc.create(lib, "Trip", None).await.unwrap();
        let img = seed_image(&pool, lib, gid, "/data/a.jpg").await;

        svc.remove_image(gid, img).await.unwrap();
        svc.delete(gid).await.unwrap(); // ON DELETE SET NULL nulls prev_gallery_id
        svc.restore_image(img).await.unwrap();

        let (gallery_id, _prev, deleted_at) = state(&pool, img).await;
        assert_eq!(gallery_id, None); // ungrouped, but no longer trashed
        assert!(deleted_at.is_none());
    }

    #[tokio::test]
    async fn purge_only_removes_trashed_photos() {
        let pool = test_pool().await;
        let lib = seed_library(&pool, "Photos", "images").await;
        let svc = GalleryService::new(pool.clone());
        let gid = svc.create(lib, "Trip", None).await.unwrap();
        let live = seed_image(&pool, lib, gid, "/data/live.jpg").await;
        let gone = seed_image(&pool, lib, gid, "/data/gone.jpg").await;

        // A live photo cannot be purged.
        assert!(svc.purge_image(live).await.is_err());

        svc.remove_image(gid, gone).await.unwrap();
        svc.purge_image(gone).await.unwrap();

        // Spine row is gone; the live one survives.
        let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM media_items WHERE library_id = ?")
            .bind(lib).fetch_all(&pool).await.unwrap();
        assert_eq!(remaining, vec![live]);
    }

    #[tokio::test]
    async fn empty_trash_purges_all_trashed() {
        let pool = test_pool().await;
        let lib = seed_library(&pool, "Photos", "images").await;
        let svc = GalleryService::new(pool.clone());
        let gid = svc.create(lib, "Trip", None).await.unwrap();
        let a = seed_image(&pool, lib, gid, "/data/a.jpg").await;
        let b = seed_image(&pool, lib, gid, "/data/b.jpg").await;
        let keep = seed_image(&pool, lib, gid, "/data/keep.jpg").await;

        svc.remove_image(gid, a).await.unwrap();
        svc.remove_image(gid, b).await.unwrap();
        assert_eq!(svc.empty_trash(lib).await.unwrap(), 2);

        let remaining: Vec<i64> = sqlx::query_scalar("SELECT id FROM media_items WHERE library_id = ?")
            .bind(lib).fetch_all(&pool).await.unwrap();
        assert_eq!(remaining, vec![keep]);
    }
}

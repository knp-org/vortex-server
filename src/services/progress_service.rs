//! Progress Service
//!
//! Owns reads/writes for per-user playback/reading progress
//! (`user_media_progress`), including the "continue watching" rail.

use sqlx::SqlitePool;
use crate::error::AppError;

/// A row backing the "continue watching" rail. The handler shapes this into the
/// public response (adding the stream URL).
#[derive(sqlx::FromRow)]
pub struct ContinueWatchingRow {
    pub id: i64,
    pub item_type: String,
    pub title: Option<String>,
    pub poster_url: Option<String>,
    pub position: i64,
    pub total_duration: i64,
    pub reading_style: Option<String>,
    /// Opaque per-format resume locator (EPUB CFI); `None` for paged formats.
    pub location: Option<String>,
}

/// A user's stored progress for one item.
#[derive(Debug, Default, sqlx::FromRow)]
pub struct ItemProgress {
    /// Seconds for video; page index for CBZ/PDF; permille (0-10000) for EPUB.
    pub position: i64,
    /// Total seconds / pages; `0` when the client has not reported a length yet.
    pub total_duration: i64,
    pub reading_style: Option<String>,
    /// Opaque per-format resume locator (EPUB CFI); `None` for paged formats.
    pub location: Option<String>,
}

pub struct ProgressService {
    pool: SqlitePool,
}

impl ProgressService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// In-progress items for the user: started but not yet effectively finished,
    /// excluding `other`-type libraries, most-recent first.
    ///
    /// Books are gated differently from video: a reader's `position` is a page
    /// index (CBZ/PDF) or permille of the book (EPUB), so the 10-second "did they
    /// really start it" threshold would hide the first pages, and an EPUB whose
    /// length the client has not reported yet has `total_duration = 0`.
    pub async fn continue_watching(&self, user_id: i64) -> Result<Vec<ContinueWatchingRow>, AppError> {
        Ok(sqlx::query_as::<_, ContinueWatchingRow>(
            "SELECT mi.id, mi.item_type,
                    COALESCE(mv.title, e.title, mvd.title, b.title) AS title,
                    COALESCE(mv.poster_url, e.still_url, mvd.poster_url, b.poster_url) AS poster_url,
                    p.position, p.total_duration, p.reading_style, p.location
             FROM user_media_progress p
             JOIN media_items mi ON mi.id = p.item_id
             JOIN libraries l ON l.id = mi.library_id AND l.library_type != 'other'
             LEFT JOIN movies mv ON mv.item_id = mi.id
             LEFT JOIN episodes e ON e.item_id = mi.id
             LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
             LEFT JOIN books b ON b.item_id = mi.id
             WHERE p.user_id = ?
               AND CASE WHEN mi.item_type = 'book'
                        THEN p.position > 0
                             AND (p.total_duration <= 0 OR p.position < (p.total_duration * 0.95))
                        ELSE p.position > 10 AND p.position < (p.total_duration * 0.95)
                   END
             ORDER BY p.last_watched DESC LIMIT 10"
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Upsert a user's progress for an item. A `None` reading_style or `location`
    /// preserves any previously stored value.
    pub async fn update(
        &self,
        user_id: i64,
        item_id: i64,
        position: i64,
        total_duration: i64,
        reading_style: Option<&str>,
        location: Option<&str>,
    ) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO user_media_progress (user_id, item_id, position, total_duration, reading_style, location, last_watched)
             VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
             ON CONFLICT(user_id, item_id) DO UPDATE SET
                position = excluded.position,
                total_duration = excluded.total_duration,
                reading_style = COALESCE(excluded.reading_style, user_media_progress.reading_style),
                location = COALESCE(excluded.location, user_media_progress.location),
                last_watched = CURRENT_TIMESTAMP"
        )
        .bind(user_id)
        .bind(item_id)
        .bind(position)
        .bind(total_duration)
        .bind(reading_style)
        .bind(location)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Resume position for HLS playback by media id.
    ///
    /// NOTE: this reads the legacy `playback_progress` table, which was dropped in
    /// migration `20260613120200_per_user_state.sql`, so it currently always yields
    /// `None` (errors are swallowed). Kept behavior-identical during the repository
    /// refactor; rewiring it to the per-user `user_media_progress` table needs the
    /// caller's user id plumbed into `get_hls_playlist`.
    pub async fn resume_position(&self, media_id: i64) -> Option<i64> {
        sqlx::query_scalar::<_, i64>("SELECT position FROM playback_progress WHERE media_id = ?")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .unwrap_or(None)
    }

    /// A user's stored progress for an item, or a zeroed record when none exists.
    pub async fn get(&self, user_id: i64, item_id: i64) -> Result<ItemProgress, AppError> {
        let row: Option<ItemProgress> = sqlx::query_as(
            "SELECT position, total_duration, reading_style, location \
             FROM user_media_progress WHERE user_id = ? AND item_id = ?"
        )
        .bind(user_id)
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{seed_book, seed_library, seed_movie, seed_user, test_pool};

    /// EPUB resume: the CFI round-trips, and a later update that omits it (e.g. a
    /// page-count correction) keeps the stored locator rather than wiping it.
    #[tokio::test]
    async fn epub_location_round_trips_and_is_preserved_when_omitted() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "reader").await;
        let lib = seed_library(&pool, "Books", "books").await;
        let book = seed_book(&pool, lib, "/data/dune.epub", "Dune").await;
        let svc = ProgressService::new(pool);

        let cfi = "epubcfi(/6/14[chap05ref]!/4[body01]/10[para05]/3:10)";
        svc.update(user, book, 3200, 10000, Some("paginated"), Some(cfi)).await.unwrap();

        let p = svc.get(user, book).await.unwrap();
        assert_eq!(p.location.as_deref(), Some(cfi));
        assert_eq!(p.position, 3200);
        assert_eq!(p.total_duration, 10000);

        svc.update(user, book, 3300, 10000, None, None).await.unwrap();
        let p = svc.get(user, book).await.unwrap();
        assert_eq!(p.location.as_deref(), Some(cfi), "location must survive an update that omits it");
        assert_eq!(p.reading_style.as_deref(), Some("paginated"));
        assert_eq!(p.position, 3300);
    }

    #[tokio::test]
    async fn unread_book_has_zeroed_progress() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "reader").await;
        let lib = seed_library(&pool, "Books", "books").await;
        let book = seed_book(&pool, lib, "/data/dune.epub", "Dune").await;

        let p = ProgressService::new(pool).get(user, book).await.unwrap();
        assert_eq!(p.position, 0);
        assert_eq!(p.location, None);
    }

    /// A barely-started book belongs in the rail even though the same numbers on a
    /// video would be below the 10-second "did they really start it" threshold,
    /// and an EPUB of not-yet-known length (`total_duration = 0`) still qualifies.
    #[tokio::test]
    async fn barely_started_book_is_in_the_rail_but_a_barely_started_video_is_not() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "reader").await;
        let books_lib = seed_library(&pool, "Books", "books").await;
        let movies_lib = seed_library(&pool, "Movies", "movies").await;
        let paged = seed_book(&pool, books_lib, "/data/akira.cbz", "Akira").await;
        let unsized_epub = seed_book(&pool, books_lib, "/data/dune.epub", "Dune").await;
        let movie = seed_movie(&pool, movies_lib, "/data/heat.mkv", "Heat").await;
        let svc = ProgressService::new(pool);

        svc.update(user, paged, 2, 180, None, None).await.unwrap();
        svc.update(user, unsized_epub, 40, 0, None, Some("epubcfi(/6/4!/2)")).await.unwrap();
        svc.update(user, movie, 5, 7200, None, None).await.unwrap();

        let rows = svc.continue_watching(user).await.unwrap();
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert!(ids.contains(&paged));
        assert!(ids.contains(&unsized_epub), "an EPUB of unknown length should still resume");
        assert!(!ids.contains(&movie), "5s into a film is not 'continue watching'");

        // Books carry their own title/poster and locator through the rail.
        let epub_row = rows.iter().find(|r| r.id == unsized_epub).unwrap();
        assert_eq!(epub_row.title.as_deref(), Some("Dune"));
        assert_eq!(epub_row.item_type, "book");
        assert_eq!(epub_row.location.as_deref(), Some("epubcfi(/6/4!/2)"));
    }

    #[tokio::test]
    async fn finished_book_drops_out_of_the_rail() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "reader").await;
        let lib = seed_library(&pool, "Books", "books").await;
        let book = seed_book(&pool, lib, "/data/dune.epub", "Dune").await;
        let svc = ProgressService::new(pool);

        svc.update(user, book, 9900, 10000, None, None).await.unwrap();
        assert!(svc.continue_watching(user).await.unwrap().is_empty());
    }
}

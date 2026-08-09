//! Playlists Service
//!
//! Owns reads/writes for per-user playlists (`playlists`, `playlist_tracks`),
//! including ownership checks, per-kind item-type rules, and the PIN that gates
//! hidden (`other`) playlists.
//!
//! A playlist has a `kind` that fixes which item types it may hold:
//!
//! | kind          | accepts                                  |
//! |---------------|------------------------------------------|
//! | `music`       | `track`                                  |
//! | `movie`       | `movie`                                  |
//! | `tvshow`      | `episode`                                |
//! | `music_video` | `music_video`                            |
//! | `other`       | any of the above (mixed), and is hidden  |
//!
//! Hidden playlists are omitted from listings and unreachable by id until the
//! caller proves knowledge of their PIN; callers pass that as `unlocked`, which
//! the API layer derives from a short-lived unlock token.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::api::dtos::responses::{PlaylistDto, PlaylistDetail};
use crate::services::media_service;
use crate::services::settings_service::SettingsService;

/// `user_settings` key holding the argon2 hash of the hidden-playlist PIN.
pub const PIN_SETTING_KEY: &str = "hidden_playlists_pin_hash";

/// The kind marking a playlist hidden behind the PIN.
pub const HIDDEN_KIND: &str = "other";

const KINDS: [&str; 5] = ["music", "movie", "tvshow", "music_video", "other"];

/// The `media_items.item_type` values a playlist kind accepts.
fn accepted_item_types(kind: &str) -> &'static [&'static str] {
    match kind {
        "music" => &["track"],
        "movie" => &["movie"],
        "tvshow" => &["episode"],
        "music_video" => &["music_video"],
        // The catch-all: any playable type, mixed freely.
        _ => &["movie", "episode", "music_video", "track"],
    }
}

pub fn normalize_kind(kind: Option<&str>) -> Result<String, AppError> {
    let kind = kind.unwrap_or("music").trim().to_lowercase();
    if KINDS.contains(&kind.as_str()) {
        Ok(kind)
    } else {
        Err(AppError::BadRequest(format!(
            "Unknown playlist kind '{}'; expected one of {}", kind, KINDS.join(", ")
        )))
    }
}

pub struct PlaylistsService {
    pool: SqlitePool,
}

impl PlaylistsService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Ensure the playlist exists, belongs to the caller, and — when hidden — that
    /// the caller has unlocked it. Returns the playlist's kind.
    ///
    /// A hidden playlist reached without an unlock reads as `NotFound` rather than
    /// `Forbidden`, so a locked client cannot enumerate which ids exist.
    pub async fn assert_access(&self, playlist_id: i64, user_id: i64, unlocked: bool) -> Result<String, AppError> {
        let row: Option<(i64, String)> =
            sqlx::query_as("SELECT user_id, kind FROM playlists WHERE id = ?")
                .bind(playlist_id)
                .fetch_optional(&self.pool)
                .await?;

        match row {
            Some((uid, kind)) if uid == user_id => {
                if kind == HIDDEN_KIND && !unlocked {
                    return Err(AppError::NotFound("Playlist not found".to_string()));
                }
                Ok(kind)
            }
            Some(_) => Err(AppError::Forbidden("Not your playlist".to_string())),
            None => Err(AppError::NotFound("Playlist not found".to_string())),
        }
    }

    /// The caller's playlists. Hidden ones appear only once `unlocked`.
    pub async fn list(&self, user_id: i64, unlocked: bool) -> Result<Vec<PlaylistDto>, AppError> {
        Ok(sqlx::query_as::<_, PlaylistDto>(
            "SELECT p.id, p.name, p.kind,
                    (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id) AS track_count,
                    p.created_at
             FROM playlists p
             WHERE p.user_id = ? AND (? OR p.kind != 'other')
             ORDER BY p.created_at DESC"
        )
        .bind(user_id)
        .bind(unlocked)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create(&self, user_id: i64, name: &str, kind: &str, unlocked: bool) -> Result<PlaylistDto, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::BadRequest("Playlist name is required".to_string()));
        }
        if kind == HIDDEN_KIND && !unlocked {
            return Err(AppError::Forbidden("Unlock required to create a hidden playlist".to_string()));
        }

        let id = sqlx::query("INSERT INTO playlists (user_id, name, kind) VALUES (?, ?, ?)")
            .bind(user_id)
            .bind(name)
            .bind(kind)
            .execute(&self.pool)
            .await?
            .last_insert_rowid();

        Ok(sqlx::query_as::<_, PlaylistDto>(
            "SELECT id, name, kind, 0 AS track_count, created_at FROM playlists WHERE id = ?"
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?)
    }

    /// Full playlist with its ordered members. Caller must have passed
    /// [`Self::assert_access`] first.
    pub async fn detail(&self, playlist_id: i64, kind: &str) -> Result<PlaylistDetail, AppError> {
        let name: (String,) = sqlx::query_as("SELECT name FROM playlists WHERE id = ?")
            .bind(playlist_id).fetch_one(&self.pool).await?;

        let media = media_service::MediaService::new(self.pool.clone());

        // Audio playlists keep the richer track shape existing clients expect;
        // every kind also gets the polymorphic card list.
        let tracks = if kind == "music" {
            media.playlist_tracks(playlist_id).await?
        } else {
            Vec::new()
        };

        Ok(PlaylistDetail {
            id: playlist_id,
            name: name.0,
            kind: kind.to_string(),
            tracks,
            items: media.playlist_items(playlist_id).await?,
        })
    }

    /// Append an item, rejecting types the playlist's kind does not accept.
    pub async fn add_track(&self, playlist_id: i64, kind: &str, item_id: i64) -> Result<(), AppError> {
        let item_type: Option<(String,)> =
            sqlx::query_as("SELECT item_type FROM media_items WHERE id = ?")
                .bind(item_id)
                .fetch_optional(&self.pool)
                .await?;
        let item_type = item_type
            .ok_or(AppError::MediaNotFound(item_id))?
            .0;

        let accepted = accepted_item_types(kind);
        if !accepted.contains(&item_type.as_str()) {
            return Err(AppError::BadRequest(format!(
                "A '{}' playlist accepts {}, not '{}'", kind, accepted.join("/"), item_type
            )));
        }

        sqlx::query(
            "INSERT OR IGNORE INTO playlist_tracks (playlist_id, item_id, position)
             VALUES (?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?))"
        )
        .bind(playlist_id).bind(item_id).bind(playlist_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_track(&self, playlist_id: i64, item_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM playlist_tracks WHERE playlist_id = ? AND item_id = ?")
            .bind(playlist_id).bind(item_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, playlist_id: i64) -> Result<(), AppError> {
        sqlx::query("DELETE FROM playlists WHERE id = ?")
            .bind(playlist_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Hidden-playlist PIN
    // -----------------------------------------------------------------------

    async fn pin_hash(&self, user_id: i64) -> Result<Option<String>, AppError> {
        Ok(sqlx::query_scalar::<_, String>(
            "SELECT value FROM user_settings WHERE user_id = ? AND key = ?"
        )
        .bind(user_id)
        .bind(PIN_SETTING_KEY)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn pin_is_set(&self, user_id: i64) -> Result<bool, AppError> {
        Ok(self.pin_hash(user_id).await?.is_some())
    }

    /// Set or change the caller's PIN. Changing an existing PIN requires the
    /// current one.
    pub async fn set_pin(&self, user_id: i64, current: Option<&str>, new_pin: &str) -> Result<(), AppError> {
        if !(4..=12).contains(&new_pin.len()) || !new_pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(AppError::BadRequest("PIN must be 4-12 digits".to_string()));
        }
        if self.pin_is_set(user_id).await? {
            let current = current.ok_or(AppError::BadRequest("Current PIN is required".to_string()))?;
            self.verify_pin(user_id, current).await?;
        }

        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(new_pin.as_bytes(), &salt)
            .map_err(|e| AppError::Internal(format!("PIN hashing failed: {}", e)))?
            .to_string();

        SettingsService::new(self.pool.clone())
            .upsert_for_user_unchecked(user_id, PIN_SETTING_KEY, &hash)
            .await
    }

    /// Check a PIN against the stored hash. Returns `AuthError` when it does not
    /// match, or when no PIN has been set yet.
    pub async fn verify_pin(&self, user_id: i64, pin: &str) -> Result<(), AppError> {
        let stored = self.pin_hash(user_id).await?
            .ok_or(AppError::AuthError("No PIN has been set".to_string()))?;
        let parsed = PasswordHash::new(&stored)
            .map_err(|e| AppError::Internal(format!("Invalid PIN hash: {}", e)))?;
        Argon2::default()
            .verify_password(pin.as_bytes(), &parsed)
            .map_err(|_| AppError::AuthError("Invalid PIN".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{seed_item, seed_library, seed_user, test_pool};

    /// Seed a movie and return its item id.
    async fn seed_movie(pool: &SqlitePool, library_id: i64, title: &str) -> i64 {
        let item_id = seed_item(pool, library_id, "movie", &format!("/data/{}.mkv", title)).await;
        sqlx::query("INSERT INTO movies (item_id, title) VALUES (?, ?)")
            .bind(item_id).bind(title).execute(pool).await.unwrap();
        item_id
    }

    #[tokio::test]
    async fn movie_playlist_holds_movies_and_rejects_other_types() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "a").await;
        let library = seed_library(&pool, "Movies", "movies").await;
        let movie = seed_movie(&pool, library, "Heat").await;
        let stray = seed_item(&pool, library, "track", "/data/song.flac").await;

        let service = PlaylistsService::new(pool.clone());
        let playlist = service.create(user, "Night in", "movie", false).await.unwrap();
        assert_eq!(playlist.kind, "movie");

        service.add_track(playlist.id, "movie", movie).await.unwrap();
        let err = service.add_track(playlist.id, "movie", stray).await.unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));

        let detail = service.detail(playlist.id, "movie").await.unwrap();
        assert_eq!(detail.items.len(), 1);
        assert_eq!(detail.items[0].title.as_deref(), Some("Heat"));
        assert_eq!(detail.items[0].kind, "movie");
        // Only music playlists carry the audio-shaped track list.
        assert!(detail.tracks.is_empty());
    }

    #[tokio::test]
    async fn other_playlist_mixes_types_but_hides_until_unlocked() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "a").await;
        let library = seed_library(&pool, "Stuff", "other").await;
        let movie = seed_movie(&pool, library, "Heat").await;

        let mv = seed_item(&pool, library, "music_video", "/data/mv.mp4").await;
        sqlx::query("INSERT INTO music_videos (item_id, title) VALUES (?, 'Take On Me')")
            .bind(mv).execute(&pool).await.unwrap();

        let service = PlaylistsService::new(pool.clone());
        service.create(user, "Visible", "movie", false).await.unwrap();

        // Creating a hidden playlist itself requires an unlock.
        assert!(matches!(
            service.create(user, "Private", HIDDEN_KIND, false).await.unwrap_err(),
            AppError::Forbidden(_)
        ));
        let hidden = service.create(user, "Private", HIDDEN_KIND, true).await.unwrap();

        // A mixed bag is fine in an `other` playlist.
        service.add_track(hidden.id, HIDDEN_KIND, movie).await.unwrap();
        service.add_track(hidden.id, HIDDEN_KIND, mv).await.unwrap();
        assert_eq!(service.detail(hidden.id, HIDDEN_KIND).await.unwrap().items.len(), 2);

        // Locked: absent from listings and unreachable by id.
        let locked = service.list(user, false).await.unwrap();
        assert_eq!(locked.len(), 1);
        assert_eq!(locked[0].name, "Visible");
        assert!(matches!(
            service.assert_access(hidden.id, user, false).await.unwrap_err(),
            AppError::NotFound(_)
        ));

        // Unlocked: visible and reachable.
        assert_eq!(service.list(user, true).await.unwrap().len(), 2);
        assert_eq!(service.assert_access(hidden.id, user, true).await.unwrap(), HIDDEN_KIND);
    }

    #[tokio::test]
    async fn pin_must_be_verified_before_it_can_be_changed() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "a").await;
        let service = PlaylistsService::new(pool.clone());

        assert!(!service.pin_is_set(user).await.unwrap());
        assert!(matches!(
            service.verify_pin(user, "1234").await.unwrap_err(),
            AppError::AuthError(_)
        ));

        assert!(matches!(
            service.set_pin(user, None, "12").await.unwrap_err(),
            AppError::BadRequest(_)
        ));
        service.set_pin(user, None, "1234").await.unwrap();
        assert!(service.pin_is_set(user).await.unwrap());
        service.verify_pin(user, "1234").await.unwrap();
        assert!(matches!(
            service.verify_pin(user, "4321").await.unwrap_err(),
            AppError::AuthError(_)
        ));

        // Replacing it requires the current PIN.
        assert!(service.set_pin(user, None, "5678").await.is_err());
        assert!(service.set_pin(user, Some("0000"), "5678").await.is_err());
        service.set_pin(user, Some("1234"), "5678").await.unwrap();
        service.verify_pin(user, "5678").await.unwrap();
    }

    #[tokio::test]
    async fn pin_hash_is_not_exposed_or_writable_as_a_user_setting() {
        let pool = test_pool().await;
        let user = seed_user(&pool, "a").await;
        let settings = SettingsService::new(pool.clone());

        PlaylistsService::new(pool.clone()).set_pin(user, None, "1234").await.unwrap();
        settings.upsert_for_user(user, "theme", "dark").await.unwrap();

        let listed = settings.list_for_user(user).await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].key, "theme");

        assert!(matches!(
            settings.upsert_for_user(user, PIN_SETTING_KEY, "planted").await.unwrap_err(),
            AppError::Forbidden(_)
        ));
    }

    #[tokio::test]
    async fn unknown_kinds_are_rejected_and_music_is_the_default() {
        assert_eq!(normalize_kind(None).unwrap(), "music");
        assert_eq!(normalize_kind(Some("TvShow")).unwrap(), "tvshow");
        assert!(normalize_kind(Some("photos")).is_err());
    }
}

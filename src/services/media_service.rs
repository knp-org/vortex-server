//! Read layer over the per-type catalog tables.
//!
//! Assembles the response DTOs (cards, detail views) the API returns. All writes go
//! through [`crate::services::catalog_service`]; this module only reads.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::libraries::LibraryType;
use crate::api::dtos::responses::{Card, CreditDto, MovieDetail, MusicVideoDetail, SeriesDetail, SeasonDto, EpisodeDto, BookDetail, AlbumDetail, TrackDto, ArtistDetail, GalleryDetail, ImageDto, ImageDetail};

/// Read layer over the per-type catalog tables. The free functions below are the
/// internal query layer; this struct is the public entry point.
pub struct MediaService {
    pool: SqlitePool,
}

impl MediaService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_library(&self, library_id: i64, library_type: &LibraryType) -> Result<Vec<Card>, AppError> {
        list_library(&self.pool, library_id, library_type).await
    }
    pub async fn recently_added(&self) -> Result<Vec<Card>, AppError> {
        recently_added(&self.pool).await
    }
    pub async fn search(&self, query: &str) -> Result<Vec<Card>, AppError> {
        search(&self.pool, query).await
    }
    pub async fn movie_detail(&self, item_id: i64) -> Result<MovieDetail, AppError> {
        movie_detail(&self.pool, item_id).await
    }
    pub async fn music_video_detail(&self, item_id: i64) -> Result<MusicVideoDetail, AppError> {
        music_video_detail(&self.pool, item_id).await
    }
    pub async fn series_cards(&self, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
        series_cards(&self.pool, library_id).await
    }
    pub async fn series_detail(&self, series_id: i64) -> Result<SeriesDetail, AppError> {
        series_detail(&self.pool, series_id).await
    }
    pub async fn series_seasons(&self, series_id: i64) -> Result<Vec<SeasonDto>, AppError> {
        series_seasons(&self.pool, series_id).await
    }
    pub async fn season_episodes(&self, series_id: i64, season_number: i64) -> Result<Vec<EpisodeDto>, AppError> {
        season_episodes(&self.pool, series_id, season_number).await
    }
    pub async fn episode_detail(&self, item_id: i64) -> Result<EpisodeDto, AppError> {
        episode_detail(&self.pool, item_id).await
    }
    pub async fn book_detail(&self, item_id: i64) -> Result<BookDetail, AppError> {
        book_detail(&self.pool, item_id).await
    }
    pub async fn artist_cards(&self, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
        artist_cards(&self.pool, library_id).await
    }
    pub async fn library_tracks(&self, library_id: i64) -> Result<Vec<TrackDto>, AppError> {
        library_tracks(&self.pool, library_id).await
    }
    pub async fn artist_detail(&self, artist_id: i64) -> Result<ArtistDetail, AppError> {
        artist_detail(&self.pool, artist_id).await
    }
    pub async fn album_detail(&self, album_id: i64) -> Result<AlbumDetail, AppError> {
        album_detail(&self.pool, album_id).await
    }
    pub async fn gallery_cards(&self, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
        gallery_cards(&self.pool, library_id).await
    }
    pub async fn gallery_detail(&self, gallery_id: i64) -> Result<GalleryDetail, AppError> {
        gallery_detail(&self.pool, gallery_id).await
    }
    pub async fn library_images(&self, library_id: i64) -> Result<Vec<ImageDto>, AppError> {
        library_images(&self.pool, library_id).await
    }
    pub async fn trashed_images(&self, library_id: i64) -> Result<Vec<ImageDto>, AppError> {
        trashed_images(&self.pool, library_id).await
    }
    pub async fn image_detail(&self, item_id: i64) -> Result<ImageDetail, AppError> {
        image_detail(&self.pool, item_id).await
    }
    pub async fn playlist_tracks(&self, playlist_id: i64) -> Result<Vec<TrackDto>, AppError> {
        playlist_tracks(&self.pool, playlist_id).await
    }
    pub async fn movie_provider_lookup(&self, item_id: i64) -> Result<(Option<String>, Option<String>), AppError> {
        movie_provider_lookup(&self.pool, item_id).await
    }
    pub async fn file_path(&self, id: i64) -> Result<Option<String>, AppError> {
        file_path(&self.pool, id).await
    }
    pub async fn require_file_path(&self, id: i64) -> Result<String, AppError> {
        require_file_path(&self.pool, id).await
    }
    pub async fn thumbnail_sources(&self, id: i64) -> Result<Option<(String, Option<String>, Option<String>)>, AppError> {
        thumbnail_sources(&self.pool, id).await
    }
    pub async fn item_type(&self, id: i64) -> Result<String, AppError> {
        item_type(&self.pool, id).await
    }
    pub async fn track_album_id(&self, item_id: i64) -> Result<Option<i64>, AppError> {
        track_album_id(&self.pool, item_id).await
    }
    pub async fn series_provider_lookup(&self, series_id: i64) -> Result<(String, Option<String>), AppError> {
        series_provider_lookup(&self.pool, series_id).await
    }
    pub async fn episode_item_id(&self, series_id: i64, season_number: i64, episode_number: i64) -> Result<Option<i64>, AppError> {
        episode_item_id(&self.pool, series_id, season_number, episode_number).await
    }
}

fn stream_url(item_id: i64) -> String {
    format!("/api/v1/stream/{}", item_id)
}

/// Full-resolution original photo URL for an image item.
fn image_url(item_id: i64) -> String {
    format!("/api/v1/images/item/{}", item_id)
}

/// Server-scaled thumbnail URL (shared thumbnail endpoint, works for photos).
fn image_thumb_url(item_id: i64) -> String {
    format!("/api/v1/media/{}/thumbnail", item_id)
}

async fn item_genres(pool: &SqlitePool, item_id: i64) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT g.name FROM item_genres ig JOIN genres g ON g.id = ig.genre_id WHERE ig.item_id = ? ORDER BY g.name"
    ).bind(item_id).fetch_all(pool).await?)
}

async fn series_genres(pool: &SqlitePool, series_id: i64) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT g.name FROM series_genres sg JOIN genres g ON g.id = sg.genre_id WHERE sg.series_id = ? ORDER BY g.name"
    ).bind(series_id).fetch_all(pool).await?)
}

async fn series_tags(pool: &SqlitePool, series_id: i64) -> Result<Vec<String>, AppError> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT t.name FROM series_tags st JOIN tags t ON t.id = st.tag_id WHERE st.series_id = ? ORDER BY t.name"
    ).bind(series_id).fetch_all(pool).await?)
}

async fn item_credits(pool: &SqlitePool, item_id: i64) -> Result<Vec<CreditDto>, AppError> {
    Ok(sqlx::query_as::<_, CreditDto>(
        "SELECT p.name, c.character, c.role, p.profile_url, c.ord
         FROM credits c JOIN people p ON p.id = c.person_id
         WHERE c.item_id = ? ORDER BY c.ord"
    ).bind(item_id).fetch_all(pool).await?)
}

async fn series_credits(pool: &SqlitePool, series_id: i64) -> Result<Vec<CreditDto>, AppError> {
    Ok(sqlx::query_as::<_, CreditDto>(
        "SELECT p.name, c.character, c.role, p.profile_url, c.ord
         FROM credits c JOIN people p ON p.id = c.person_id
         WHERE c.series_id = ? ORDER BY c.ord"
    ).bind(series_id).fetch_all(pool).await?)
}

/// Cards for a single library, shaped by its type.
async fn list_library(pool: &SqlitePool, library_id: i64, library_type: &LibraryType) -> Result<Vec<Card>, AppError> {
    let cards = match library_type {
        LibraryType::TvShows => sqlx::query_as::<_, Card>(
            "SELECT id, 'series' AS kind, name AS title, poster_url, year, NULL AS stream_url
             FROM series WHERE library_id = ? ORDER BY name"
        ).bind(library_id).fetch_all(pool).await?,

        LibraryType::Books => sqlx::query_as::<_, Card>(
            "SELECT mi.id, 'book' AS kind, b.title, b.poster_url, NULL AS year, NULL AS stream_url
             FROM media_items mi JOIN books b ON b.item_id = mi.id
             WHERE mi.library_id = ? ORDER BY b.title"
        ).bind(library_id).fetch_all(pool).await?,

        LibraryType::Music => sqlx::query_as::<_, Card>(
            "SELECT id, 'album' AS kind, title, cover_url AS poster_url, year, NULL AS stream_url
             FROM albums WHERE library_id = ? ORDER BY title"
        ).bind(library_id).fetch_all(pool).await?,

        LibraryType::MusicVideos => sqlx::query_as::<_, Card>(
            "SELECT mi.id, 'music_video' AS kind, mv.title, mv.poster_url, mv.year,
                    ('/api/v1/stream/' || mi.id) AS stream_url
             FROM media_items mi JOIN music_videos mv ON mv.item_id = mi.id
             WHERE mi.library_id = ? ORDER BY mv.title"
        ).bind(library_id).fetch_all(pool).await?,

        // Reuse gallery_cards so gallery cards carry their mosaic `thumbs`
        // (identical query + ordering, plus up to 4 thumbnails per gallery).
        LibraryType::Images => gallery_cards(pool, Some(library_id)).await?,

        // Movies, Other -> movie cards.
        _ => sqlx::query_as::<_, Card>(
            "SELECT mi.id, 'movie' AS kind, mv.title, mv.poster_url, mv.year,
                    ('/api/v1/stream/' || mi.id) AS stream_url
             FROM media_items mi JOIN movies mv ON mv.item_id = mi.id
             WHERE mi.library_id = ? ORDER BY mv.title"
        ).bind(library_id).fetch_all(pool).await?,
    };
    Ok(cards)
}

/// Recently-added cards across all libraries (series collapse to one card).
async fn recently_added(pool: &SqlitePool) -> Result<Vec<Card>, AppError> {
    let cards = sqlx::query_as::<_, Card>(
        "SELECT id, kind, title, poster_url, year, stream_url FROM (
            SELECT mi.id, 'movie' AS kind, mv.title, mv.poster_url, mv.year,
                   ('/api/v1/stream/' || mi.id) AS stream_url, mi.added_at AS added_at
            FROM media_items mi JOIN movies mv ON mv.item_id = mi.id
            JOIN libraries l ON l.id = mi.library_id AND l.library_type != 'other'
            UNION ALL
            SELECT mi.id, 'music_video' AS kind, mvd.title, mvd.poster_url, mvd.year,
                   ('/api/v1/stream/' || mi.id) AS stream_url, mi.added_at
            FROM media_items mi JOIN music_videos mvd ON mvd.item_id = mi.id
            UNION ALL
            SELECT mi.id, 'book' AS kind, b.title, b.poster_url, NULL AS year,
                   NULL AS stream_url, mi.added_at
            FROM media_items mi JOIN books b ON b.item_id = mi.id
            UNION ALL
            SELECT s.id, 'series' AS kind, s.name AS title, s.poster_url, s.year,
                   NULL AS stream_url,
                   (SELECT MAX(mi.added_at) FROM media_items mi
                      JOIN episodes e ON e.item_id = mi.id
                      JOIN seasons se ON se.id = e.season_id
                    WHERE se.series_id = s.id) AS added_at
            FROM series s
            UNION ALL
            SELECT al.id, 'album' AS kind, al.title, al.cover_url, al.year,
                   NULL AS stream_url,
                   (SELECT MAX(mi.added_at) FROM media_items mi
                      JOIN tracks t ON t.item_id = mi.id
                    WHERE t.album_id = al.id) AS added_at
            FROM albums al
            UNION ALL
            SELECT g.id, 'gallery' AS kind, g.name AS title, g.cover_url, NULL AS year,
                   NULL AS stream_url,
                   (SELECT MAX(mi.added_at) FROM media_items mi
                      JOIN images i ON i.item_id = mi.id
                    WHERE i.gallery_id = g.id) AS added_at
            FROM galleries g
        )
        WHERE added_at IS NOT NULL
        ORDER BY added_at DESC LIMIT 20"
    ).fetch_all(pool).await?;
    Ok(cards)
}

/// Title search across movies, series, books and music videos.
async fn search(pool: &SqlitePool, query: &str) -> Result<Vec<Card>, AppError> {
    let like = format!("%{}%", query);
    let cards = sqlx::query_as::<_, Card>(
        "SELECT id, kind, title, poster_url, year, stream_url FROM (
            SELECT mi.id, 'movie' AS kind, mv.title, mv.poster_url, mv.year,
                   ('/api/v1/stream/' || mi.id) AS stream_url
            FROM media_items mi JOIN movies mv ON mv.item_id = mi.id WHERE mv.title LIKE ?
            UNION ALL
            SELECT s.id, 'series' AS kind, s.name AS title, s.poster_url, s.year, NULL AS stream_url
            FROM series s WHERE s.name LIKE ?
            UNION ALL
            SELECT mi.id, 'book' AS kind, b.title, b.poster_url, NULL AS year, NULL AS stream_url
            FROM media_items mi JOIN books b ON b.item_id = mi.id WHERE b.title LIKE ?
            UNION ALL
            SELECT mi.id, 'music_video' AS kind, mvd.title, mvd.poster_url, mvd.year,
                   ('/api/v1/stream/' || mi.id) AS stream_url
            FROM media_items mi JOIN music_videos mvd ON mvd.item_id = mi.id WHERE mvd.title LIKE ?
            UNION ALL
            SELECT id, 'album' AS kind, title, cover_url AS poster_url, year, NULL AS stream_url
            FROM albums WHERE title LIKE ?
            UNION ALL
            SELECT id, 'artist' AS kind, name AS title, image_url AS poster_url, NULL AS year, NULL AS stream_url
            FROM artists WHERE name LIKE ?
            UNION ALL
            SELECT id, 'gallery' AS kind, name AS title, cover_url AS poster_url, NULL AS year, NULL AS stream_url
            FROM galleries WHERE name LIKE ?
        )
        ORDER BY title LIMIT 20"
    ).bind(&like).bind(&like).bind(&like).bind(&like).bind(&like).bind(&like).bind(&like).fetch_all(pool).await?;
    Ok(cards)
}

async fn movie_detail(pool: &SqlitePool, item_id: i64) -> Result<MovieDetail, AppError> {
    let row = sqlx::query_as::<_, crate::models::db::movies::Movie>(
        "SELECT * FROM movies WHERE item_id = ?"
    ).bind(item_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Movie {} not found", item_id)))?;

    let studio = match row.studio_id {
        Some(sid) => sqlx::query_scalar::<_, String>("SELECT name FROM studios WHERE id = ?")
            .bind(sid).fetch_optional(pool).await?,
        None => None,
    };

    // Fetch the source file path and extract just the file name.
    let file_name: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT file_path FROM media_items WHERE id = ?"
    ).bind(item_id).fetch_optional(pool).await?
        .map(|p| std::path::Path::new(&p).file_name().unwrap_or_default().to_string_lossy().to_string());

    Ok(MovieDetail {
        id: row.item_id,
        title: row.title,
        year: row.year,
        plot: row.plot,
        tagline: row.tagline,
        runtime: row.runtime,
        rating: row.rating,
        age_rating: row.age_rating,
        studio,
        collection_name: row.collection_name,
        origin_country: row.origin_country,
        creator: row.creator,
        poster_url: row.poster_url,
        backdrop_url: row.backdrop_url,
        trailer_url: row.trailer_url,
        provider_ids: row.provider_ids,
        genres: item_genres(pool, item_id).await?,
        cast: item_credits(pool, item_id).await?,
        stream_url: stream_url(item_id),
        file_name,
    })
}

async fn music_video_detail(pool: &SqlitePool, item_id: i64) -> Result<MusicVideoDetail, AppError> {
    let row = sqlx::query_as::<_, crate::models::db::music_videos::MusicVideo>(
        "SELECT * FROM music_videos WHERE item_id = ?"
    ).bind(item_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Music video {} not found", item_id)))?;

    Ok(MusicVideoDetail {
        id: row.item_id,
        title: row.title,
        artist: row.artist_name,
        year: row.year,
        plot: row.plot,
        poster_url: row.poster_url,
        runtime: row.runtime,
        genres: item_genres(pool, item_id).await?,
        stream_url: stream_url(item_id),
    })
}

/// Series cards, optionally restricted to one library.
async fn series_cards(pool: &SqlitePool, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
    let cards = match library_id {
        Some(id) => sqlx::query_as::<_, Card>(
            "SELECT id, 'series' AS kind, name AS title, poster_url, year, NULL AS stream_url
             FROM series WHERE library_id = ? ORDER BY name"
        ).bind(id).fetch_all(pool).await?,
        None => sqlx::query_as::<_, Card>(
            "SELECT id, 'series' AS kind, name AS title, poster_url, year, NULL AS stream_url
             FROM series ORDER BY name"
        ).fetch_all(pool).await?,
    };
    Ok(cards)
}

async fn season_list(pool: &SqlitePool, series_id: i64) -> Result<Vec<SeasonDto>, AppError> {
    let rows = sqlx::query_as::<_, (i64, i64, i64, Option<String>)>(
        "SELECT se.id, se.season_number,
                (SELECT COUNT(*) FROM episodes e WHERE e.season_id = se.id) AS episode_count,
                se.poster_url
         FROM seasons se WHERE se.series_id = ? ORDER BY se.season_number"
    ).bind(series_id).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|(id, season_number, episode_count, poster_url)| SeasonDto {
        id, season_number, episode_count, poster_url,
    }).collect())
}

async fn series_detail(pool: &SqlitePool, series_id: i64) -> Result<SeriesDetail, AppError> {
    let s = sqlx::query_as::<_, crate::models::db::series::Series>(
        "SELECT * FROM series WHERE id = ?"
    ).bind(series_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Series {} not found", series_id)))?;

    let studio = match s.studio_id {
        Some(sid) => sqlx::query_scalar::<_, String>("SELECT name FROM studios WHERE id = ?")
            .bind(sid).fetch_optional(pool).await?,
        None => None,
    };

    Ok(SeriesDetail {
        id: s.id,
        name: s.name,
        year: s.year,
        plot: s.plot,
        poster_url: s.poster_url,
        backdrop_url: s.backdrop_url,
        rating: s.rating,
        age_rating: s.age_rating,
        studio,
        trailer_url: s.trailer_url,
        collection_name: s.collection_name,
        origin_country: s.origin_country,
        creator: s.creator,
        provider_ids: s.provider_ids,
        genres: series_genres(pool, series_id).await?,
        tags: series_tags(pool, series_id).await?,
        cast: series_credits(pool, series_id).await?,
        seasons: season_list(pool, series_id).await?,
    })
}

async fn series_seasons(pool: &SqlitePool, series_id: i64) -> Result<Vec<SeasonDto>, AppError> {
    season_list(pool, series_id).await
}

async fn season_episodes(pool: &SqlitePool, series_id: i64, season_number: i64) -> Result<Vec<EpisodeDto>, AppError> {
    let series_name = sqlx::query_scalar::<_, String>("SELECT name FROM series WHERE id = ?")
        .bind(series_id).fetch_optional(pool).await?;

    let rows = sqlx::query_as::<_, crate::models::db::episodes::Episode>(
        "SELECT e.* FROM episodes e
         JOIN seasons se ON se.id = e.season_id
         WHERE se.series_id = ? AND se.season_number = ?
         ORDER BY e.episode_number"
    ).bind(series_id).bind(season_number).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|e| EpisodeDto {
        id: e.item_id,
        series_id: Some(series_id),
        series_name: series_name.clone(),
        season_number: Some(season_number),
        episode_number: e.episode_number,
        title: e.title,
        plot: e.plot,
        still_url: e.still_url,
        runtime: e.runtime,
        air_date: e.air_date,
        stream_url: stream_url(e.item_id),
    }).collect())
}

async fn episode_detail(pool: &SqlitePool, item_id: i64) -> Result<EpisodeDto, AppError> {
    let row = sqlx::query_as::<_, (Option<i64>, Option<String>, Option<i64>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>)>(
        "SELECT s.id, s.name, se.season_number, e.episode_number, e.title, e.plot, e.still_url, e.runtime, e.air_date
         FROM episodes e
         LEFT JOIN seasons se ON se.id = e.season_id
         LEFT JOIN series s ON s.id = se.series_id
         WHERE e.item_id = ?"
    ).bind(item_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Episode {} not found", item_id)))?;

    let (series_id, series_name, season_number, episode_number, title, plot, still_url, runtime, air_date) = row;
    Ok(EpisodeDto {
        id: item_id,
        series_id,
        series_name,
        season_number,
        episode_number,
        title,
        plot,
        still_url,
        runtime,
        air_date,
        stream_url: stream_url(item_id),
    })
}

async fn book_detail(pool: &SqlitePool, item_id: i64) -> Result<BookDetail, AppError> {
    let b = sqlx::query_as::<_, crate::models::db::books::Book>(
        &format!("SELECT {} FROM media_items mi JOIN books b ON b.item_id = mi.id WHERE mi.id = ?",
            crate::models::db::books::BOOK_SELECT)
    ).bind(item_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Book {} not found", item_id)))?;

    Ok(BookDetail {
        id: b.item_id,
        title: b.title,
        plot: b.plot,
        poster_url: b.poster_url,
        page_count: b.page_count,
        reading_mode: b.reading_mode,
        publisher: b.publisher,
        published_date: b.published_date,
        isbn: b.isbn,
    })
}

// ── Music reads ────────────────────────────────────────────────────────────

/// Album cards, optionally restricted to one artist.
async fn artist_albums(pool: &SqlitePool, artist_id: i64) -> Result<Vec<Card>, AppError> {
    Ok(sqlx::query_as::<_, Card>(
        "SELECT id, 'album' AS kind, title, cover_url AS poster_url, year, NULL AS stream_url
         FROM albums WHERE artist_id = ? ORDER BY year, title"
    ).bind(artist_id).fetch_all(pool).await?)
}

/// Artist cards, optionally restricted to one library.
async fn artist_cards(pool: &SqlitePool, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
    let cards = match library_id {
        Some(id) => sqlx::query_as::<_, Card>(
            "SELECT id, 'artist' AS kind, name AS title, image_url AS poster_url, NULL AS year, NULL AS stream_url
             FROM artists WHERE library_id = ? ORDER BY name"
        ).bind(id).fetch_all(pool).await?,
        None => sqlx::query_as::<_, Card>(
            "SELECT id, 'artist' AS kind, name AS title, image_url AS poster_url, NULL AS year, NULL AS stream_url
             FROM artists ORDER BY name"
        ).fetch_all(pool).await?,
    };
    Ok(cards)
}

/// All tracks in a music library, ordered by artist → album → disc/track.
async fn library_tracks(pool: &SqlitePool, library_id: i64) -> Result<Vec<TrackDto>, AppError> {
    let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
        "SELECT mi.id, t.track_number, t.disc_number, t.title, ar.name AS artist, al.title AS album, al.cover_url, t.duration
         FROM media_items mi
         JOIN tracks t ON t.item_id = mi.id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = t.artist_id
         WHERE mi.library_id = ?
         ORDER BY ar.name, al.title, COALESCE(t.disc_number, 1), COALESCE(t.track_number, 9999), t.title"
    ).bind(library_id).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|(id, track_number, disc_number, title, artist, album, cover_url, duration)| TrackDto {
        id, track_number, disc_number, title, artist, album, cover_url, duration,
        stream_url: stream_url(id),
    }).collect())
}

async fn artist_detail(pool: &SqlitePool, artist_id: i64) -> Result<ArtistDetail, AppError> {
    let a = sqlx::query_as::<_, crate::models::db::artists::Artist>(
        "SELECT * FROM artists WHERE id = ?"
    ).bind(artist_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Artist {} not found", artist_id)))?;

    Ok(ArtistDetail {
        id: a.id,
        name: a.name,
        bio: a.bio,
        image_url: a.image_url,
        albums: artist_albums(pool, artist_id).await?,
    })
}

async fn album_detail(pool: &SqlitePool, album_id: i64) -> Result<AlbumDetail, AppError> {
    let al = sqlx::query_as::<_, crate::models::db::albums::Album>(
        "SELECT * FROM albums WHERE id = ?"
    ).bind(album_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Album {} not found", album_id)))?;

    let artist = match al.artist_id {
        Some(aid) => sqlx::query_scalar::<_, String>("SELECT name FROM artists WHERE id = ?")
            .bind(aid).fetch_optional(pool).await?,
        None => None,
    };

    let tracks = sqlx::query_as::<_, crate::models::db::tracks::Track>(
        "SELECT t.* FROM tracks t WHERE t.album_id = ?
         ORDER BY COALESCE(t.disc_number, 1), COALESCE(t.track_number, 9999), t.title"
    ).bind(album_id).fetch_all(pool).await?;

    let album_title = al.title.clone();
    let cover = al.cover_url.clone();
    Ok(AlbumDetail {
        id: al.id,
        title: al.title,
        artist_id: al.artist_id,
        artist: artist.clone(),
        year: al.year,
        cover_url: al.cover_url,
        tracks: tracks.into_iter().map(|t| TrackDto {
            id: t.item_id,
            track_number: t.track_number,
            disc_number: t.disc_number,
            title: t.title,
            artist: artist.clone(),
            album: Some(album_title.clone()),
            cover_url: cover.clone(),
            duration: t.duration,
            stream_url: stream_url(t.item_id),
        }).collect(),
    })
}

// ── Images: galleries → images ───────────────────────────────────────────────

/// Gallery cards, optionally restricted to one library.
async fn gallery_cards(pool: &SqlitePool, library_id: Option<i64>) -> Result<Vec<Card>, AppError> {
    let mut cards = match library_id {
        Some(id) => sqlx::query_as::<_, Card>(
            "SELECT id, 'gallery' AS kind, name AS title, cover_url AS poster_url,
                    NULL AS year, NULL AS stream_url
             FROM galleries WHERE library_id = ? ORDER BY COALESCE(taken_at, '') DESC, name"
        ).bind(id).fetch_all(pool).await?,
        None => sqlx::query_as::<_, Card>(
            "SELECT id, 'gallery' AS kind, name AS title, cover_url AS poster_url,
                    NULL AS year, NULL AS stream_url
             FROM galleries ORDER BY COALESCE(taken_at, '') DESC, name"
        ).fetch_all(pool).await?,
    };

    // Attach up to 4 thumbnails per gallery so the client can render a mosaic
    // collage card. One windowed query covers every gallery in the result set.
    if !cards.is_empty() {
        let ids: Vec<i64> = cards.iter().map(|c| c.id).collect();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT gallery_id, item_id FROM (
                 SELECT i.gallery_id AS gallery_id, mi.id AS item_id,
                        ROW_NUMBER() OVER (
                            PARTITION BY i.gallery_id
                            ORDER BY COALESCE(i.taken_at, ''), mi.file_path
                        ) AS rn
                 FROM media_items mi JOIN images i ON i.item_id = mi.id
                 WHERE i.gallery_id IN ({})
             ) WHERE rn <= 4
             ORDER BY gallery_id, rn",
            placeholders
        );
        let mut q = sqlx::query_as::<_, (i64, i64)>(&sql);
        for id in &ids {
            q = q.bind(id);
        }
        let rows = q.fetch_all(pool).await?;

        use std::collections::HashMap;
        let mut by_gallery: HashMap<i64, Vec<String>> = HashMap::new();
        for (gallery_id, item_id) in rows {
            by_gallery.entry(gallery_id).or_default().push(image_thumb_url(item_id));
        }
        for card in &mut cards {
            if let Some(thumbs) = by_gallery.remove(&card.id) {
                card.thumbs = thumbs;
            }
        }
    }

    Ok(cards)
}

/// Every photo in an Images library, across all its albums (and unsorted ones).
/// Used by the album "Add Photos" picker to choose existing photos to move in.
async fn library_images(pool: &SqlitePool, library_id: i64) -> Result<Vec<ImageDto>, AppError> {
    let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT mi.id, i.gallery_id, i.title, i.taken_at, i.width, i.height
         FROM media_items mi JOIN images i ON i.item_id = mi.id
         WHERE mi.library_id = ? AND mi.item_type = 'image' AND i.deleted_at IS NULL
         ORDER BY COALESCE(i.taken_at, ''), mi.file_path"
    ).bind(library_id).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|(id, gallery_id, title, taken_at, width, height)| ImageDto {
        id, gallery_id, title, taken_at, width, height,
        url: image_url(id),
        thumb_url: image_thumb_url(id),
    }).collect())
}

/// Photos in an Images library's recycle bin (removed from their album but not
/// yet permanently deleted), most-recently-trashed first.
async fn trashed_images(pool: &SqlitePool, library_id: i64) -> Result<Vec<ImageDto>, AppError> {
    let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT mi.id, i.prev_gallery_id, i.title, i.taken_at, i.width, i.height
         FROM media_items mi JOIN images i ON i.item_id = mi.id
         WHERE mi.library_id = ? AND mi.item_type = 'image' AND i.deleted_at IS NOT NULL
         ORDER BY i.deleted_at DESC, mi.file_path"
    ).bind(library_id).fetch_all(pool).await?;

    // gallery_id carries the album the photo would be restored to (its former home).
    Ok(rows.into_iter().map(|(id, gallery_id, title, taken_at, width, height)| ImageDto {
        id, gallery_id, title, taken_at, width, height,
        url: image_url(id),
        thumb_url: image_thumb_url(id),
    }).collect())
}

async fn gallery_detail(pool: &SqlitePool, gallery_id: i64) -> Result<GalleryDetail, AppError> {
    let g = sqlx::query_as::<_, crate::models::db::galleries::Gallery>(
        "SELECT * FROM galleries WHERE id = ?"
    ).bind(gallery_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Gallery {} not found", gallery_id)))?;

    let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<String>, Option<i64>, Option<i64>)>(
        "SELECT mi.id, i.gallery_id, i.title, i.taken_at, i.width, i.height
         FROM media_items mi JOIN images i ON i.item_id = mi.id
         WHERE i.gallery_id = ?
         ORDER BY COALESCE(i.taken_at, ''), mi.file_path"
    ).bind(gallery_id).fetch_all(pool).await?;

    let images: Vec<ImageDto> = rows.into_iter().map(|(id, gallery_id, title, taken_at, width, height)| ImageDto {
        id, gallery_id, title, taken_at, width, height,
        url: image_url(id),
        thumb_url: image_thumb_url(id),
    }).collect();

    Ok(GalleryDetail {
        id: g.id,
        library_id: g.library_id,
        name: g.name,
        description: g.description,
        cover_url: g.cover_url,
        taken_at: g.taken_at,
        image_count: images.len() as i64,
        images,
    })
}

async fn image_detail(pool: &SqlitePool, item_id: i64) -> Result<ImageDetail, AppError> {
    let img = sqlx::query_as::<_, crate::models::db::images::Image>(
        &format!("SELECT {} FROM media_items mi JOIN images i ON i.item_id = mi.id WHERE mi.id = ?",
            crate::models::db::images::IMAGE_SELECT)
    ).bind(item_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Image {} not found", item_id)))?;

    let file_name = std::path::Path::new(&img.file_path)
        .file_name().map(|n| n.to_string_lossy().to_string());

    Ok(ImageDetail {
        id: img.item_id,
        gallery_id: img.gallery_id,
        title: img.title,
        taken_at: img.taken_at,
        width: img.width,
        height: img.height,
        camera_make: img.camera_make,
        camera_model: img.camera_model,
        lens: img.lens,
        iso: img.iso,
        focal_length: img.focal_length,
        aperture: img.aperture,
        gps_lat: img.gps_lat,
        gps_lon: img.gps_lon,
        orientation: img.orientation,
        url: image_url(item_id),
        thumb_url: image_thumb_url(item_id),
        file_name,
    })
}

/// Enriched tracks for a playlist, in order.
async fn playlist_tracks(pool: &SqlitePool, playlist_id: i64) -> Result<Vec<TrackDto>, AppError> {
    let rows = sqlx::query_as::<_, (i64, Option<i64>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<i64>)>(
        "SELECT mi.id, t.track_number, t.disc_number, t.title, ar.name AS artist, al.title AS album, al.cover_url, t.duration
         FROM playlist_tracks pt
         JOIN media_items mi ON mi.id = pt.item_id
         JOIN tracks t ON t.item_id = mi.id
         LEFT JOIN albums al ON al.id = t.album_id
         LEFT JOIN artists ar ON ar.id = t.artist_id
         WHERE pt.playlist_id = ?
         ORDER BY pt.position"
    ).bind(playlist_id).fetch_all(pool).await?;

    Ok(rows.into_iter().map(|(id, track_number, disc_number, title, artist, album, cover_url, duration)| TrackDto {
        id, track_number, disc_number, title, artist, album, cover_url, duration,
        stream_url: stream_url(id),
    }).collect())
}

/// Look up the provider id stored on a movie or series, for metadata refresh.
async fn movie_provider_lookup(pool: &SqlitePool, item_id: i64) -> Result<(Option<String>, Option<String>), AppError> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT title, provider_ids FROM movies WHERE item_id = ?")
            .bind(item_id).fetch_optional(pool).await?;
    Ok(row.unwrap_or((None, None)))
}

/// The on-disk path backing a media item, if it exists.
async fn file_path(pool: &SqlitePool, id: i64) -> Result<Option<String>, AppError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media_items WHERE id = ?")
        .bind(id).fetch_optional(pool).await?;
    Ok(row.map(|r| r.0))
}

/// Like [`file_path`] but errors with `NotFound` when the item is missing.
async fn require_file_path(pool: &SqlitePool, id: i64) -> Result<String, AppError> {
    file_path(pool, id).await?
        .ok_or_else(|| AppError::NotFound("Media not found".to_string()))
}

/// Source images for thumbnail generation: `(file_path, poster_url, backdrop_url)`.
async fn thumbnail_sources(pool: &SqlitePool, id: i64) -> Result<Option<(String, Option<String>, Option<String>)>, AppError> {
    Ok(sqlx::query_as(
        "SELECT mi.file_path,
                COALESCE(mv.poster_url, e.still_url, mvd.poster_url) AS poster_url,
                mv.backdrop_url AS backdrop_url
         FROM media_items mi
         LEFT JOIN movies mv ON mv.item_id = mi.id
         LEFT JOIN episodes e ON e.item_id = mi.id
         LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
         WHERE mi.id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?)
}

/// The `item_type` discriminator for a media item, erroring if it doesn't exist.
async fn item_type(pool: &SqlitePool, id: i64) -> Result<String, AppError> {
    let row: Option<(String,)> = sqlx::query_as("SELECT item_type FROM media_items WHERE id = ?")
        .bind(id).fetch_optional(pool).await?;
    Ok(row.ok_or_else(|| AppError::NotFound(format!("Media {} not found", id)))?.0)
}

/// The album a track belongs to (if any).
async fn track_album_id(pool: &SqlitePool, item_id: i64) -> Result<Option<i64>, AppError> {
    let row: Option<(Option<i64>,)> = sqlx::query_as("SELECT album_id FROM tracks WHERE item_id = ?")
        .bind(item_id).fetch_optional(pool).await?;
    Ok(row.and_then(|r| r.0))
}

/// A series' display name and stored `provider_ids` JSON, erroring if missing.
async fn series_provider_lookup(pool: &SqlitePool, series_id: i64) -> Result<(String, Option<String>), AppError> {
    sqlx::query_as("SELECT name, provider_ids FROM series WHERE id = ?")
        .bind(series_id).fetch_optional(pool).await?
        .ok_or_else(|| AppError::NotFound(format!("Series {} not found", series_id)))
}

/// Resolve the `media_items` id for a specific episode within a series.
async fn episode_item_id(pool: &SqlitePool, series_id: i64, season_number: i64, episode_number: i64) -> Result<Option<i64>, AppError> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT e.item_id FROM episodes e JOIN seasons se ON se.id = e.season_id
         WHERE se.series_id = ? AND se.season_number = ? AND e.episode_number = ?"
    )
    .bind(series_id).bind(season_number).bind(episode_number)
    .fetch_optional(pool).await?;
    Ok(row.map(|r| r.0))
}

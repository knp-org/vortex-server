use sqlx::SqlitePool;
use walkdir::WalkDir;
use std::path::Path;
use std::sync::OnceLock;
use regex::Regex;
use crate::models::db::libraries::{Library, LibraryType};
use crate::services::library_service::LibraryService;
use crate::services::catalog_service::CatalogService;
use std::path::PathBuf;
use crate::services::metadata_service::MetadataService;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};

// Static regex patterns - compiled once for performance
static SEASON_REGEX: OnceLock<Regex> = OnceLock::new();
static EPISODE_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

const VIDEO_EXTENSIONS: &[&str] = &["mp4", "mkv", "avi", "mov", "webm", "wmv", "m4v", "mpg", "mpeg", "flv", "ts"];
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "m4a", "m4b", "aac", "ogg", "oga", "opus", "wav", "wma", "alac", "aiff", "aif", "ape", "wv", "mpc"];

fn get_season_regex() -> &'static Regex {
    SEASON_REGEX.get_or_init(|| Regex::new(r"season\s*(\d+)").unwrap())
}

fn get_episode_patterns() -> &'static Vec<Regex> {
    EPISODE_PATTERNS.get_or_init(|| vec![
        Regex::new(r"s\d+e(\d+)").unwrap(),           // S01E05
        Regex::new(r"\d+x(\d+)").unwrap(),            // 1x05
        Regex::new(r"ep(?:isode)?\s*(\d+)").unwrap(), // Episode 5, Ep5
        Regex::new(r"[-\s](\d{1,3})[-\s]").unwrap(),  // - 05 -
        Regex::new(r"(?:^|\s)e(\d+)").unwrap(),       // E05
    ])
}

struct ScanCache {
    /// Series-level metadata, keyed by series name. `Option` caches failures too.
    series_metadata: HashMap<String, Option<NormalizedMetadata>>,
    /// Per-season episode lists, keyed by (provider series id, season number).
    season_episodes: HashMap<(String, i64), Option<Vec<EpisodeMetadata>>>,
    /// Movie metadata, keyed by file stem.
    movie_metadata: HashMap<String, Option<NormalizedMetadata>>,
}

impl ScanCache {
    fn new() -> Self {
        Self {
            series_metadata: HashMap::new(),
            season_episodes: HashMap::new(),
            movie_metadata: HashMap::new(),
        }
    }
}

pub async fn scan_media(pool: &SqlitePool, target_library_id: Option<i64>, force_refresh: bool) {
    let service = LibraryService::new(pool.clone());
    let libraries: Vec<Library> = if let Some(id) = target_library_id {
        service.get_by_id(id).await.map(|l| vec![l]).unwrap_or_default()
    } else {
        service.get_all().await.unwrap_or_default()
    };

    let cache = Arc::new(Mutex::new(ScanCache::new()));

    use futures::{StreamExt, stream};

    for library in libraries {
        tracing::info!(library = %library.name, library_type = ?library.library_type, "Scanning library");

        let is_books = library.library_type == LibraryType::Books;
        let is_music = library.library_type == LibraryType::Music;
        let is_images = library.library_type == LibraryType::Images;

        // Each entry is (file path, owning root path) so TV parsing can strip the
        // correct root when a library spans multiple folders.
        let mut paths: Vec<(PathBuf, String)> = Vec::new();
        for root in &library.paths {
            for entry in WalkDir::new(root) {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        if path.is_file() {
                            if let Some(ext) = path.extension() {
                                let ext_str = ext.to_string_lossy().to_lowercase();
                                let matches = if is_books {
                                    crate::services::books::BOOK_EXTENSIONS.contains(&ext_str.as_str())
                                } else if is_music {
                                    AUDIO_EXTENSIONS.contains(&ext_str.as_str())
                                } else if is_images {
                                    crate::services::images::IMAGE_EXTENSIONS.contains(&ext_str.as_str())
                                } else {
                                    VIDEO_EXTENSIONS.contains(&ext_str.as_str())
                                };
                                if matches {
                                    paths.push((path.to_path_buf(), root.clone()));
                                }
                            }
                        }
                    },
                    Err(e) => tracing::warn!(error = %e, "Error scanning entry"),
                }
            }
        }

        tracing::info!(file_count = paths.len(), library = %library.name, "Found files, processing with concurrency 4");

        let pool_ref = &pool;
        let cache_ref = &cache;
        let lib_ref = &library;

        stream::iter(paths)
            .for_each_concurrent(4, |(path, root)| async move {
                if is_books {
                    process_book(pool_ref, &path, &root, lib_ref, force_refresh).await;
                } else if is_music {
                    process_music(pool_ref, &path, lib_ref, force_refresh).await;
                } else if is_images {
                    process_image(pool_ref, &path, &root, lib_ref, force_refresh).await;
                } else {
                    process_video(pool_ref, &path, &root, lib_ref, force_refresh, cache_ref.clone()).await;
                }
            })
            .await;
    }

    cleanup_missing_files(pool, target_library_id).await;
}

/// Remove spine rows whose backing file has disappeared. The cascade clears the
/// per-type detail row, credits, genre links and user state automatically.
async fn cleanup_missing_files(pool: &SqlitePool, library_id: Option<i64>) {
    tracing::info!("Cleaning up missing files");

    let rows: Vec<(i64, String)> = if let Some(id) = library_id {
        sqlx::query_as("SELECT id, file_path FROM media_items WHERE library_id = ?")
            .bind(id).fetch_all(pool).await.unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, file_path FROM media_items")
            .fetch_all(pool).await.unwrap_or_default()
    };

    for (id, path_str) in rows {
        if !Path::new(&path_str).exists() {
            tracing::info!(file_path = %path_str, "Removing missing file from DB");
            let _ = CatalogService::new(pool.clone()).delete_item(id).await;
        }
    }

    // Prune orphaned grouping entities left behind after episode/track deletion.
    // The detail rows (episodes, tracks) were cascade-deleted with media_items,
    // but their parent series/seasons/albums/artists are independent tables.

    let orphaned_seasons = sqlx::query_scalar::<_, i64>(
        "SELECT se.id FROM seasons se
         LEFT JOIN episodes e ON e.season_id = se.id
         WHERE e.item_id IS NULL"
    ).fetch_all(pool).await.unwrap_or_default();
    for sid in &orphaned_seasons {
        tracing::info!(season_id = %sid, "Removing orphaned season");
        let _ = sqlx::query("DELETE FROM seasons WHERE id = ?").bind(sid).execute(pool).await;
    }

    let orphaned_series = sqlx::query_scalar::<_, i64>(
        "SELECT s.id FROM series s
         LEFT JOIN seasons se ON se.series_id = s.id
         WHERE se.id IS NULL"
    ).fetch_all(pool).await.unwrap_or_default();
    for sid in &orphaned_series {
        tracing::info!(series_id = %sid, "Removing orphaned series");
        let _ = sqlx::query("DELETE FROM series WHERE id = ?").bind(sid).execute(pool).await;
    }

    let orphaned_albums = sqlx::query_scalar::<_, i64>(
        "SELECT al.id FROM albums al
         LEFT JOIN tracks t ON t.album_id = al.id
         WHERE t.item_id IS NULL"
    ).fetch_all(pool).await.unwrap_or_default();
    for aid in &orphaned_albums {
        tracing::info!(album_id = %aid, "Removing orphaned album");
        let _ = sqlx::query("DELETE FROM albums WHERE id = ?").bind(aid).execute(pool).await;
    }

    let orphaned_artists = sqlx::query_scalar::<_, i64>(
        "SELECT ar.id FROM artists ar
         LEFT JOIN albums al ON al.artist_id = ar.id
         WHERE al.id IS NULL"
    ).fetch_all(pool).await.unwrap_or_default();
    for aid in &orphaned_artists {
        tracing::info!(artist_id = %aid, "Removing orphaned artist");
        let _ = sqlx::query("DELETE FROM artists WHERE id = ?").bind(aid).execute(pool).await;
    }

    let orphaned_galleries = sqlx::query_scalar::<_, i64>(
        "SELECT g.id FROM galleries g
         LEFT JOIN images i ON i.gallery_id = g.id
         WHERE i.item_id IS NULL"
    ).fetch_all(pool).await.unwrap_or_default();
    for gid in &orphaned_galleries {
        tracing::info!(gallery_id = %gid, "Removing orphaned gallery");
        let _ = sqlx::query("DELETE FROM galleries WHERE id = ?").bind(gid).execute(pool).await;
    }
}

fn parse_tv_show_info(path: &Path, library_path: &str, library_name: &str) -> Option<(String, i64, i64)> {
    let relative = path.strip_prefix(library_path).ok()?;
    let components: Vec<_> = relative.components().collect();

    if components.is_empty() { return None; }

    let filename = path.file_stem()?.to_string_lossy().to_string();
    let episode_number = parse_episode_number(&filename).unwrap_or(1);

    if components.len() >= 3 {
        let series_name = components[0].as_os_str().to_string_lossy().to_string();
        let season_folder = components[1].as_os_str().to_string_lossy().to_lowercase();
        let season_number = get_season_regex().captures(&season_folder)
            .and_then(|c| c.get(1)?.as_str().parse().ok()).unwrap_or(1);
        return Some((series_name, season_number, episode_number));
    }

    if components.len() == 2 {
        let series_name = components[0].as_os_str().to_string_lossy().to_string();
        return Some((series_name, 1, episode_number));
    }

    if components.len() == 1 {
        return Some((library_name.to_string(), 1, episode_number));
    }

    None
}

fn parse_episode_number(filename: &str) -> Option<i64> {
    let lower = filename.to_lowercase();
    for pattern in get_episode_patterns() {
        if let Some(caps) = pattern.captures(&lower) {
            if let Some(num) = caps.get(1).and_then(|m| m.as_str().parse().ok()) {
                return Some(num);
            }
        }
    }
    None
}

/// Ingest a single book file (pdf/cbz/epub) into `media_items` + `books`.
/// True if a media item with this file path already exists in the DB.
async fn item_exists(pool: &SqlitePool, file_path: &str) -> bool {
    sqlx::query_as::<_, (i64,)>("SELECT id FROM media_items WHERE file_path = ?")
        .bind(file_path)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .is_some()
}

async fn process_book(pool: &SqlitePool, path: &Path, root_path: &str, library: &Library, force_refresh: bool) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

    // Fast scan: only ingest new files. Skip files already in the library so
    // manual metadata edits are preserved. A full refresh re-applies everything.
    if !force_refresh && item_exists(pool, &path_str).await {
        return;
    }

    let page_count: Option<i64> = match crate::services::books::detect(&path_str) {
        Some(crate::services::books::BookFormat::Cbz) => {
            match crate::services::books::cbz_page_count(&path_str).await {
                Ok(n) => Some(n as i64),
                Err(e) => {
                    tracing::warn!(file = %file_stem, error = %e, "Failed to count CBZ pages");
                    None
                }
            }
        }
        _ => None,
    };

    let mut book_series_id: Option<i64> = None;
    let mut chapter_number: Option<f64> = None;

    if let Some((series_name, _, ep_num)) = parse_tv_show_info(path, root_path, &library.name) {
        // If parse_tv_show_info returned a series name that differs from the library name, it means it's in a folder.
        if series_name != library.name {
            if let Ok(id) = CatalogService::get_or_create_book_series(pool, library.id, &series_name).await {
                book_series_id = Some(id);
                chapter_number = Some(ep_num as f64);
            }
        }
    }

    let item_id = match CatalogService::new(pool.clone()).upsert_item(library.id, "book", &path_str).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(file = %file_stem, error = %e, "Failed to upsert book item"); return; }
    };
    if let Err(e) = CatalogService::new(pool.clone()).upsert_book(item_id, &file_stem, page_count, book_series_id, chapter_number).await {
        tracing::warn!(file = %file_stem, error = %e, "Failed to upsert book");
    }
}

/// Ingest a single photo into `media_items` + `images`, grouping it into a gallery
/// derived from its containing folder (or the library name when it sits at the root).
async fn process_image(pool: &SqlitePool, path: &Path, root_path: &str, library: &Library, force_refresh: bool) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

    // Fast scan: only ingest new files. Skip photos already in the library so
    // manual title/gallery edits are preserved. A full refresh re-reads EXIF.
    if !force_refresh && item_exists(pool, &path_str).await {
        return;
    }

    // Gallery = the immediate parent folder name. When the photo sits directly in
    // a scan root, use that root folder's own name (e.g. ".../Wallpapers" -> the
    // "Wallpapers" album), falling back to the library name only if the root has
    // no nameable component.
    let gallery_name = path.parent()
        .filter(|p| p.to_string_lossy() != root_path)
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .or_else(|| Path::new(root_path).file_name().map(|n| n.to_string_lossy().to_string()))
        .unwrap_or_else(|| library.name.clone());

    let p = path_str.clone();
    let exif = tokio::task::spawn_blocking(move || crate::services::images::read_exif(&p))
        .await
        .unwrap_or_default();

    let catalog = CatalogService::new(pool.clone());

    let gallery_id = match catalog.get_or_create_gallery(library.id, &gallery_name).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(gallery = %gallery_name, error = %e, "Failed to get/create gallery"); return; }
    };
    let item_id = match catalog.upsert_item(library.id, "image", &path_str).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(file = %file_stem, error = %e, "Failed to upsert image item"); return; }
    };
    if let Err(e) = catalog.upsert_image(item_id, Some(gallery_id), &file_stem, &exif).await {
        tracing::warn!(file = %file_stem, error = %e, "Failed to upsert image");
        return;
    }

    // First photo in the gallery becomes its cover; track the earliest capture date.
    let _ = catalog.set_gallery_cover_if_empty(gallery_id, &format!("/api/v1/media/{}/thumbnail", item_id)).await;
    let _ = catalog.min_gallery_taken_at(gallery_id, exif.taken_at.as_deref()).await;
}

/// Extracted audio tags (read off-thread via lofty).
#[derive(Default)]
struct AudioTags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    track: Option<i64>,
    disc: Option<i64>,
    year: Option<i64>,
    duration: Option<i64>,
    /// Embedded cover art as (bytes, file extension).
    cover: Option<(Vec<u8>, String)>,
}

/// Read tags from an audio file (blocking; call via `spawn_blocking`).
fn read_audio_tags(path: &str) -> AudioTags {
    use lofty::prelude::*;
    use lofty::tag::ItemKey;
    let mut out = AudioTags::default();

    let tagged = match lofty::read_from_path(path) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(file = %path, error = %e, "Failed to read audio tags");
            return out;
        }
    };

    out.duration = Some(tagged.properties().duration().as_secs() as i64);

    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        out.title = tag.title().map(|s| s.to_string());
        out.artist = tag.artist().map(|s| s.to_string());
        out.album = tag.album().map(|s| s.to_string());
        out.track = tag.track().map(|n| n as i64);
        out.disc = tag.disk().map(|n| n as i64);
        // lofty 0.24 has no `year()` accessor; read the Year/RecordingDate string.
        out.year = tag.get_string(ItemKey::Year)
            .or_else(|| tag.get_string(ItemKey::RecordingDate))
            .and_then(|s| s.get(0..4).and_then(|y| y.parse::<i64>().ok()));

        if let Some(pic) = tag.pictures().first() {
            let ext = match pic.mime_type().map(|m| m.as_str()) {
                Some("image/png") => "png",
                _ => "jpg",
            };
            out.cover = Some((pic.data().to_vec(), ext.to_string()));
        }
    }

    out
}

/// Ingest a single audio file into `media_items` + `tracks`, building the
/// artist → album → track hierarchy from its tags.
async fn process_music(pool: &SqlitePool, path: &Path, library: &Library, force_refresh: bool) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

    // Fast scan: only ingest new files. Skip tracks already in the library so
    // manual metadata edits are preserved. A full refresh re-applies tag data.
    if !force_refresh && item_exists(pool, &path_str).await {
        return;
    }

    let p = path_str.clone();
    let tags = tokio::task::spawn_blocking(move || read_audio_tags(&p))
        .await
        .unwrap_or_default();

    let artist_name = tags.artist.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Unknown Artist")
        .to_string();

    // Album falls back to the containing folder name, then "Unknown Album".
    let album_title = tags.album.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            path.parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Unknown Album".to_string())
        });

    let track_title = tags.title.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or(file_stem);

    let artist_id = match CatalogService::new(pool.clone()).get_or_create_artist(library.id, &artist_name).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(error = %e, "Failed to get/create artist"); return; }
    };
    let album_id = match CatalogService::new(pool.clone()).get_or_create_album(artist_id, library.id, &album_title, tags.year).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(error = %e, "Failed to get/create album"); return; }
    };
    let item_id = match CatalogService::new(pool.clone()).upsert_item(library.id, "track", &path_str).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(error = %e, "Failed to upsert track item"); return; }
    };
    if let Err(e) = CatalogService::new(pool.clone()).upsert_track(item_id, album_id, artist_id, tags.track, tags.disc, &track_title, tags.duration).await {
        tracing::warn!(error = %e, "Failed to upsert track");
    }

    // Save embedded album art once per album (best-effort).
    if let Some((bytes, ext)) = tags.cover {
        let cfg = crate::infrastructure::config::config();
        let dir = cfg.data_dir.join("thumbnails");
        if !dir.exists() { let _ = std::fs::create_dir(&dir); }
        let fname = format!("album_{}.{}", album_id, ext);
        if tokio::fs::write(dir.join(&fname), &bytes).await.is_ok() {
            let _ = CatalogService::new(pool.clone()).set_album_cover_if_empty(album_id, &format!("/api/v1/images/{}", fname)).await;
        }
    }
}

async fn process_video(pool: &SqlitePool, path: &Path, root_path: &str, library: &Library, force_refresh: bool, cache: Arc<Mutex<ScanCache>>) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

    let item_type = match library.library_type {
        LibraryType::TvShows => "episode",
        LibraryType::MusicVideos => "music_video",
        _ => "movie",
    };

    let item_id = match CatalogService::new(pool.clone()).upsert_item(library.id, item_type, &path_str).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(file = %file_stem, error = %e, "Failed to upsert media item"); return; }
    };

    match library.library_type {
        LibraryType::MusicVideos => {
            let _ = CatalogService::new(pool.clone()).upsert_music_video(item_id, &file_stem).await;
            return;
        }
        LibraryType::Other => {
            let _ = CatalogService::new(pool.clone()).ensure_movie_stub(item_id, &file_stem).await;
            return;
        }
        LibraryType::TvShows => {
            process_episode(pool, item_id, path, root_path, library, force_refresh, cache).await;
            return;
        }
        _ => {} // Movies and anything else fall through to the movie path
    }

    process_movie(pool, item_id, &file_stem, force_refresh, cache).await;
}

async fn process_movie(pool: &SqlitePool, item_id: i64, file_stem: &str, force_refresh: bool, cache: Arc<Mutex<ScanCache>>) {
    // Skip enrichment if we already have a populated movie row and aren't forcing.
    if !force_refresh {
        let has_meta: Option<(Option<String>,)> = sqlx::query_as("SELECT poster_url FROM movies WHERE item_id = ?")
            .bind(item_id).fetch_optional(pool).await.unwrap_or(None);
        if let Some((Some(_),)) = has_meta { return; }
    }

    let meta = {
        let c = cache.lock().await;
        if let Some(cached) = c.movie_metadata.get(file_stem) {
            cached.clone()
        } else {
            drop(c);
            let fetched = MetadataService::new(pool.clone()).fetch_metadata(file_stem, Some("movie")).await.ok();
            cache.lock().await.movie_metadata.insert(file_stem.to_string(), fetched.clone());
            fetched
        }
    };

    if let Some(meta) = meta {
        if let Err(e) = CatalogService::new(pool.clone()).apply_movie_metadata(item_id, &meta).await {
            tracing::warn!(file = %file_stem, error = %e, "Failed to apply movie metadata");
        }
    } else {
        tracing::warn!(search_term = %file_stem, "Failed to fetch movie metadata");
        let _ = CatalogService::new(pool.clone()).ensure_movie_stub(item_id, file_stem).await;
    }
}

async fn process_episode(pool: &SqlitePool, item_id: i64, path: &Path, root_path: &str, library: &Library, force_refresh: bool, cache: Arc<Mutex<ScanCache>>) {
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

    let (series_name, season_number, episode_number) = match parse_tv_show_info(path, root_path, &library.name) {
        Some(t) => t,
        None => (library.name.clone(), 1, 1),
    };

    let series_id = match CatalogService::new(pool.clone()).get_or_create_series(library.id, &series_name).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(error = %e, "Failed to get/create series"); return; }
    };
    let season_id = match CatalogService::new(pool.clone()).get_or_create_season(series_id, season_number).await {
        Ok(id) => id,
        Err(e) => { tracing::warn!(error = %e, "Failed to get/create season"); return; }
    };
    // upsert_episode keeps the hierarchy/episode number current but never
    // overwrites an existing title, so it's safe on every scan.
    if let Err(e) = CatalogService::new(pool.clone()).upsert_episode(item_id, season_id, episode_number, &file_stem).await {
        tracing::warn!(error = %e, "Failed to upsert episode");
        return;
    }

    // Fast scan: skip re-fetching/re-applying provider metadata once this
    // episode is already populated, so manual title/plot edits survive.
    if !force_refresh {
        let has_meta: Option<(Option<String>,)> = sqlx::query_as("SELECT plot FROM episodes WHERE item_id = ?")
            .bind(item_id).fetch_optional(pool).await.unwrap_or(None);
        if let Some((Some(plot),)) = has_meta {
            if !plot.is_empty() { return; }
        }
    }

    // Series-level metadata (cached per series name).
    let meta = {
        let c = cache.lock().await;
        if let Some(cached) = c.series_metadata.get(&series_name) {
            cached.clone()
        } else {
            drop(c);
            let fetched = MetadataService::new(pool.clone()).fetch_metadata(&series_name, Some("series")).await.ok();
            cache.lock().await.series_metadata.insert(series_name.clone(), fetched.clone());
            fetched
        }
    };

    let Some(meta) = meta else {
        tracing::warn!(series = %series_name, "Failed to fetch series metadata");
        return;
    };

    // Only (re)apply series-level metadata when forcing or the series isn't
    // populated yet — otherwise a new episode would clobber series edits.
    let apply_series = if force_refresh {
        true
    } else {
        let has_meta: Option<(Option<String>,)> = sqlx::query_as("SELECT poster_url FROM series WHERE id = ?")
            .bind(series_id).fetch_optional(pool).await.unwrap_or(None);
        !matches!(has_meta, Some((Some(_),)))
    };
    if apply_series {
        let _ = CatalogService::new(pool.clone()).apply_series_metadata(series_id, &meta).await;
    }

    // Episode-specific details, via the provider's per-season episode list.
    let provider_name = MetadataService::new(pool.clone()).get_default_provider().await;
    let Some(provider_id) = meta.provider_ids.as_ref()
        .and_then(|ids| ids.get(&provider_name))
        .and_then(|v| v.as_i64())
    else { return; };

    let id_str = provider_id.to_string();
    let episodes = {
        let c = cache.lock().await;
        if let Some(cached) = c.season_episodes.get(&(id_str.clone(), season_number)) {
            cached.clone()
        } else {
            drop(c);
            let fetched = MetadataService::new(pool.clone()).fetch_episodes(&id_str, season_number as i32, None).await.ok();
            cache.lock().await.season_episodes.insert((id_str.clone(), season_number), fetched.clone());
            fetched
        }
    };

    if let Some(episodes) = episodes {
        if let Some(ep) = episodes.iter().find(|e| e.episode_number as i64 == episode_number) {
            let _ = CatalogService::new(pool.clone()).apply_episode_details(item_id, &ep.name, &ep.overview, ep.still_path.clone()).await;
        }
    }
}

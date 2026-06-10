use sqlx::SqlitePool;
use walkdir::WalkDir;
use std::path::Path;
use std::sync::OnceLock;
use regex::Regex;
use crate::models::db::library::{Library, LibraryType};
use crate::services::library_service::LibraryService;
use std::path::PathBuf;
use crate::services::metadata::{fetch_metadata, fetch_episodes, get_default_provider};
use crate::services::transcode::codecs::probe_media; 
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::models::metadata::{NormalizedMetadata, EpisodeMetadata};

// Static regex patterns - compiled once for performance
static SEASON_REGEX: OnceLock<Regex> = OnceLock::new();
static EPISODE_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

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
    series_metadata: HashMap<String, Option<NormalizedMetadata>>, // Key: Series Name. Option to cache failures too.
    season_episodes: HashMap<(String, i32), Option<Vec<EpisodeMetadata>>>, // Key: (ProviderID, SeasonNum)
}

impl ScanCache {
    fn new() -> Self {
        Self {
            series_metadata: HashMap::new(),
            season_episodes: HashMap::new(),
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
                                } else {
                                    ["mp4", "mkv", "avi", "mov", "webm", "wmv", "m4v", "mpg", "mpeg", "flv", "ts"].contains(&ext_str.as_str())
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
                    process_book(pool_ref, &path, lib_ref).await;
                } else {
                    process_video(pool_ref, &path, &root, lib_ref, force_refresh, cache_ref.clone()).await;
                }
            })
            .await;
    }
    
    cleanup_missing_files(pool, target_library_id).await;
}

async fn cleanup_missing_files(pool: &SqlitePool, library_id: Option<i64>) {
    tracing::info!("Cleaning up missing files");
    
    let rows: Vec<(i64, String)> = if let Some(id) = library_id {
        sqlx::query_as("SELECT id, file_path FROM media WHERE library_id = ?")
            .bind(id)
            .fetch_all(pool)
            .await
            .unwrap_or_default()
    } else {
        sqlx::query_as("SELECT id, file_path FROM media")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
    };

    for (id, path_str) in rows {
        let path = Path::new(&path_str);
        if !path.exists() {
            tracing::info!(file_path = %path_str, "Removing missing file from DB");
            let _ = sqlx::query("DELETE FROM media WHERE id = ?").bind(id).execute(pool).await;
            let _ = sqlx::query("DELETE FROM playback_progress WHERE media_id = ?").bind(id).execute(pool).await;
        }
    }
}

fn parse_tv_show_info(path: &Path, library_path: &str, library_name: &str) -> Option<(String, i32, i32)> {
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

fn parse_episode_number(filename: &str) -> Option<i32> {
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

/// Ingest a single book file (pdf/cbz/epub). Books carry no external metadata for
/// now: the title comes from the filename, and CBZ archives get a page count so the
/// reader can paginate. PDF/EPUB page counts are determined client-side.
async fn process_book(pool: &SqlitePool, path: &Path, library: &Library) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());

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

    let service = crate::services::book_service::BookService::new(pool.clone());
    if let Err(e) = service.upsert_scanned(&path_str, &file_stem, library.id, page_count).await {
        tracing::warn!(file = %file_stem, error = %e, "Failed to upsert book");
    }
}

async fn process_video(pool: &SqlitePool, path: &Path, root_path: &str, library: &Library, force_refresh: bool, cache: Arc<Mutex<ScanCache>>) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());
    
    let (series_name, season_number, episode_number) = if library.library_type == LibraryType::TvShows {
        parse_tv_show_info(path, root_path, &library.name)
            .map(|(s, sn, en)| (Some(s), Some(sn), Some(en)))
            .unwrap_or((None, None, None))
    } else {
        (None, None, None)
    };

    let existing: Option<(i64, Option<String>)> = sqlx::query_as("SELECT id, media_info FROM media WHERE file_path = ?")
        .bind(&path_str).fetch_optional(pool).await.unwrap_or(None);
    
    tracing::debug!(file = %file_stem, force_refresh, exists = existing.is_some(), "Processing media file");

    let mut media_info_json = None;
    let should_probe = force_refresh || existing.as_ref().map(|(_, info)| info.is_none()).unwrap_or(true);

    if should_probe {
        // Probe media info
        match probe_media(&path_str).await {
            Ok(probe) => {
                if let Ok(json) = serde_json::to_string(&probe.media_info) {
                    media_info_json = Some(json);
                }
            },
            Err(e) => tracing::warn!(file = %file_stem, error = %e, "Failed to probe media"),
        }
    }

    if let Some((id, _)) = existing {
        if let Some(info) = media_info_json.clone() {
             let _ = sqlx::query("UPDATE media SET media_info = ? WHERE id = ?").bind(info).bind(id).execute(pool).await;
        }

        if !force_refresh {
            let _ = sqlx::query("UPDATE media SET library_id = ? WHERE file_path = ?").bind(library.id).bind(&path_str).execute(pool).await;
            if library.library_type == LibraryType::TvShows && series_name.is_some() {
                let _ = sqlx::query("UPDATE media SET series_name = ?, season_number = ?, episode_number = ? WHERE file_path = ? AND series_name IS NULL")
                    .bind(&series_name).bind(season_number).bind(episode_number).bind(&path_str).execute(pool).await;
            }
            return;
        }
        // Fallthrough if force_refresh is true: proceed to metadata update despite existing
    } else {
        tracing::info!(file_path = %path_str, "Inserting new media");
        // Insert new record if not existing
        let _ = sqlx::query("INSERT INTO media (file_path, title, library_id, series_name, season_number, episode_number, media_info) VALUES (?, ?, ?, ?, ?, ?, ?)")
            .bind(&path_str).bind(&file_stem).bind(library.id).bind(&series_name).bind(season_number).bind(episode_number).bind(&media_info_json).execute(pool).await;
    }

    if library.library_type == LibraryType::Other {
        let _ = sqlx::query("UPDATE media SET media_type = 'movie' WHERE file_path = ?").bind(&path_str).execute(pool).await;
        return;
    }

    let search_term = series_name.as_ref().unwrap_or(&file_stem);
    let media_type_hint = if library.library_type == LibraryType::TvShows { Some("series") } else { Some("movie") };

    // 1. Fetch Series/Movie Metadata (Cached)
    let meta_opt = {
        let c = cache.lock().await;
        if let Some(cached) = c.series_metadata.get(search_term) {
            cached.clone()
        } else {
            drop(c); // Unlock to await
            let fetched = fetch_metadata(search_term, media_type_hint, pool).await.ok();
            let mut c = cache.lock().await;
            c.series_metadata.insert(search_term.to_string(), fetched.clone());
            fetched
        }
    };

    if let Some(meta) = meta_opt {
        if !cache.lock().await.series_metadata.contains_key(search_term) {
             tracing::info!(search_term, "Metadata fetch success");
        }
        
        // ... (rest of processing using `meta`)
        let mut final_plot = meta.plot.clone();
        let mut final_title = None;
        let mut final_still = None;

        // Get the provider name dynamically to look up ID
        let provider_name = get_default_provider(pool).await;
        if let Some(provider_id) = meta.provider_ids.as_ref().and_then(|ids| ids.get(&provider_name)).and_then(|v| v.as_i64()) {
            if let (Some(sn), Some(en)) = (season_number, episode_number) {
                let id_str = provider_id.to_string();
                
                // 2. Fetch Season Episodes (Cached)
                let episodes_opt = {
                    let c = cache.lock().await;
                    if let Some(cached) = c.season_episodes.get(&(id_str.clone(), sn)) {
                        cached.clone()
                    } else {
                        drop(c); // Unlock
                        let fetched = fetch_episodes(&id_str, sn, pool, None).await.ok();
                        let mut c = cache.lock().await;
                        c.season_episodes.insert((id_str.clone(), sn), fetched.clone());
                        fetched
                    }
                };

                if let Some(episodes) = episodes_opt {
                    if let Some(ep) = episodes.iter().find(|e| e.episode_number == en) {
                        final_title = Some(ep.name.clone());
                        if !ep.overview.is_empty() { final_plot = Some(ep.overview.clone()); }
                        final_still = ep.still_path.clone();
                    }
                }
            }
        }

        let genres_str = meta.genres.as_ref().map(|g| g.join(", "));
        let year_int = meta.year.as_ref().and_then(|y| y.parse::<i64>().ok()).unwrap_or(0);
        let cast_json = meta.cast.as_ref().and_then(|c| serde_json::to_string(c).ok());
        let director_str = meta.director.as_ref().map(|d| d.join(", "));

        if library.library_type == LibraryType::TvShows {
            let _ = sqlx::query("UPDATE media SET year = ?, poster_url = ?, plot = ?, media_type = ?, backdrop_url = ?, series_name = ?, provider_ids = ?, title = COALESCE(?, title), still_url = ?, runtime = ?, genres = ?, rating = ?, cast = ?, director = ?, age_rating = ?, studio = ?, trailer_url = ?, origin_country = ?, collection_name = ?, creator = ?, tags = ? WHERE file_path = ?")
                .bind(year_int).bind(&meta.poster_url).bind(final_plot).bind(&meta.media_type).bind(&meta.backdrop_url).bind(&meta.title)
                .bind(meta.provider_ids.as_ref().map(|v| v.to_string())).bind(final_title).bind(final_still).bind(meta.runtime).bind(&genres_str)
                .bind(meta.rating).bind(&cast_json).bind(&director_str)
                .bind(&meta.age_rating).bind(&meta.studio).bind(&meta.trailer_url).bind(&meta.origin_country).bind(&meta.collection_name)
                .bind(meta.creator.as_ref().map(|c| c.join(", "))).bind(meta.tags.as_ref().map(|t| t.join(", ")))
                .bind(&path_str)
                .execute(pool).await;
        } else {
            let _ = sqlx::query("UPDATE media SET title = ?, year = ?, poster_url = ?, plot = ?, media_type = ?, backdrop_url = ?, provider_ids = ?, runtime = ?, genres = ?, rating = ?, cast = ?, director = ?, age_rating = ?, studio = ?, trailer_url = ?, origin_country = ?, collection_name = ?, creator = ?, tags = ? WHERE file_path = ?")
                .bind(&meta.title).bind(year_int).bind(&meta.poster_url).bind(&meta.plot).bind(&meta.media_type).bind(&meta.backdrop_url)
                .bind(meta.provider_ids.as_ref().map(|v| v.to_string())).bind(meta.runtime).bind(&genres_str)
                .bind(meta.rating).bind(&cast_json).bind(&director_str)
                .bind(&meta.age_rating).bind(&meta.studio).bind(&meta.trailer_url).bind(&meta.origin_country).bind(&meta.collection_name)
                .bind(meta.creator.as_ref().map(|c| c.join(", "))).bind(meta.tags.as_ref().map(|t| t.join(", ")))
                .bind(&path_str)
                .execute(pool).await;
        }
        tracing::info!(file = %file_stem, "Updated metadata");
    } else {
        tracing::warn!(search_term, "Failed to fetch metadata");
        // Fallback checks...
        if let Some(s_name) = series_name {
            let canonical: Option<(String,)> = sqlx::query_as("SELECT series_name FROM media WHERE LOWER(series_name) = LOWER(?) AND series_name IS NOT NULL LIMIT 1")
                .bind(&s_name)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

            if let Some((existing_name,)) = canonical {
                tracing::debug!(from = %s_name, to = %existing_name, "Using existing series name from DB");
                 let _ = sqlx::query("UPDATE media SET series_name = ? WHERE file_path = ?")
                    .bind(&existing_name)
                    .bind(&path_str)
                    .execute(pool).await;
            }
        }
    }
}

use sqlx::SqlitePool;
use walkdir::WalkDir;
use std::path::Path;
use regex::Regex;
use crate::models::db::library::{Library, LibraryType};
use crate::core::metadata::{fetch_metadata, fetch_episodes, get_default_provider};

pub async fn scan_media(pool: &SqlitePool) {
    let libraries = sqlx::query_as::<_, Library>("SELECT * FROM libraries")
        .fetch_all(pool)
        .await
        .unwrap_or(vec![]);

    for library in libraries {
        println!("Scanning library: {} (type: {:?})", library.name, library.library_type);
        for entry in WalkDir::new(&library.path).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ["mp4", "mkv", "avi", "mov", "webm", "wmv", "m4v", "mpg", "mpeg", "flv", "ts"].contains(&ext_str.as_str()) {
                        process_video(pool, path, &library).await;
                    }
                }
            }
        }
    }
    
    cleanup_missing_files(pool).await;
}

async fn cleanup_missing_files(pool: &SqlitePool) {
    println!("Cleaning up missing files...");
    let rows: Vec<(i64, String)> = sqlx::query_as("SELECT id, file_path FROM media")
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    for (id, path_str) in rows {
        let path = Path::new(&path_str);
        if !path.exists() {
            println!("Removing missing file from DB: {}", path_str);
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
        let season_number = Regex::new(r"season\s*(\d+)").ok()?.captures(&season_folder)
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
    if let Some(caps) = Regex::new(r"s\d+e(\d+)").ok()?.captures(&lower) { return caps.get(1)?.as_str().parse().ok(); }
    if let Some(caps) = Regex::new(r"\d+x(\d+)").ok()?.captures(&lower) { return caps.get(1)?.as_str().parse().ok(); }
    if let Some(caps) = Regex::new(r"ep(?:isode)?\s*(\d+)").ok()?.captures(&lower) { return caps.get(1)?.as_str().parse().ok(); }
    if let Some(caps) = Regex::new(r"[-\s](\d{1,3})[-\s]").ok()?.captures(&lower) { return caps.get(1)?.as_str().parse().ok(); }
    if let Some(caps) = Regex::new(r"(?:^|\s)e(\d+)").ok()?.captures(&lower) { return caps.get(1)?.as_str().parse().ok(); }
    None
}

async fn process_video(pool: &SqlitePool, path: &Path, library: &Library) {
    let path_str = path.to_string_lossy().to_string();
    let file_stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Unknown".to_string());
    
    let (series_name, season_number, episode_number) = if library.library_type == LibraryType::TvShows {
        parse_tv_show_info(path, &library.path, &library.name)
            .map(|(s, sn, en)| (Some(s), Some(sn), Some(en)))
            .unwrap_or((None, None, None))
    } else {
        (None, None, None)
    };

    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM media WHERE file_path = ?")
        .bind(&path_str).fetch_optional(pool).await.unwrap_or(None);

    if let Some((_id,)) = existing {
        let _ = sqlx::query("UPDATE media SET library_id = ? WHERE file_path = ?").bind(library.id).bind(&path_str).execute(pool).await;
        if library.library_type == LibraryType::TvShows && series_name.is_some() {
            let _ = sqlx::query("UPDATE media SET series_name = ?, season_number = ?, episode_number = ? WHERE file_path = ? AND series_name IS NULL")
                .bind(&series_name).bind(season_number).bind(episode_number).bind(&path_str).execute(pool).await;
        }
        return;
    }

    let _ = sqlx::query("INSERT INTO media (file_path, title, library_id, series_name, season_number, episode_number) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&path_str).bind(&file_stem).bind(library.id).bind(&series_name).bind(season_number).bind(episode_number).execute(pool).await;

    if library.library_type == LibraryType::Other {
        let _ = sqlx::query("UPDATE media SET media_type = 'movie' WHERE file_path = ?").bind(&path_str).execute(pool).await;
        return;
    }

    let search_term = series_name.as_ref().unwrap_or(&file_stem);
    let media_type_hint = if library.library_type == LibraryType::TvShows { Some("series") } else { Some("movie") };

    if let Ok(meta) = fetch_metadata(search_term, media_type_hint, pool).await {
        let mut final_plot = meta.plot.clone();
        let mut final_title = None;
        let mut final_still = None;

        // Get the provider name dynamically to look up ID
        let provider_name = get_default_provider(pool).await;
        if let Some(provider_id) = meta.provider_ids.as_ref().and_then(|ids| ids.get(&provider_name)).and_then(|v| v.as_i64()) {
            if let (Some(sn), Some(en)) = (season_number, episode_number) {
                let id_str = provider_id.to_string();
                if let Ok(episodes) = fetch_episodes(&id_str, sn, pool).await {
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

        if library.library_type == LibraryType::TvShows {
            let _ = sqlx::query("UPDATE media SET year = ?, poster_url = ?, plot = ?, media_type = ?, backdrop_url = ?, series_name = ?, provider_ids = ?, title = COALESCE(?, title), still_url = ?, runtime = ?, genres = ? WHERE file_path = ?")
                .bind(year_int).bind(&meta.poster_url).bind(final_plot).bind(&meta.media_type).bind(&meta.backdrop_url).bind(&meta.title)
                .bind(meta.provider_ids.as_ref().map(|v| v.to_string())).bind(final_title).bind(final_still).bind(meta.runtime).bind(genres_str).bind(&path_str)
                .execute(pool).await;
        } else {
            let _ = sqlx::query("UPDATE media SET title = ?, year = ?, poster_url = ?, plot = ?, media_type = ?, backdrop_url = ?, provider_ids = ?, runtime = ?, genres = ? WHERE file_path = ?")
                .bind(&meta.title).bind(year_int).bind(&meta.poster_url).bind(&meta.plot).bind(&meta.media_type).bind(&meta.backdrop_url)
                .bind(meta.provider_ids.as_ref().map(|v| v.to_string())).bind(meta.runtime).bind(genres_str).bind(&path_str)
                .execute(pool).await;
        }
        println!("Updated metadata for: {}", file_stem);
    }
}

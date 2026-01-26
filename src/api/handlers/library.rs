use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::db::models::{Library, LibraryType};
use crate::core::scanner::scan_media;
use super::common::{ListDirectoriesRequest, DirectoryEntry};
use std::path::Path as StdPath;

#[derive(serde::Deserialize)]
pub struct CreateLibraryRequest {
    name: String,
    path: String,
    library_type: LibraryType,
}

pub async fn get_libraries(State(pool): State<SqlitePool>) -> Result<Json<Vec<Library>>, AppError> {
    let libraries = sqlx::query_as::<_, Library>("SELECT * FROM libraries")
        .fetch_all(&pool)
        .await?;
    Ok(Json(libraries))
}

pub async fn create_library(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateLibraryRequest>,
) -> Result<StatusCode, AppError> {
    sqlx::query("INSERT INTO libraries (name, path, library_type) VALUES (?, ?, ?)")
        .bind(&payload.name)
        .bind(&payload.path)
        .bind(&payload.library_type)
        .execute(&pool)
        .await?;

    // Trigger background scan so content appears immediately
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scan_media(&pool_clone).await;
    });
    
    Ok(StatusCode::CREATED)
}

pub async fn delete_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    // 1. Delete progress for all media in this library
    sqlx::query("DELETE FROM playback_progress WHERE media_id IN (SELECT id FROM media WHERE library_id = ?)")
        .bind(id)
        .execute(&pool)
        .await?;

    // 2. Delete all media entries for this library
    sqlx::query("DELETE FROM media WHERE library_id = ?")
        .bind(id)
        .execute(&pool)
        .await?;

    // 3. Delete the library itself
    sqlx::query("DELETE FROM libraries WHERE id = ?")
        .bind(id)
        .execute(&pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn scan_all_libraries(State(pool): State<SqlitePool>) -> StatusCode {
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scan_media(&pool_clone).await;
    });
    StatusCode::ACCEPTED
}

pub async fn list_directories(
    Json(payload): Json<ListDirectoriesRequest>,
) -> Json<Vec<DirectoryEntry>> {
    let default_path = if cfg!(target_os = "windows") { "." } else { "/" };
    let path_str = payload.path.unwrap_or_else(|| default_path.to_string());
    let path = StdPath::new(&path_str);
    
    if path_str == "." && cfg!(target_os = "windows") {
        let mut drives = Vec::new();
        for b in b'A'..=b'Z' {
            let drive = format!("{}:\\", b as char);
            if StdPath::new(&drive).exists() {
                drives.push(DirectoryEntry {
                    name: drive.clone(),
                    path: drive,
                });
            }
        }
        return Json(drives);
    }

    let mut entries = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(path) {
        for entry in read_dir.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let full_path = entry.path().to_string_lossy().to_string();
                    entries.push(DirectoryEntry {
                        name,
                        path: full_path,
                    });
                }
            }
        }
    }
    
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Json(entries)
}

#[derive(serde::Deserialize)]
pub struct BrowseQuery {
    path: Option<String>,
}

#[derive(serde::Serialize)]
pub struct FileSystemEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub media_id: Option<i64>,
    pub poster_url: Option<String>,
}

pub async fn browse_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    axum::extract::Query(query): axum::extract::Query<BrowseQuery>,
) -> Result<Json<Vec<FileSystemEntry>>, AppError> {
    // 1. Get Library Root
    let library: Library = sqlx::query_as("SELECT * FROM libraries WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?
        .ok_or(AppError::NotFound("Library not found".to_string()))?;
    
    // 2. Resolve Path
    let root_path_str = library.path;
    let root = std::path::Path::new(&root_path_str);
    
    let relative_path_str = query.path.unwrap_or_default();
    // Prevent directory traversal
    if relative_path_str.contains("..") {
         return Err(AppError::BadRequest("Invalid path".to_string()));
    }

    let current_path = if relative_path_str.is_empty() {
        root.to_path_buf()
    } else {
        root.join(&relative_path_str)
    };
    
    if !current_path.starts_with(root) {
        return Err(AppError::BadRequest("Path outside library root".to_string()));
    }

    let mut entries = Vec::new();
    // Use tokio::fs for async directory reading
    if let Ok(mut read_dir) = tokio::fs::read_dir(&current_path).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files
            if name.starts_with('.') { continue; }
            
            let full_path = entry.path();
            let is_dir = full_path.is_dir();
            let mut media_id = None;
            let mut poster_url = None;

            // For files, filter by supported video extensions and get DB ID
            if !is_dir {
                if let Some(ext) = full_path.extension() {
                     let ext_str = ext.to_string_lossy().to_lowercase();
                     if !["mp4", "mkv", "avi", "mov", "webm", "wmv", "m4v", "mpg", "mpeg", "flv", "ts"].contains(&ext_str.as_str()) {
                         continue;
                     }
                     // Look up media ID and poster_url
                     let path_str = full_path.to_string_lossy().to_string();
                     
                     // 1. Try SELECT (Case Insensitive for Windows robustness)
                     let result: Option<(i64, Option<String>)> = sqlx::query_as("SELECT id, poster_url FROM media WHERE file_path = ? COLLATE NOCASE")
                        .bind(&path_str)
                        .fetch_optional(&pool)
                        .await
                        .unwrap_or(None);
                    
                     if let Some((id, poster)) = result {
                         media_id = Some(id);
                         poster_url = poster;
                     } else {
                         let title = full_path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default();
                         // 2. Try INSERT
                         if let Ok(r) = sqlx::query("INSERT INTO media (file_path, library_id, title, media_type) VALUES (?, ?, ?, 'movie')")
                             .bind(&path_str)
                             .bind(id)
                             .bind(title)
                             .execute(&pool)
                             .await 
                         {
                             media_id = Some(r.last_insert_rowid());
                         } else {
                             // 3. INSERT failed (likely exists but missed by SELECT due to race/weird case?), Try SELECT again
                             let retry: Option<(i64, Option<String>)> = sqlx::query_as("SELECT id, poster_url FROM media WHERE file_path = ? COLLATE NOCASE")
                                .bind(&path_str)
                                .fetch_optional(&pool)
                                .await
                                .unwrap_or(None);
                                
                             if let Some((id, poster)) = retry {
                                 media_id = Some(id);
                                 poster_url = poster;
                             }
                         }
                     }
                } else {
                    continue;
                }
            }

            let rel_entry_path = full_path.strip_prefix(root).unwrap_or(&full_path).to_string_lossy().to_string();

            entries.push(FileSystemEntry {
                name,
                path: rel_entry_path.replace("\\", "/"),
                is_directory: is_dir,
                media_id,
                poster_url,
            });
        }
    }
    
    // Sort: Directories first, then files
    entries.sort_by(|a, b| {
        if a.is_directory == b.is_directory {
            a.name.cmp(&b.name)
        } else {
            if a.is_directory { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
        }
    });
    
    Ok(Json(entries))
}


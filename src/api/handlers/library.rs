use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use crate::api::middleware::AuthUser;
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::libraries::{Library, LibraryType};
use crate::services::scanner::scan_media;
use super::common::{ListDirectoriesRequest, DirectoryEntry};
use std::path::Path as StdPath;

use crate::services::library_service::LibraryService;
use crate::models::db::library_providers::LibraryProvider;

#[derive(serde::Deserialize)]
pub struct CreateLibraryRequest {
    name: String,
    paths: Vec<String>,
    library_type: LibraryType,
    #[serde(default)]
    default_reading_mode: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct UpdateLibraryRequest {
    name: String,
    paths: Vec<String>,
    #[serde(default)]
    default_reading_mode: Option<String>,
}

pub async fn get_libraries(State(pool): State<SqlitePool>) -> Result<Json<Vec<Library>>, AppError> {
    let service = LibraryService::new(pool);
    let libraries = service.get_all().await?;
    Ok(Json(libraries))
}

pub async fn create_library(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateLibraryRequest>,
) -> Result<StatusCode, AppError> {
    let service = LibraryService::new(pool);
    service.create(payload.name, payload.paths, payload.library_type, payload.default_reading_mode).await?;
    Ok(StatusCode::CREATED)
}

pub async fn delete_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<StatusCode, AppError> {
    let service = LibraryService::new(pool);
    service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateLibraryRequest>,
) -> Result<StatusCode, AppError> {
    let service = LibraryService::new(pool);
    service.update(id, payload.name, payload.paths, payload.default_reading_mode).await?;
    Ok(StatusCode::OK)
}

pub async fn scan_all_libraries(State(pool): State<SqlitePool>) -> StatusCode {
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scan_media(&pool_clone, None, false).await;
    });
    StatusCode::ACCEPTED
}

pub async fn scan_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> StatusCode {
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scan_media(&pool_clone, Some(id), false).await;
    });
    StatusCode::ACCEPTED
}

pub async fn refresh_library(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> StatusCode {
    let pool_clone = pool.clone();
    tokio::spawn(async move {
        scan_media(&pool_clone, Some(id), true).await;
    });
    StatusCode::ACCEPTED
}

/// Browse server-side directories for library-path configuration. Admin-only:
/// it can enumerate any directory the server process can read, so it must not be
/// exposed to ordinary users.
pub async fn list_directories(
    Extension(auth_user): Extension<AuthUser>,
    Json(payload): Json<ListDirectoriesRequest>,
) -> Result<Json<Vec<DirectoryEntry>>, AppError> {
    auth_user.require_admin()?;
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
        return Ok(Json(drives));
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
    Ok(Json(entries))
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
    let service = LibraryService::new(pool);
    let entries = service.browse(id, query.path).await?;
    Ok(Json(entries))
}

#[derive(serde::Deserialize)]
pub struct UpdateLibraryProvidersRequest {
    pub providers: Vec<LibraryProviderInput>,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct LibraryProviderInput {
    pub provider_id: String,
    pub priority: i32,
    pub enabled: bool,
}

pub async fn get_library_providers(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<LibraryProvider>>, AppError> {
    let providers = LibraryService::new(pool).list_providers(id).await?;
    Ok(Json(providers))
}

pub async fn update_library_providers(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateLibraryProvidersRequest>,
) -> Result<StatusCode, AppError> {
    let providers: Vec<(String, i32, bool)> = payload.providers
        .into_iter()
        .map(|p| (p.provider_id, p.priority, p.enabled))
        .collect();
    LibraryService::new(pool).replace_providers(id, &providers).await?;
    Ok(StatusCode::OK)
}



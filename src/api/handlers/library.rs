use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use sqlx::SqlitePool;
use crate::error::AppError;
use crate::db::models::{Library, LibraryType};
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
    let providers: Vec<LibraryProvider> = sqlx::query_as(
        "SELECT library_id, provider_id, priority, enabled FROM library_providers WHERE library_id = ? ORDER BY priority ASC"
    )
    .bind(id)
    .fetch_all(&pool)
    .await?;

    Ok(Json(providers))
}

pub async fn update_library_providers(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateLibraryProvidersRequest>,
) -> Result<StatusCode, AppError> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM library_providers WHERE library_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;

    for p in payload.providers {
        sqlx::query(
            "INSERT INTO library_providers (library_id, provider_id, priority, enabled) VALUES (?, ?, ?, ?)"
        )
        .bind(id)
        .bind(p.provider_id)
        .bind(p.priority)
        .bind(p.enabled)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(StatusCode::OK)
}



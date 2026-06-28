//! Library Service
//! 
//! Handles business logic for library management.

use sqlx::SqlitePool;
use crate::error::AppError;
use crate::models::db::libraries::{Library, LibraryRow, LibraryType};
use crate::models::db::library_providers::LibraryProvider;
use crate::services::scanner::scan_media;

pub struct LibraryService {
    pool: SqlitePool,
}

impl LibraryService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Fetch the folder paths associated with a library, in insertion order.
    async fn get_paths(&self, library_id: i64) -> Result<Vec<String>, AppError> {
        let paths = sqlx::query_scalar::<_, String>(
            "SELECT path FROM library_paths WHERE library_id = ? ORDER BY id",
        )
        .bind(library_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(paths)
    }

    fn hydrate(row: LibraryRow, paths: Vec<String>) -> Library {
        Library {
            id: row.id,
            name: row.name,
            paths,
            library_type: row.library_type,
            default_reading_mode: row.default_reading_mode,
        }
    }

    pub async fn get_all(&self) -> Result<Vec<Library>, AppError> {
        let rows = sqlx::query_as::<_, LibraryRow>("SELECT id, name, library_type, default_reading_mode FROM libraries")
            .fetch_all(&self.pool)
            .await?;

        let mut libraries = Vec::with_capacity(rows.len());
        for row in rows {
            let paths = self.get_paths(row.id).await?;
            libraries.push(Self::hydrate(row, paths));
        }
        Ok(libraries)
    }

    pub async fn create(&self, name: String, paths: Vec<String>, library_type: LibraryType, default_reading_mode: Option<String>) -> Result<i64, AppError> {
        let mut tx = self.pool.begin().await.map_err(|e| AppError::Internal(e.to_string()))?;

        // The legacy `path` column is NOT NULL; keep it populated with the first path
        // for backward compatibility while `library_paths` is the source of truth.
        let primary_path = paths.first().cloned().unwrap_or_default();
        let result = sqlx::query("INSERT INTO libraries (name, path, library_type, default_reading_mode) VALUES (?, ?, ?, ?)")
            .bind(&name)
            .bind(&primary_path)
            .bind(&library_type)
            .bind(&default_reading_mode)
            .execute(&mut *tx)
            .await?;
        let id = result.last_insert_rowid();

        for path in &paths {
            sqlx::query("INSERT INTO library_paths (library_id, path) VALUES (?, ?)")
                .bind(id)
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await.map_err(|e| AppError::Internal(e.to_string()))?;

        // Trigger background scan
        let pool = self.pool.clone();
        tokio::spawn(async move {
            scan_media(&pool, None, false).await;
        });

        Ok(id)
    }

    pub async fn get_by_id(&self, id: i64) -> Result<Library, AppError> {
        let row = sqlx::query_as::<_, LibraryRow>("SELECT id, name, library_type, default_reading_mode FROM libraries WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound("Library not found".to_string()))?;
        let paths = self.get_paths(id).await?;
        Ok(Self::hydrate(row, paths))
    }

    pub async fn update(&self, id: i64, name: String, paths: Vec<String>, default_reading_mode: Option<String>) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await.map_err(|e| AppError::Internal(e.to_string()))?;

        let primary_path = paths.first().cloned().unwrap_or_default();
        sqlx::query("UPDATE libraries SET name = ?, path = ?, default_reading_mode = ? WHERE id = ?")
            .bind(name)
            .bind(&primary_path)
            .bind(&default_reading_mode)
            .bind(id)
            .execute(&mut *tx)
            .await?;

        // Replace the set of paths.
        sqlx::query("DELETE FROM library_paths WHERE library_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for path in &paths {
            sqlx::query("INSERT INTO library_paths (library_id, path) VALUES (?, ?)")
                .bind(id)
                .bind(path)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await.map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }


    /// The metadata providers configured for a library, ordered by priority.
    pub async fn list_providers(&self, library_id: i64) -> Result<Vec<LibraryProvider>, AppError> {
        Ok(sqlx::query_as::<_, LibraryProvider>(
            "SELECT library_id, provider_id, priority, enabled FROM library_providers WHERE library_id = ? ORDER BY priority ASC"
        )
        .bind(library_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Replace the full set of providers for a library. Each entry is
    /// `(provider_id, priority, enabled)`.
    pub async fn replace_providers(&self, library_id: i64, providers: &[(String, i32, bool)]) -> Result<(), AppError> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM library_providers WHERE library_id = ?")
            .bind(library_id)
            .execute(&mut *tx)
            .await?;

        for (provider_id, priority, enabled) in providers {
            sqlx::query(
                "INSERT INTO library_providers (library_id, provider_id, priority, enabled) VALUES (?, ?, ?, ?)"
            )
            .bind(library_id)
            .bind(provider_id)
            .bind(priority)
            .bind(enabled)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<(), AppError> {
        // `media_items`, `series`, `artists`, `albums` and `library_paths` all declare
        // `ON DELETE CASCADE` on `library_id`, which in turn cascades to the per-type
        // detail tables, credits/genre links and per-user state. So deleting the
        // library row tears down everything beneath it (foreign keys are enabled on
        // the pool connection).
        sqlx::query("DELETE FROM libraries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn browse(&self, id: i64, relative_path: Option<String>) -> Result<Vec<crate::api::handlers::library::FileSystemEntry>, AppError> {
        // 1. Get Library and its root paths
        let library = self.get_by_id(id).await?;
        if library.paths.is_empty() {
            return Ok(Vec::new());
        }

        let relative_path_str = relative_path.unwrap_or_default();
        // Prevent directory traversal
        if relative_path_str.contains("..") {
             return Err(AppError::BadRequest("Invalid path".to_string()));
        }

        // 2. Resolve which directories to read.
        // When no relative path is given we list the contents of *every* root, merged.
        // Otherwise we resolve the relative path against the root that contains it.
        // Each target carries its owning root so entry paths stay relative to that root.
        let mut targets: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
        if relative_path_str.is_empty() {
            for root in &library.paths {
                let root_buf = std::path::PathBuf::from(root);
                targets.push((root_buf.clone(), root_buf));
            }
        } else {
            let mut resolved = None;
            for root in &library.paths {
                let root_buf = std::path::PathBuf::from(root);
                let candidate = root_buf.join(&relative_path_str);
                if candidate.starts_with(&root_buf) && candidate.exists() {
                    resolved = Some((candidate, root_buf));
                    break;
                }
            }
            match resolved {
                Some(target) => targets.push(target),
                None => return Err(AppError::BadRequest("Path outside library root".to_string())),
            }
        }

        let mut entries = Vec::new();
        // Absolute path per file entry (aligned by index with `entries`), used to match
        // against DB records. Directories get `None`.
        let mut entry_abs_paths: Vec<Option<String>> = Vec::new();
        let mut file_paths_to_check = Vec::new();

        // Use tokio::fs for async directory reading
        for (current_path, root) in &targets {
        if let Ok(mut read_dir) = tokio::fs::read_dir(&current_path).await {
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') { continue; }

                let full_path = entry.path();
                let is_dir = full_path.is_dir();
                let abs_path_str = full_path.to_string_lossy().to_string();

                let mut abs_for_entry = None;
                if !is_dir {
                     if let Some(ext) = full_path.extension() {
                         let ext_str = ext.to_string_lossy().to_lowercase();
                         if ["mp4", "mkv", "avi", "mov", "webm", "wmv", "m4v", "mpg", "mpeg", "flv", "ts"].contains(&ext_str.as_str()) {
                             file_paths_to_check.push(abs_path_str.clone());
                             abs_for_entry = Some(abs_path_str);
                         }
                     }
                }

                let rel_entry_path = full_path.strip_prefix(root).unwrap_or(&full_path).to_string_lossy().to_string();

                entries.push(crate::api::handlers::library::FileSystemEntry {
                    name,
                    path: rel_entry_path.replace("\\", "/"),
                    is_directory: is_dir,
                    media_id: None, // Will populate later
                    poster_url: None, // Will populate later
                });
                entry_abs_paths.push(abs_for_entry);
            }
        }
        }

        // Batch Query for Files
        if !file_paths_to_check.is_empty() {
            // SQLite has a limit on variables, but 50-100 files in a folder is typical.
            // For safety, we can chunk if needed, but for now assuming folder size < 900 files.
            let placeholders: Vec<String> = file_paths_to_check.iter().map(|_| "?".to_string()).collect();
            let query = format!(
                "SELECT mi.id, mi.file_path,
                        COALESCE(mv.poster_url, e.still_url, b.poster_url, mvd.poster_url) AS poster_url
                 FROM media_items mi
                 LEFT JOIN movies mv ON mv.item_id = mi.id
                 LEFT JOIN episodes e ON e.item_id = mi.id
                 LEFT JOIN books b ON b.item_id = mi.id
                 LEFT JOIN music_videos mvd ON mvd.item_id = mi.id
                 WHERE mi.file_path IN ({}) COLLATE NOCASE",
                placeholders.join(",")
            );

            let mut q = sqlx::query_as::<_, (i64, String, Option<String>)>(&query);
            for p in &file_paths_to_check {
                q = q.bind(p);
            }

            let results = q.fetch_all(&self.pool).await?;

            // Map results for quick lookup
            use std::collections::HashMap;
            let mut lookup: HashMap<String, (i64, Option<String>)> = HashMap::new();
            for (id, path, poster) in results {
                lookup.insert(path, (id, poster));
            }

            // Update entries by matching the absolute path captured during the walk.
            for (entry, abs_path) in entries.iter_mut().zip(entry_abs_paths.iter()) {
                if let Some(abs) = abs_path {
                    if let Some((id, poster)) = lookup.get(abs) {
                        entry.media_id = Some(*id);
                        entry.poster_url = poster.clone();
                    }
                }
            }
        }
        
        // Sort
        entries.sort_by(|a, b| {
            if a.is_directory == b.is_directory {
                a.name.cmp(&b.name)
            } else {
                if a.is_directory { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
            }
        });
        
        Ok(entries)
    }
}


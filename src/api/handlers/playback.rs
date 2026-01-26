use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use sqlx::{SqlitePool, FromRow};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use serde::{Serialize, Deserialize};
use crate::error::AppError;

#[derive(serde::Deserialize)]
pub struct UpdateProgressRequest {
    position: i64,
    total_duration: i64,
}

#[derive(Debug, FromRow, Serialize, Deserialize, Clone)]
pub struct MediaWithProgress {
    pub id: i64,
    pub library_id: i64,
    pub file_path: String,
    pub title: Option<String>,
    pub year: Option<i64>,
    pub poster_url: Option<String>,
    pub plot: Option<String>,
    pub media_type: Option<String>,
    pub added_at: Option<chrono::NaiveDateTime>,
    pub series_name: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub provider_ids: Option<String>,
    pub backdrop_url: Option<String>,
    pub still_url: Option<String>,
    pub runtime: Option<i32>,
    pub genres: Option<String>,
    pub progress: Option<i64>,
    pub library_type: Option<crate::db::models::LibraryType>,
}

pub async fn get_continue_watching(State(pool): State<SqlitePool>) -> Result<Json<Vec<MediaWithProgress>>, AppError> {
    let media = sqlx::query_as::<_, MediaWithProgress>(
        "SELECT m.*, p.position as progress, l.library_type 
         FROM media m
         JOIN playback_progress p ON m.id = p.media_id
         JOIN libraries l ON m.library_id = l.id
         WHERE p.position > 10 AND p.position < (p.total_duration * 0.95)
         AND l.library_type != 'other'
         ORDER BY p.last_watched DESC
         LIMIT 10"
    )
    .fetch_all(&pool)
    .await?;

    Ok(Json(media))
}

pub async fn update_progress(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateProgressRequest>,
) -> Result<StatusCode, AppError> {
    sqlx::query(
        "INSERT INTO playback_progress (media_id, position, total_duration, last_watched) 
         VALUES (?, ?, ?, CURRENT_TIMESTAMP) 
         ON CONFLICT(media_id) DO UPDATE SET position = ?, total_duration = ?, last_watched = CURRENT_TIMESTAMP"
    )
    .bind(id)
    .bind(payload.position)
    .bind(payload.total_duration)
    .bind(payload.position)
    .bind(payload.total_duration)
    .execute(&pool)
    .await?;

    Ok(StatusCode::OK)
}

pub async fn get_media_progress(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<serde_json::Value>, AppError> {
    let progress: Option<i64> = sqlx::query_scalar("SELECT position FROM playback_progress WHERE media_id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;
    
    Ok(Json(serde_json::json!({ "position": progress.unwrap_or(0) })))
}

pub async fn stream_video(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    method: axum::http::Method,
    headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let file_path = match result {
        Some((path,)) => path,
        None => return Err(StatusCode::NOT_FOUND),
    };

    let mut file = File::open(&file_path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let metadata = file.metadata().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let file_size = metadata.len();

    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|s| {
            let s = s.strip_prefix("bytes=")?;
            let mut parts = s.split('-');
            let start = parts.next()?.parse::<u64>().ok()?;
            let end = parts.next().and_then(|s| s.parse::<u64>().ok());
            Some((start, end))
        });

    let mime = match std::path::Path::new(&file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .as_deref()
    {
        Some("mkv") => "video/x-matroska",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("avi") => "video/x-msvideo",
        Some("wmv") => "video/x-ms-wmv",
        _ => "video/mp4",
    };

    if method == axum::http::Method::HEAD {
        return Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CONTENT_LENGTH, file_size.to_string())
            .header(header::ACCEPT_RANGES, "bytes")
            .body(Body::empty())
            .unwrap());
    }

    match range {
        Some((start, end)) => {
            let end = end.unwrap_or(file_size - 1);
            let length = end - start + 1;

            file.seek(SeekFrom::Start(start)).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let stream = tokio_util::io::ReaderStream::new(file.take(length));
            let body = Body::from_stream(stream);

            let mut response = Response::new(body);
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(header::CONTENT_RANGE, format!("bytes {}-{}/{}", start, end, file_size).parse().unwrap());
            response.headers_mut().insert(header::CONTENT_LENGTH, length.to_string().parse().unwrap());
            response.headers_mut().insert(header::CONTENT_TYPE, mime.parse().unwrap());
            response.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());

            Ok(response)
        }
        None => {
            let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));
            let mut response = Response::new(body);
            response.headers_mut().insert(header::CONTENT_LENGTH, file_size.to_string().parse().unwrap());
            response.headers_mut().insert(header::CONTENT_TYPE, mime.parse().unwrap());
            response.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            Ok(response)
        }
    }
}

#[derive(Serialize)]
pub struct SubtitleTrack {
    pub id: String,
    pub label: String,
    pub language: String,
    pub source: String, // "url"
    pub url: String,
}

pub async fn get_subtitles(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<SubtitleTrack>>, AppError> {
    let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;

    let file_path = match result {
        Some((path,)) => std::path::PathBuf::from(path),
        None => return Err(AppError::NotFound("Media not found".to_string())),
    };

    let parent_dir = file_path.parent().ok_or(AppError::Internal("Invalid file path".to_string()))?;
    let file_stem = file_path.file_stem().ok_or(AppError::Internal("Invalid filename".to_string()))?.to_string_lossy().to_string();

    let mut subtitles = Vec::new();

    if let Ok(mut read_dir) = tokio::fs::read_dir(parent_dir).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "srt" || ext_str == "vtt" {
                        let filename = path.file_name().unwrap().to_string_lossy().to_string();
                        // Check if it belongs to this video
                        // Logic: 
                        // 1. Exact match: video.srt
                        // 2. Language match: video.en.srt, video.eng.srt
                        
                        if filename.starts_with(&file_stem) {
                            // It's a match!
                            let label = if filename == format!("{}.{}", file_stem, ext_str) {
                                "Default".to_string()
                            } else {
                                // Try to extract language code/label from suffix
                                // e.g., movie.en.srt -> en
                                let suffix = filename.strip_prefix(&file_stem).unwrap_or("").strip_suffix(&format!(".{}", ext_str)).unwrap_or("");
                                let clean_suffix = suffix.trim_start_matches('.').trim_end_matches('.');
                                if clean_suffix.is_empty() {
                                    "Unknown".to_string()
                                } else {
                                    clean_suffix.to_string()
                                }
                            };
                            
                            // Using filename as ID for simplicity
                            subtitles.push(SubtitleTrack {
                                id: filename.clone(),
                                label,
                                language: "en".to_string(), // Naive default, real impl would parse code
                                source: "url".to_string(),
                                url: format!("/api/v1/stream/{}/subtitle/{}", id, filename),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(Json(subtitles))
}

pub async fn stream_subtitle(
    Path((id, filename)): Path<(i64, String)>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Get Media Path to verify security/locality
    let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;

    let media_path = match result {
        Some((path,)) => std::path::PathBuf::from(path),
        None => return Err(AppError::NotFound("Media not found".to_string())),
    };

    let parent_dir = media_path.parent().ok_or(AppError::Internal("Invalid file path".to_string()))?;
    let subtitle_path = parent_dir.join(&filename);

    // Security check: Ensure subtitle is actually in the same directory (prevent traversal if filename has ..)
    if !subtitle_path.starts_with(parent_dir) {
         return Err(AppError::BadRequest("Invalid subtitle path".to_string()));
    }
    
    if !subtitle_path.exists() {
        return Err(AppError::NotFound("Subtitle not found".to_string()));
    }

    let content = tokio::fs::read_to_string(&subtitle_path).await.map_err(|_| AppError::Internal("Failed to read subtitle".to_string()))?;
    
    // Simple MIME type detection based on extension
    let mime = if filename.ends_with(".vtt") {
        "text/vtt"
    } else {
        "application/x-subrip" // for .srt
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    
    Ok((headers, content))
}

pub async fn get_thumbnail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    // 1. Check for cached thumbnail
    let thumb_dir = std::path::Path::new("thumbnails");
    if !thumb_dir.exists() {
        let _ = std::fs::create_dir(thumb_dir);
    }
    
    let thumb_filename = format!("{}.jpg", id);
    let thumb_path = thumb_dir.join(&thumb_filename);

    if !thumb_path.exists() {
        // 2. Get media file path and metadata
        let result: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT file_path, poster_url, backdrop_url FROM media WHERE id = ?"
        )
            .bind(id)
            .fetch_optional(&pool)
            .await?;

        let (file_path, poster_url, backdrop_url) = match result {
            Some(row) => row,
            None => return Err(AppError::NotFound("Media not found".to_string())),
        };

        // 3. Try validation/download from metadata
        let mut generated = false;

        // Try poster first, then backdrop
        for url_opt in [poster_url, backdrop_url] {
            if let Some(url) = url_opt {
                if !url.is_empty() {
                    // Start download
                    match reqwest::get(&url).await {
                        Ok(resp) => {
                            if resp.status().is_success() {
                                match resp.bytes().await {
                                    Ok(bytes) => {
                                        // Save to thumbnail path
                                        if let Ok(_) = tokio::fs::write(&thumb_path, &bytes).await {
                                            tracing::info!("Downloaded thumbnail for {} from {}", id, url);
                                            generated = true;
                                            break;
                                        }
                                    },
                                    Err(e) => tracing::warn!("Failed to get bytes for {} from {}: {}", id, url, e)
                                }
                            }
                        },
                        Err(e) => tracing::warn!("Failed to download thumbnail for {} from {}: {}", id, url, e)
                    }
                }
            }
        }

        // 4. Fallback to FFmpeg if needed
        if !generated {
             // Find FFmpeg - check common locations first
            let ffmpeg_paths = [
                "C:\\ffmpeg\\bin\\ffmpeg.exe",  // Common Windows install
                "./ffmpeg/ffmpeg.exe",           // Bundled with server (Windows)
                "./ffmpeg/ffmpeg",               // Bundled with server (Linux/Mac)
                "ffmpeg",                        // System PATH
            ];
            
            let ffmpeg_cmd = ffmpeg_paths.iter()
                .find(|p| std::path::Path::new(p).exists() || *p == &"ffmpeg")
                .unwrap_or(&"ffmpeg");
            
            tracing::info!("Generating thumbnail for {} using FFmpeg", id);
            
            // Run FFmpeg asynchronously: extract frame at 5 seconds
            // Optimized: -ss before -i for fast input seeking
            let output = tokio::process::Command::new(ffmpeg_cmd)
                .arg("-ss")
                .arg("00:00:05.000")
                .arg("-i")
                .arg(&file_path)
                .arg("-vframes")
                .arg("1")
                .arg("-vf")
                .arg("scale=320:-1") // Limit width to 320px for smaller files
                .arg(&thumb_path)
                .arg("-y")
                .output()
                .await; // .await here needed for tokio process

            match output {
                Ok(o) => {
                    if !o.status.success() {
                        let err = String::from_utf8_lossy(&o.stderr);
                        tracing::warn!("FFmpeg failed for {}: {}", id, err);
                        // Don't return error yet, let it fall through to "Failed to read" if file wasn't created
                    }
                }
                Err(e) => {
                     tracing::error!("Failed to execute FFmpeg: {}", e);
                }
            }
        }
    }

    // 5. Read and return the thumbnail
    let thumb_bytes = tokio::fs::read(&thumb_path).await
        .map_err(|_| AppError::Internal("Failed to read (or generate) thumbnail".to_string()))?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/jpeg".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "public, max-age=31536000".parse().unwrap()); // Cache for 1 year

    Ok((headers, thumb_bytes))
}

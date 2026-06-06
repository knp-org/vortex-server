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
            response.headers_mut().insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());

            Ok(response)
        }
        None => {
            // For initial request without range, use chunked streaming for faster start
            let stream = tokio_util::io::ReaderStream::with_capacity(file, 64 * 1024); // 64KB chunks
            let body = Body::from_stream(stream);
            
            let mut response = Response::new(body);
            response.headers_mut().insert(header::CONTENT_LENGTH, file_size.to_string().parse().unwrap());
            response.headers_mut().insert(header::CONTENT_TYPE, mime.parse().unwrap());
            response.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
            response.headers_mut().insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
            Ok(response)
        }
    }
}

#[derive(Serialize)]
pub struct AudioTrack {
    pub index: i32,
    pub label: String,
    pub language: Option<String>,
    pub codec: String,
    pub channels: Option<i32>,
    pub is_default: bool,
}

pub async fn get_audio_tracks(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<Json<Vec<AudioTrack>>, AppError> {
    let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;

    let file_path = match result {
        Some((path,)) => path,
        None => return Err(AppError::NotFound("Media not found".to_string())),
    };

    let probe = crate::services::transcode::codecs::probe_media(&file_path).await?;

    let tracks: Vec<AudioTrack> = probe.media_info.audio.iter().map(|a| {
        let channel_desc = match a.channels {
            Some(8) => "7.1",
            Some(6) => "5.1",
            Some(2) => "Stereo",
            Some(1) => "Mono",
            Some(n) => return AudioTrack {
                index: a.index,
                label: format!("{} - {} {}ch",
                    a.title.as_deref().or(a.language.as_deref()).unwrap_or("Unknown"),
                    a.codec.to_uppercase(),
                    n
                ),
                language: a.language.clone(),
                codec: a.codec.clone(),
                channels: a.channels,
                is_default: a.default,
            },
            None => "Unknown",
        };

        let label = if let Some(title) = &a.title {
            format!("{} - {} {}", title, a.codec.to_uppercase(), channel_desc)
        } else {
            let lang = a.language.as_deref().unwrap_or("Unknown");
            format!("{} - {} {}", lang, a.codec.to_uppercase(), channel_desc)
        };

        AudioTrack {
            index: a.index,
            label,
            language: a.language.clone(),
            codec: a.codec.clone(),
            channels: a.channels,
            is_default: a.default,
        }
    }).collect();

    Ok(Json(tracks))
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
        Some((path,)) => path,
        None => return Err(AppError::NotFound("Media not found".to_string())),
    };

    let file_path_buf = std::path::PathBuf::from(&file_path);
    let parent_dir = file_path_buf.parent().ok_or(AppError::Internal("Invalid file path".to_string()))?;
    let file_stem = file_path_buf.file_stem().ok_or(AppError::Internal("Invalid filename".to_string()))?.to_string_lossy().to_string();

    let mut subtitles = Vec::new();

    // 1. External subtitle files (.srt, .vtt)
    if let Ok(mut read_dir) = tokio::fs::read_dir(parent_dir).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "srt" || ext_str == "vtt" {
                        let filename = path.file_name().unwrap().to_string_lossy().to_string();

                        if filename.starts_with(&file_stem) {
                            let label = if filename == format!("{}.{}", file_stem, ext_str) {
                                "Default".to_string()
                            } else {
                                let suffix = filename.strip_prefix(&file_stem).unwrap_or("").strip_suffix(&format!(".{}", ext_str)).unwrap_or("");
                                let clean_suffix = suffix.trim_start_matches('.').trim_end_matches('.');
                                if clean_suffix.is_empty() {
                                    "Unknown".to_string()
                                } else {
                                    clean_suffix.to_string()
                                }
                            };

                            subtitles.push(SubtitleTrack {
                                id: format!("ext:{}", filename),
                                label,
                                language: "en".to_string(),
                                source: "external".to_string(),
                                url: format!("/api/v1/stream/{}/subtitle/{}", id, filename),
                            });
                        }
                    }
                }
            }
        }
    }

    // 2. Embedded subtitle streams (via ffprobe)
    if let Ok(probe) = crate::services::transcode::codecs::probe_media(&file_path).await {
        for sub in &probe.media_info.subtitles {
            let lang = sub.language.as_deref().unwrap_or("und");
            let label = if let Some(title) = &sub.title {
                format!("{} ({})", title, lang)
            } else {
                let mut l = lang.to_string();
                if sub.is_forced { l.push_str(" [Forced]"); }
                if sub.is_default { l.push_str(" [Default]"); }
                l
            };

            subtitles.push(SubtitleTrack {
                id: format!("emb:{}", sub.index),
                label,
                language: lang.to_string(),
                source: "embedded".to_string(),
                url: format!("/api/v1/stream/{}/subtitle/embedded/{}", id, sub.index),
            });
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

    if !subtitle_path.exists() {
        return Err(AppError::NotFound("Subtitle not found".to_string()));
    }

    let canonical_sub = subtitle_path.canonicalize().map_err(|_| AppError::NotFound("Subtitle not found".to_string()))?;
    let canonical_parent = parent_dir.canonicalize().map_err(|_| AppError::Internal("Invalid file path".to_string()))?;
    if !canonical_sub.starts_with(&canonical_parent) {
        return Err(AppError::BadRequest("Invalid subtitle path".to_string()));
    }

    let bytes = tokio::fs::read(&subtitle_path).await
        .map_err(|e| AppError::Internal(format!("Failed to read subtitle: {}", e)))?;

    // Handle non-UTF-8 encodings (common with SRT files)
    let content = String::from_utf8(bytes.clone())
        .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());

    let (final_content, mime) = if filename.ends_with(".srt") {
        let re = regex::Regex::new(r"(\d{2}:\d{2}:\d{2}),(\d{3})").unwrap();
        let vtt_content = format!("WEBVTT\n\n{}", re.replace_all(&content, "$1.$2"));
        (vtt_content, "text/vtt")
    } else if filename.ends_with(".vtt") {
        (content, "text/vtt")
    } else {
        (content, "application/x-subrip")
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    
    Ok((headers, final_content))
}

pub async fn stream_embedded_subtitle(
    Path((id, stream_index)): Path<(i64, i32)>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
        .bind(id)
        .fetch_optional(&pool)
        .await?;

    let file_path = match result {
        Some((path,)) => path,
        None => return Err(AppError::NotFound("Media not found".to_string())),
    };

    let output = tokio::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel", "error",
            "-i", &file_path,
            "-map", &format!("0:{}", stream_index),
            "-f", "webvtt",
            "-",
        ])
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("FFmpeg error: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::Internal(format!("FFmpeg subtitle extraction failed: {}", stderr)));
    }

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "text/vtt".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "public, max-age=3600".parse().unwrap());

    Ok((headers, output.stdout))
}

pub async fn get_thumbnail(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    // Books: derive the cover from the file itself. CBZ → first page image.
    // PDF/EPUB covers are rendered client-side, so fall through to a 404 here.
    if let Some(book) = crate::services::book_service::BookService::new(pool.clone()).get_optional(id).await? {
        use crate::services::books::{self, BookFormat};
        if books::detect(&book.file_path) == Some(BookFormat::Cbz) {
            let (bytes, mime) = books::cbz_page(&book.file_path, 0).await?;
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
            headers.insert(header::CACHE_CONTROL, "public, max-age=86400".parse().unwrap());
            return Ok((headers, bytes));
        }
        return Err(AppError::NotFound("No cover available".to_string()));
    }

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

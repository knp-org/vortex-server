use axum::{
    extract::{Path, State, Query},
    http::{header, HeaderMap},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;
use crate::error::AppError;

/// Information about media stream for frontend decision-making
#[derive(Serialize)]
pub struct StreamInfo {
    pub needs_transcode: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub direct_stream_url: String,
    pub hls_url: Option<String>,
    pub duration_seconds: Option<f64>,
}

/// Encoder configuration for settings API
#[derive(Clone, Debug)]
#[allow(dead_code)]
struct EncoderConfig {
    name: &'static str,
    codec: &'static str,
}

/// Message for settings API
#[derive(Serialize)]
pub struct TranscodeSettings {
    pub current_encoder: String,
    pub available_encoders: Vec<String>,
    pub thread_count: Option<u32>,
    pub preset: Option<String>,
    pub throttle_transcodes: bool,
    pub max_bitrate: Option<u32>,
    pub system_threads: usize,
}

#[derive(Deserialize)]
pub struct UpdateTranscodeSettings {
    pub encoder: String,
    pub thread_count: Option<u32>,
    pub preset: Option<String>,
    pub throttle_transcodes: bool,
    pub max_bitrate: Option<u32>,
}

/// Cached list of available encoders
static AVAILABLE_ENCODERS: OnceLock<Vec<EncoderConfig>> = OnceLock::new();

/// Detect all available encoders
async fn detect_available_encoders() -> Vec<EncoderConfig> {
    let mut available = Vec::new();
    
    // Hardware encoder candidates (name, codec)
    let hw_candidates = [
        ("NVIDIA NVENC", "h264_nvenc"),
        ("Intel QuickSync", "h264_qsv"),
        ("VAAPI", "h264_vaapi"),
    ];

    for (name, codec) in hw_candidates {
        let result = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-f", "lavfi",
                "-i", "nullsrc=s=64x64:d=0.1",
                "-c:v", codec,
                "-f", "null",
                "-",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        if let Ok(status) = result {
            if status.success() {
                tracing::info!("Detected hardware encoder: {} ({})", name, codec);
                available.push(EncoderConfig { name, codec });
            }
        }
    }

    // Always add Software fallback
    available.push(EncoderConfig {
        name: "Software (CPU)",
        codec: "libx264",
    });

    available
}

/// Get the list of available encoders (cached)
async fn get_available_encoders() -> &'static Vec<EncoderConfig> {
    AVAILABLE_ENCODERS.get_or_init(|| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(detect_available_encoders())
        })
    })
}

/// Get the active encoder based on DB settings
async fn get_active_encoder(pool: &SqlitePool) -> &'static EncoderConfig {
    let available = get_available_encoders().await;
    
    // Get setting from DB
    let setting: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'transcode_encoder'"
    )
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    if let Some(name) = setting {
        if name != "Auto" {
            if let Some(enc) = available.iter().find(|e| e.name == name) {
                return enc;
            }
        }
    }

    // Default: First available (Order matches priority in detect_available_encoders: NVENC > QSV > VAAPI > Software)
    available.first().expect("No encoders available (not even software)")
}

// API Handlers
pub async fn get_transcode_settings(
    State(pool): State<SqlitePool>,
) -> Result<Json<TranscodeSettings>, AppError> {
    let available = get_available_encoders().await;
    let _active = get_active_encoder(&pool).await;
    
    // Fetch stored preference to distinguish "Auto" from actual active
    let stored_pref: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'transcode_encoder'"
    )
    .fetch_optional(&pool)
    .await?;

    let current = stored_pref.unwrap_or_else(|| "Auto".to_string());

    let thread_count: Option<u32> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'transcode_threads'")
        .fetch_optional(&pool).await.unwrap_or(None).and_then(|v: String| v.parse().ok());
    
    let preset: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'transcode_preset'")
        .fetch_optional(&pool).await.unwrap_or(None);

    let throttle_transcodes = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'transcode_throttle'")
        .fetch_optional(&pool).await.unwrap_or(None).map(|v: String| v == "true").unwrap_or(false);

    let max_bitrate: Option<u32> = sqlx::query_scalar("SELECT value FROM settings WHERE key = 'transcode_bitrate'")
        .fetch_optional(&pool).await.unwrap_or(None).and_then(|v: String| v.parse().ok());

    let mut names: Vec<String> = vec!["Auto".to_string()];
    names.extend(available.iter().map(|e| e.name.to_string()));

    let system_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);

    Ok(Json(TranscodeSettings {
        current_encoder: current,
        available_encoders: names,
        thread_count,
        preset,
        throttle_transcodes,
        max_bitrate,
        system_threads,
    }))
}

pub async fn update_transcode_settings(
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpdateTranscodeSettings>,
) -> Result<Json<TranscodeSettings>, AppError> {
    // Validate
    let available = get_available_encoders().await;
    let valid = payload.encoder == "Auto" || available.iter().any(|e| e.name == payload.encoder);
    
    if !valid {
        return Err(AppError::BadRequest("Invalid encoder selected".to_string()));
    }

    // Upsert encoder
    sqlx::query("INSERT INTO settings (key, value) VALUES ('transcode_encoder', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(&payload.encoder)
        .execute(&pool).await?;

    // Upsert threads
    let threads_val = payload.thread_count.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string());
    sqlx::query("INSERT INTO settings (key, value) VALUES ('transcode_threads', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(threads_val)
        .execute(&pool).await?;

    // Upsert preset
    let preset_val = payload.preset.unwrap_or_else(|| "".to_string());
    sqlx::query("INSERT INTO settings (key, value) VALUES ('transcode_preset', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(preset_val)
        .execute(&pool).await?;

    // Upsert throttle
    let throttle_val = if payload.throttle_transcodes { "true" } else { "false" };
    sqlx::query("INSERT INTO settings (key, value) VALUES ('transcode_throttle', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(throttle_val)
        .execute(&pool).await?;

    // Upsert bitrate
    let bitrate_val = payload.max_bitrate.map(|v| v.to_string()).unwrap_or_else(|| "0".to_string());
    sqlx::query("INSERT INTO settings (key, value) VALUES ('transcode_bitrate', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(bitrate_val)
        .execute(&pool).await?;

    get_transcode_settings(State(pool)).await
}



/// Get stream info for a media file - frontend uses this to decide playback method
pub async fn get_stream_info(
    Path(id): Path<i64>,
    State(pool): State<SqlitePool>,
    Json(profile): Json<crate::services::transcode::codecs::DeviceProfile>,
) -> Result<Json<StreamInfo>, AppError> {
    use crate::services::transcode::TranscodeService;
    
    let service = TranscodeService::new(pool);
    let info = service.get_stream_info(id, Some(profile)).await?;
    
    Ok(Json(StreamInfo {
        needs_transcode: info.needs_transcode,
        video_codec: info.video_codec,
        audio_codec: info.audio_codec,
        container: info.container,
        direct_stream_url: info.direct_stream_url,
        hls_url: info.hls_url,
        duration_seconds: info.duration_seconds,
    }))
}

/// Get HLS master playlist - starts transcoding if not already running
/// Supports fast seeking via `start` query param (seconds)
pub async fn get_hls_playlist(
    Path(id): Path<i64>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(pool): State<SqlitePool>,
) -> Result<impl IntoResponse, AppError> {
    use crate::services::transcode::TranscodeService;
    
    let service = TranscodeService::new(pool);
    let transcode_video = params.get("video_transcode").map(|v| v == "true").unwrap_or(false);
    let transcode_audio = params.get("audio_transcode").map(|v| v == "true").unwrap_or(false);
    let start_time = params.get("start").and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
    
    let playlist_content = service.get_hls_playlist(id, transcode_video, transcode_audio, start_time).await?;

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/vnd.apple.mpegurl".parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());

    Ok((headers, playlist_content))
}

/// Serve HLS segment files
pub async fn get_hls_segment(
    Path((id, segment)): Path<(i64, String)>,
) -> Result<impl IntoResponse, AppError> {
    let segment_path = PathBuf::from("transcode")
        .join(id.to_string())
        .join(&segment);

    // Security check - allow fMP4 segments (.m4s, .mp4) and legacy TS
    if !segment.ends_with(".ts") && !segment.ends_with(".m3u8") 
        && !segment.ends_with(".m4s") && !segment.ends_with(".mp4") {
        return Err(AppError::BadRequest("Invalid segment".to_string()));
    }

    // Wait for segment to be ready (max 5 seconds)
    let start = std::time::Instant::now();
    while !segment_path.exists() && start.elapsed().as_secs() < 5 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    if !segment_path.exists() {
        return Err(AppError::NotFound("Segment not found".to_string()));
    }

    let content = tokio::fs::read(&segment_path).await
        .map_err(|e| AppError::Internal(format!("Failed to read segment: {}", e)))?;

    let mime = if segment.ends_with(".ts") {
        "video/mp2t"
    } else if segment.ends_with(".m4s") || segment.ends_with(".mp4") {
        "video/mp4"
    } else {
        "application/vnd.apple.mpegurl"
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    headers.insert(header::CACHE_CONTROL, "max-age=3600".parse().unwrap());

    Ok((headers, content))
}

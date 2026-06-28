//! Transcode service
//!
//! High-level service for managing media transcoding operations.
//! Shared as application state so FFmpeg sessions persist across requests.
//! Generates VOD-style playlists covering full media duration.
//! FFmpeg is started on-demand when segments are requested.

use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use dashmap::DashMap;
use sqlx::SqlitePool;
use tokio::process::Child;
use crate::error::AppError;
use super::codecs::{probe_media, DeviceProfile, HardwareAccelerationType, PlayMethod};
use super::stream_builder::StreamBuilder;
use super::ffmpeg::HlsGenerator;
use super::profiles::TranscodingContext;
use crate::infrastructure::config::config;

const SESSION_TIMEOUT_SECS: u64 = 3600;

/// Identifies a unique transcode job. Every field that changes the produced
/// bytes must be included, so two requests that differ on any of them get
/// independent FFmpeg sessions and cache dirs instead of colliding on a
/// shared `media_id`. This is what makes concurrent multi-user playback of
/// the same title correct.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct JobKey {
    pub media_id: i64,
    pub transcode_video: bool,
    pub transcode_audio: bool,
    /// Requested audio track (None = server default). Keyed on the *requested*
    /// value so the handler and service agree without re-probing.
    pub audio_stream_index: Option<usize>,
    pub max_streaming_bitrate: Option<i32>,
    pub hwa: Option<HardwareAccelerationType>,
}

impl JobKey {
    /// Stable directory/session identifier: `{media_id}-{paramhash}`. The
    /// media_id prefix lets cache maintenance recover it without a sidecar.
    pub fn dir_name(&self) -> String {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut h);
        format!("{}-{:016x}", self.media_id, h.finish())
    }

    pub fn cache_dir(&self) -> PathBuf {
        config().transcode_dir.join(self.dir_name())
    }

    /// Recover the media_id encoded in a cache dir name, if any.
    fn media_id_from_dir(name: &str) -> Option<i64> {
        name.split('-').next()?.parse().ok()
    }
}

struct TranscodeSession {
    child: Child,
    started_at: Instant,
    start_segment: usize,
}

#[derive(Clone)]
pub struct TranscodeService {
    pool: SqlitePool,
    sessions: Arc<DashMap<String, TranscodeSession>>,
    // Serializes ensure_init/ensure_segment per job so concurrent seek
    // requests can't kill/restart each other's FFmpeg session mid-write.
    locks: Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

#[derive(Debug, Clone)]
pub struct StreamInfo {
    pub needs_transcode: bool,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub direct_stream_url: String,
    pub hls_url: Option<String>,
    pub duration_seconds: Option<f64>,
}

impl TranscodeService {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            sessions: Arc::new(DashMap::new()),
            locks: Arc::new(DashMap::new()),
        }
    }

    pub async fn get_file_path(&self, media_id: i64) -> Result<String, AppError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media_items WHERE id = ?")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;

        result
            .map(|(path,)| path)
            .ok_or_else(|| AppError::MediaNotFound(media_id))
    }

    pub async fn get_stream_info(&self, media_id: i64, profile: Option<DeviceProfile>) -> Result<StreamInfo, AppError> {
        let file_path = self.get_file_path(media_id).await?;
        let probe = probe_media(&file_path).await?;

        let profile = profile.unwrap_or_default();
        let (play_method, _reason) = StreamBuilder::determine_play_method(
             &probe.media_info,
             &profile,
             None,
             None,
        );

        let requires_transcode = play_method == PlayMethod::Transcode;

        Ok(StreamInfo {
            needs_transcode: requires_transcode,
            video_codec: probe.video_codec,
            audio_codec: probe.audio_codec,
            container: probe.container,
            direct_stream_url: format!("/api/v1/stream/{}", media_id),
            hls_url: if requires_transcode {
                Some(format!("/api/v1/stream/{}/hls/master.m3u8?video_transcode=true&audio_transcode=true", media_id))
            } else {
                None
            },
            duration_seconds: probe.duration_seconds,
        })
    }

    /// Generate a VOD playlist covering the full media duration.
    /// Segments are served on-demand — FFmpeg starts when a segment is requested.
    /// If `start_offset` is provided, adds #EXT-X-START for resume playback.
    /// `segment_query` is appended to every segment / init URI (without a leading
    /// `?`) so transcode parameters such as `audio_index` survive into the
    /// segment requests — HLS players resolve segment URIs relative to the
    /// playlist but do not inherit its query string.
    pub async fn generate_playlist(
        &self,
        media_id: i64,
        start_offset: Option<f64>,
        segment_query: &str,
    ) -> Result<String, AppError> {
        let file_path = self.get_file_path(media_id).await?;
        let probe = probe_media(&file_path).await?;
        let duration = probe.duration_seconds
            .ok_or_else(|| AppError::Internal("Unknown media duration".to_string()))?;

        let cfg = config();
        let segment_time = cfg.hls_segment_time as f64;
        let mut total_segments = (duration / segment_time).ceil() as usize;

        // Container metadata duration usually exceeds the last decodable
        // frame, so a tiny trailing sliver would reference a segment FFmpeg
        // never emits. Fold it into the previous segment instead.
        if total_segments > 1 {
            let last_len = duration - (total_segments - 1) as f64 * segment_time;
            if last_len < 0.5 {
                total_segments -= 1;
            }
        }
        let total_segments = total_segments.max(1);

        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:7\n");
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        // +1 covers the folded final segment and keyframe-cut jitter;
        // TARGETDURATION must be >= the longest EXTINF, rounded up.
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", cfg.hls_segment_time + 1));
        playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");

        if let Some(offset) = start_offset {
            if offset > 0.0 && offset < duration {
                playlist.push_str(&format!("#EXT-X-START:TIME-OFFSET={:.3},PRECISE=YES\n", offset));
            }
        }

        let suffix = if segment_query.is_empty() {
            String::new()
        } else {
            format!("?{}", segment_query)
        };

        playlist.push_str(&format!("#EXT-X-MAP:URI=\"init.mp4{}\"\n", suffix));

        for i in 0..total_segments {
            let seg_duration = if i == total_segments - 1 {
                duration - (i as f64 * segment_time)
            } else {
                segment_time
            };
            playlist.push_str(&format!("#EXTINF:{:.3},\n", seg_duration));
            playlist.push_str(&format!("segment_{:05}.m4s{}\n", i, suffix));
        }

        playlist.push_str("#EXT-X-ENDLIST\n");
        Ok(playlist)
    }

    /// Ensure a segment is available. Starts/restarts FFmpeg if needed.
    pub async fn ensure_segment(
        &self,
        key: &JobKey,
        segment_index: usize,
        profile: DeviceProfile,
    ) -> Result<(), AppError> {
        let dir_name = key.dir_name();
        let lock = self.job_lock(&dir_name);
        let _guard = lock.lock().await;

        let cfg = config();
        let segment_path = key.cache_dir()
            .join(format!("segment_{:05}.m4s", segment_index));

        // A matching cache file is always valid: the job key already pins the
        // audio track, codecs and bitrate, so there is no stale-params case.
        if segment_path.exists() {
            return Ok(());
        }

        let out_of_window = {
            if let Some(session) = self.sessions.get(&dir_name) {
                segment_index < session.start_segment
                    || segment_index > session.start_segment + 50
            } else {
                true
            }
        };
        // A dead session can't produce new segments (crash, or EOF before
        // reaching this index) — restart once instead of waiting on a file
        // that will never appear.
        let needs_restart = out_of_window || self.session_exited(&dir_name);

        if needs_restart {
            self.kill_session(&dir_name).await;

            let file_path = self.get_file_path(key.media_id).await?;
            let segment_time = cfg.hls_segment_time as f64;
            let start_time = segment_index as f64 * segment_time;

            self.start_hls(key, file_path, start_time, profile).await?;
        }

        // Wait for the segment file
        let timeout = cfg.segment_wait_timeout as u64;
        let start = std::time::Instant::now();
        let mut delay = std::time::Duration::from_millis(50);
        while !segment_path.exists() && start.elapsed().as_secs() < timeout {
            // Segments are finalized via temp_file renames, so once FFmpeg
            // has exited a missing segment will never appear — fail fast
            // instead of burning the full timeout.
            if self.session_exited(&dir_name) {
                if segment_path.exists() {
                    return Ok(());
                }
                tracing::warn!(
                    "FFmpeg for media {} exited without producing segment {}",
                    key.media_id, segment_index
                );
                return Err(AppError::NotFound("Segment past end of stream".to_string()));
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(std::time::Duration::from_millis(500));
        }

        if !segment_path.exists() {
            return Err(AppError::NotFound("Segment not ready in time".to_string()));
        }

        Ok(())
    }

    /// Ensure the init segment exists. Starts FFmpeg from the beginning if needed.
    pub async fn ensure_init(
        &self,
        key: &JobKey,
        profile: DeviceProfile,
    ) -> Result<(), AppError> {
        let dir_name = key.dir_name();
        let lock = self.job_lock(&dir_name);
        let _guard = lock.lock().await;

        let cfg = config();
        let init_path = key.cache_dir().join("init.mp4");

        // The job key pins audio track and codecs, so a present init blob is
        // always valid for this request — no cross-request restart needed.
        if init_path.exists() {
            return Ok(());
        }

        if !self.sessions.contains_key(&dir_name) {
            let file_path = self.get_file_path(key.media_id).await?;
            self.start_hls(key, file_path, 0.0, profile).await?;
        }

        let timeout = cfg.segment_wait_timeout as u64;
        let start = std::time::Instant::now();
        let mut delay = std::time::Duration::from_millis(50);
        while !init_path.exists() && start.elapsed().as_secs() < timeout {
            if self.session_exited(&dir_name) && !init_path.exists() {
                tracing::warn!("FFmpeg for media {} exited without producing init segment", key.media_id);
                return Err(AppError::NotFound("Init segment not ready".to_string()));
            }
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(std::time::Duration::from_millis(500));
        }

        if !init_path.exists() {
            return Err(AppError::NotFound("Init segment not ready".to_string()));
        }

        Ok(())
    }

    fn job_lock(&self, dir_name: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.locks
            .entry(dir_name.to_string())
            .or_default()
            .clone()
    }

    /// True if a session exists for this job and its FFmpeg process has exited.
    fn session_exited(&self, dir_name: &str) -> bool {
        self.sessions
            .get_mut(dir_name)
            .map_or(false, |mut s| matches!(s.child.try_wait(), Ok(Some(_))))
    }

    async fn kill_session(&self, dir_name: &str) {
        if let Some((_, mut session)) = self.sessions.remove(dir_name) {
            let _ = session.child.kill().await;
            tracing::info!("Killed transcode session {}", dir_name);
        }

        let session_dir = config().transcode_dir.join(dir_name);
        let _ = tokio::fs::remove_dir_all(&session_dir).await;
    }

    async fn start_hls(
        &self,
        key: &JobKey,
        file_path: String,
        mut start_time: f64,
        profile: DeviceProfile,
    ) -> Result<(), AppError> {
        let media_id = key.media_id;
        let dir_name = key.dir_name();

        let probe = probe_media(&file_path).await?;
        let generator = HlsGenerator::new(media_id, file_path.clone(), key.cache_dir());
        generator.prepare().await?;

        let cfg = config();
        let segment_time = cfg.hls_segment_time as f64;

        // Clamp near-end seeks: container duration is metadata and often
        // exceeds the last decodable frame, so -ss into the final segments
        // can land past EOF and produce nothing. Back up two segments
        // (grid-aligned) so the EOF flush still emits the requested numbers.
        if let Some(duration) = probe.duration_seconds {
            let latest_start = ((duration - 2.0 * segment_time).max(0.0) / segment_time).floor() * segment_time;
            if start_time > latest_start {
                start_time = latest_start;
            }
        }

        let start_number = (start_time / segment_time).floor() as usize;

        let source_bitrate = probe.media_info.bit_rate.unwrap_or(8_000_000);
        // Cap bitrate by device profile and source
        let max_transcode_bitrate = std::cmp::min(
            profile.max_streaming_bitrate.unwrap_or(8_000_000) as i64,
            std::cmp::min(source_bitrate, 20_000_000),
        );

        let audio_idx = key.audio_stream_index
            .unwrap_or_else(|| probe.audio_stream_index.unwrap_or(0));

        let video_fps = parse_frame_rate(
            probe.media_info.video.as_ref().and_then(|v| v.frame_rate.as_deref())
        );

        let context = TranscodingContext {
            hwa_type: profile.hardware_acceleration.unwrap_or_default(),
            video_stream_index: probe.video_stream_index,
            audio_stream_index: audio_idx,
            max_video_bitrate: max_transcode_bitrate,
            audio_bitrate: profile.music_streaming_transcoding_bitrate.unwrap_or(192_000) as i64,
            audio_channels: profile.max_audio_channels.unwrap_or(2),
            start_time,
            start_number,
            is_video_transcode: key.transcode_video,
            is_audio_transcode: key.transcode_audio,
            video_fps,
        };

        let mut child = generator.start(context).await?;

        if !generator.wait_for_ready(&mut child).await {
            let _ = child.kill().await;
            let _ = tokio::fs::remove_dir_all(&key.cache_dir()).await;
            return Err(AppError::Internal("FFmpeg failed to produce playlist in time".to_string()));
        }

        self.sessions.insert(dir_name, TranscodeSession {
            child,
            started_at: Instant::now(),
            start_segment: start_number,
        });

        Ok(())
    }

    pub async fn cleanup_stale_sessions(&self) {
        let stale_ids: Vec<String> = self.sessions.iter()
            .filter(|entry| entry.value().started_at.elapsed().as_secs() > SESSION_TIMEOUT_SECS)
            .map(|entry| entry.key().clone())
            .collect();

        for id in stale_ids {
            self.kill_session(&id).await;
            tracing::info!("Cleaned up stale transcode session {}", id);
        }
    }

    /// Remove orphaned cache dirs that have no active session (e.g. from crashes).
    async fn cleanup_orphaned_dirs(&self) {
        let cfg = config();
        let mut read_dir = match tokio::fs::read_dir(&cfg.transcode_dir).await {
            Ok(rd) => rd,
            Err(_) => return,
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            if let Ok(ft) = entry.file_type().await {
                if ft.is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        // Dir names are job keys (`{media_id}-{hash}`); a dir
                        // with no live session is leftover from a crash/restart.
                        if JobKey::media_id_from_dir(name).is_some()
                            && !self.sessions.contains_key(name)
                        {
                            let _ = tokio::fs::remove_dir_all(entry.path()).await;
                            tracing::info!("Removed orphaned cache dir {}", name);
                        }
                    }
                }
            }
        }
    }

    /// Calculate total size of the transcode cache directory in bytes.
    async fn cache_size_bytes(&self) -> u64 {
        let cfg = config();
        dir_size(&cfg.transcode_dir).await
    }

    /// Evict inactive session dirs (oldest first) until cache is under the limit.
    async fn enforce_cache_limit(&self) {
        let cfg = config();
        if cfg.max_cache_size_mb == 0 {
            return;
        }

        let max_bytes = cfg.max_cache_size_mb * 1024 * 1024;
        let current = self.cache_size_bytes().await;
        if current <= max_bytes {
            return;
        }

        tracing::info!(
            "Transcode cache {}MB exceeds limit {}MB, evicting...",
            current / (1024 * 1024),
            cfg.max_cache_size_mb
        );

        // Collect job dirs with modification time, sorted oldest first
        let mut dirs: Vec<(String, std::time::SystemTime, u64)> = Vec::new();
        if let Ok(mut read_dir) = tokio::fs::read_dir(&cfg.transcode_dir).await {
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let (Some(name), Ok(ft)) = (entry.file_name().to_str().map(String::from), entry.file_type().await) {
                    if ft.is_dir() && JobKey::media_id_from_dir(&name).is_some() {
                        let mtime = entry.metadata().await
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        let size = dir_size(&entry.path()).await;
                        dirs.push((name, mtime, size));
                    }
                }
            }
        }

        // Evict oldest first, prefer dirs without an active session
        dirs.sort_by(|a, b| {
            let a_active = self.sessions.contains_key(&a.0);
            let b_active = self.sessions.contains_key(&b.0);
            a_active.cmp(&b_active).then(a.1.cmp(&b.1))
        });

        let mut freed: u64 = 0;
        let need_to_free = current - max_bytes;

        for (dir_name, _, size) in &dirs {
            if freed >= need_to_free {
                break;
            }
            self.kill_session(dir_name).await;
            freed += size;
            tracing::info!("Evicted cache {} ({}MB)", dir_name, size / (1024 * 1024));
        }
    }

    /// Run all periodic maintenance: stale sessions, orphaned dirs, cache limit.
    pub async fn run_maintenance(&self) {
        self.cleanup_stale_sessions().await;
        self.cleanup_orphaned_dirs().await;
        self.enforce_cache_limit().await;
    }

    /// Spawn a background task that runs maintenance every 60 seconds.
    pub fn spawn_maintenance_task(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                service.run_maintenance().await;
            }
        });
    }

    pub async fn shutdown_all(&self) {
        let ids: Vec<String> = self.sessions.iter().map(|e| e.key().clone()).collect();
        for id in ids {
            self.kill_session(&id).await;
        }
    }
}

/// Parse ffprobe's `r_frame_rate` (e.g. "24000/1001" or "25") into fps.
/// Falls back to 24.0 when missing or malformed.
fn parse_frame_rate(rate: Option<&str>) -> f64 {
    rate.and_then(|r| {
        let mut parts = r.splitn(2, '/');
        let num: f64 = parts.next()?.trim().parse().ok()?;
        let den: f64 = match parts.next() {
            Some(d) => d.trim().parse().ok()?,
            None => 1.0,
        };
        if num > 0.0 && den > 0.0 {
            Some(num / den)
        } else {
            None
        }
    })
    .unwrap_or(24.0)
}

async fn dir_size(path: &std::path::Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if let Ok(mut read_dir) = tokio::fs::read_dir(&dir).await {
            while let Ok(Some(entry)) = read_dir.next_entry().await {
                if let Ok(meta) = entry.metadata().await {
                    if meta.is_dir() {
                        stack.push(entry.path());
                    } else {
                        total += meta.len();
                    }
                }
            }
        }
    }
    total
}

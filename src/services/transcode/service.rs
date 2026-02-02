//! Transcode service
//! 
//! High-level service for managing media transcoding operations.
//! Implements Jellyfin-style fast seeking with session management.

use std::path::PathBuf;
use std::sync::Arc;
use dashmap::DashMap;
use sqlx::SqlitePool;
use tokio::process::Child;
use crate::error::AppError;
use super::codecs::{probe_media, MediaProbeResult, DeviceProfile, PlayMethod, TranscodeReason, HardwareAccelerationType};
use super::stream_builder::StreamBuilder;
use super::ffmpeg::HlsGenerator;
use crate::infrastructure::config::config;

/// Information about media stream for frontend decision-making
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

/// Active transcode session
struct TranscodeSession {
    child: Child,
    start_time: f64,
    output_dir: PathBuf,
}

/// Transcode service for managing media streaming
/// Uses DashMap for concurrent session management (kill-on-seek)
pub struct TranscodeService {
    pool: SqlitePool,
    sessions: Arc<DashMap<i64, TranscodeSession>>,
}

impl TranscodeService {
    /// Create a new transcode service
    pub fn new(pool: SqlitePool) -> Self {
        Self { 
            pool,
            sessions: Arc::new(DashMap::new()),
        }
    }

    /// Get the file path for a media item
    pub async fn get_file_path(&self, media_id: i64) -> Result<String, AppError> {
        let result: Option<(String,)> = sqlx::query_as("SELECT file_path FROM media WHERE id = ?")
            .bind(media_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::from)?;

        result
            .map(|(path,)| path)
            .ok_or_else(|| AppError::MediaNotFound(media_id))
    }

    /// Get stream info for a media file
    pub async fn get_stream_info(&self, media_id: i64, profile: Option<DeviceProfile>) -> Result<StreamInfo, AppError> {
        let file_path = self.get_file_path(media_id).await?;
        let probe = probe_media(&file_path).await?;
        
        let profile = profile.unwrap_or_default();
        let (play_method, reason) = StreamBuilder::determine_play_method(
             &probe.media_info, 
             &profile, 
             None,
             None // Subtitle index (none selected for basic info)
        );
        
        let mut requires_transcode = play_method == PlayMethod::Transcode;
        
        // Determine granular needs for logging and URL generation
        let mut video_needs = false;
        let mut audio_needs = false;
        
        match play_method {
            PlayMethod::Transcode => {
                // If transcode, check if it's due to video or strictly audio/container
                if !StreamBuilder::is_video_compatible(&probe.media_info, &profile) {
                    video_needs = true;
                }
                // Force audio transcode if video is transcoded (safety) or if audio incompatible
                // But generally StreamBuilder tells us `reason`. 
                // However, existing behavior expects `audio_needs` flag.
                if !StreamBuilder::is_audio_compatible(&probe.media_info, &profile) {
                    audio_needs = true;
                }
            },
            PlayMethod::DirectStream => {
                 // Remuxing - codecs are fine, container is not.
                 // video_needs = false;
            },
            PlayMethod::DirectPlay => {
                 // All good
            }
        }

        // HARD RULE: HEVC videos >= 15 minutes MUST be transcoded (Vortex Safety)
        let video_codec_lower = probe.video_codec.as_deref().unwrap_or("").to_lowercase();
        let is_hevc = video_codec_lower.contains("hevc") || video_codec_lower.contains("h265");
        let duration_mins = probe.duration_seconds.unwrap_or(0.0) / 60.0;
        
        if is_hevc && duration_mins >= 15.0 {
            if !requires_transcode {
                tracing::warn!(
                    "Media {}: Forcing video transcode for HEVC ({:.1} min >= 15 min threshold)",
                    media_id, duration_mins
                );
            }
            requires_transcode = true;
            video_needs = true;
        }

        // Check container usage for logging
        let container = probe.container.as_deref().unwrap_or("").to_lowercase();
        let container_needs = !profile.containers.iter().any(|c| container.contains(c));
        
        if requires_transcode {
             tracing::info!("Media {} decision: TRANSCODE. Reason: {:?}. Video={}, Audio={}", 
                 media_id, reason, video_needs, audio_needs);
        } else {
             tracing::info!("Media {} decision: {:?} / STREAM", media_id, play_method);
        }

        if requires_transcode {
             tracing::info!("Media {} decision: TRANSCODE. Video={}, Audio={}, Container={}", 
                 media_id, video_needs, audio_needs, container_needs);
        } else {
             tracing::info!("Media {} decision: DIRECT PLAY", media_id);
        }

        Ok(StreamInfo {
            needs_transcode: requires_transcode,
            video_codec: probe.video_codec,
            audio_codec: probe.audio_codec,
            container: probe.container,
            direct_stream_url: format!("/api/v1/stream/{}", media_id),
            hls_url: if requires_transcode {
                let mut rules = Vec::new();
                if video_needs { rules.push("video_transcode=true"); }
                if audio_needs { rules.push("audio_transcode=true"); }
                
                let query = if rules.is_empty() { String::new() } else { format!("?{}", rules.join("&")) };
                Some(format!("/api/v1/stream/{}/hls/master.m3u8{}", media_id, query))
            } else {
                None
            },
            duration_seconds: probe.duration_seconds,
        })
    }

    /// Generate HLS playlist for a media file (with Jellyfin-style fast seeking)
    pub async fn get_hls_playlist(
        &self, 
        media_id: i64, 
        transcode_video: bool, 
        transcode_audio: bool,
        start_time_seconds: f64,
    ) -> Result<String, AppError> {
        let file_path = self.get_file_path(media_id).await?;
        let probe = probe_media(&file_path).await?;
        let generator = HlsGenerator::new(media_id, file_path, probe.duration_seconds);
        
        // Match FFmpeg segment duration
        const SEGMENT_DURATION: f64 = 3.0; // Corrected from 6.0
        
        // Calculate start segment index based on seek time
        let start_segment_index = (start_time_seconds / SEGMENT_DURATION).floor() as usize;

        // Check if we need to kill existing session (seek position changed)
        let should_restart = if let Some(session) = self.sessions.get(&media_id) {
            let time_diff = (session.start_time - start_time_seconds).abs();
            time_diff > 5.0  // If seek > 5 seconds from current position, restart
        } else {
            false
        };
        
        if should_restart {
            // Kill existing FFmpeg process
            if let Some((_, mut session)) = self.sessions.remove(&media_id) {
                tracing::info!(
                    "🔪 Killing FFmpeg for media {} (seek from {:.1}s to {:.1}s)",
                    media_id, session.start_time, start_time_seconds
                );
                let _ = session.child.kill().await;
                // Clean up old segments
                if session.output_dir.exists() {
                    let _ = tokio::fs::remove_dir_all(&session.output_dir).await;
                }
            }
        }

        // Prepare output directory
        generator.prepare().await?;

        // Start new FFmpeg if needed
        let session_exists = self.sessions.contains_key(&media_id);
        if !session_exists && !generator.is_ready() {
            // HWA selection logic: Use server config env var (VORTEX_TRANSCODING_HWA)
            let hwa_config = config().transcoding_hwa.as_deref().unwrap_or("none");
            let hwa_type = match hwa_config {
                "vaapi" => HardwareAccelerationType::Vaapi,
                "nvenc" => HardwareAccelerationType::Nvenc,
                "qsv" => HardwareAccelerationType::Qsv,
                "amf" => HardwareAccelerationType::Amf,
                "videotoolbox" => HardwareAccelerationType::VideoToolbox,
                _ => HardwareAccelerationType::None,
            };

            let child = generator.start_process(
                transcode_video, 
                transcode_audio,
                probe.media_info.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("h264"),
                start_time_seconds,
                start_segment_index, // Pass start number to FFmpeg
                hwa_type
            ).await?;
            
            // Store session for kill-on-seek
            self.sessions.insert(media_id, TranscodeSession {
                child,
                start_time: start_time_seconds,
                output_dir: generator.output_dir().clone(),
            });
            
            generator.wait_for_first_segment(start_segment_index).await;
        }

        // Generate dynamic playlist from seek offset
        let playlist = self.generate_dynamic_playlist(
            &generator, 
            probe.duration_seconds.unwrap_or(0.0),
            start_time_seconds,
        );
        
        Ok(playlist)
    }
    
    /// Generate playlist starting from seek offset
    fn generate_dynamic_playlist(
        &self,
        _generator: &HlsGenerator,
        total_duration: f64,
        start_time: f64,
    ) -> String {
        const SEGMENT_DURATION: f64 = 3.0; // Corrected from 6.0
        
        let remaining_duration = total_duration - start_time;
        // Use ceil to include partial last segment
        let num_segments = (remaining_duration / SEGMENT_DURATION).ceil() as i32;
        // Correct index calculation
        let first_segment_index = (start_time / SEGMENT_DURATION).floor() as i32;
        
        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:7\n");
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", SEGMENT_DURATION.ceil() as i32));
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        playlist.push_str(&format!("#EXT-X-MEDIA-SEQUENCE:{}\n", first_segment_index));
        playlist.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");
        
        let mut remaining = remaining_duration;
        for i in 0..num_segments {
            let seg_duration = if remaining >= SEGMENT_DURATION { SEGMENT_DURATION } else { remaining };
            playlist.push_str(&format!("#EXTINF:{:.6},\n", seg_duration));
            playlist.push_str(&format!("segment_{:05}.m4s\n", first_segment_index + i));
            remaining -= seg_duration;
        }
        
        playlist.push_str("#EXT-X-ENDLIST\n");
        
        playlist
    }

    /// Cleanup transcode session
    #[allow(dead_code)]
    pub async fn cleanup_session(&self, media_id: i64) -> Result<(), AppError> {
        if let Some((_, mut session)) = self.sessions.remove(&media_id) {
            let _ = session.child.kill().await;
            if session.output_dir.exists() {
                tokio::fs::remove_dir_all(&session.output_dir).await
                    .map_err(|e| AppError::Internal(format!("Cleanup failed: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Probe a media file and return result
    #[allow(dead_code)]
    pub async fn probe(&self, media_id: i64) -> Result<MediaProbeResult, AppError> {
        let file_path = self.get_file_path(media_id).await?;
        probe_media(&file_path).await
    }
}


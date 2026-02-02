//! FFmpeg HLS generation
//! 
//! Handles FFmpeg process management for HLS segment generation.

use std::path::PathBuf;
use std::process::Stdio;
use crate::infrastructure::config;
use crate::error::AppError;
use super::profiles::TranscodeProfile;

/// HLS Generator for creating fMP4 HLS streams
pub struct HlsGenerator {
    media_id: i64,
    file_path: String,
    output_dir: PathBuf,
    duration_seconds: Option<f64>,
}

/// Segment duration in seconds (matches FFmpeg -hls_time)
const SEGMENT_DURATION: f64 = 3.0;

impl HlsGenerator {
    /// Create a new HLS generator for a media file
    pub fn new(media_id: i64, file_path: String, duration_seconds: Option<f64>) -> Self {
        let cfg = config();
        let output_dir = cfg.transcode_dir.join(media_id.to_string());
        
        Self {
            media_id,
            file_path,
            output_dir,
            duration_seconds,
        }
    }

    /// Get the output directory path
    #[allow(dead_code)]
    pub fn output_dir(&self) -> &PathBuf {
        &self.output_dir
    }

    /// Get the playlist path
    pub fn playlist_path(&self) -> PathBuf {
        self.output_dir.join("master.m3u8")
    }

    /// Get the init file path
    pub fn init_path(&self) -> PathBuf {
        self.output_dir.join("init.mp4")
    }

    /// Check if transcoding is already complete or in progress (init file exists)
    pub fn is_ready(&self) -> bool {
        // Check for init.mp4 which is only created by FFmpeg, not by our pre-generated playlist
        self.init_path().exists()
    }

    /// Ensure output directory exists and generate pre-calculated playlist
    pub async fn prepare(&self) -> Result<(), AppError> {
        if !self.output_dir.exists() {
            tokio::fs::create_dir_all(&self.output_dir)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create transcode dir: {}", e)))?;
        }
        
        // Generate pre-calculated VOD playlist if we have duration
        if let Some(duration) = self.duration_seconds {
            self.generate_vod_playlist(duration).await?;
        }
        
        Ok(())
    }
    
    /// Generate a pre-calculated VOD playlist with all segments
    async fn generate_vod_playlist(&self, duration: f64) -> Result<(), AppError> {
        let num_segments = (duration / SEGMENT_DURATION).ceil() as i32;
        
        let mut playlist = String::new();
        playlist.push_str("#EXTM3U\n");
        playlist.push_str("#EXT-X-VERSION:7\n");
        playlist.push_str(&format!("#EXT-X-TARGETDURATION:{}\n", SEGMENT_DURATION.ceil() as i32));
        playlist.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
        playlist.push_str("#EXT-X-MEDIA-SEQUENCE:0\n");
        playlist.push_str("#EXT-X-MAP:URI=\"init.mp4\"\n");
        
        let mut remaining = duration;
        for i in 0..num_segments {
            let seg_duration = if remaining >= SEGMENT_DURATION { SEGMENT_DURATION } else { remaining };
            playlist.push_str(&format!("#EXTINF:{:.6},\n", seg_duration));
            playlist.push_str(&format!("segment_{:05}.m4s\n", i));
            remaining -= seg_duration;
        }
        
        playlist.push_str("#EXT-X-ENDLIST\n");
        
        tokio::fs::write(&self.playlist_path(), playlist)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to write playlist: {}", e)))?;
        
        tracing::info!("Generated VOD playlist for media {} with {} segments ({:.1}s)", 
            self.media_id, num_segments, duration);
        
        Ok(())
    }

    /// Start FFmpeg process with optional seek offset
    /// Returns Child handle for kill support (session management)
    /// Start FFmpeg process with optional seek offset
    /// Returns Child handle for kill support (session management)
    pub async fn start_process(
        &self, 
        transcode_video: bool, 
        transcode_audio: bool,
        video_codec: &str,
        start_time_seconds: f64,
        start_number: usize,
        hwa_type: super::codecs::HardwareAccelerationType,
    ) -> Result<tokio::process::Child, AppError> {
        let segment_pattern = self.output_dir.join("segment_%05d.m4s");
        let ffmpeg_playlist = self.output_dir.join("ffmpeg_output.m3u8");

        if start_time_seconds > 0.0 {
            tracing::info!(
                "🎯 Starting HLS at SEEK OFFSET {:.1}s (Seg #{}) for media {} (Video: {}, Codec: {}, Audio: {}, HWA: {:?})",
                start_time_seconds, start_number, self.media_id, transcode_video, video_codec, transcode_audio, hwa_type
            );
        } else {
            tracing::info!(
                "Starting HLS for media {} (Seg #{}) (Video: {}, Codec: {}, Audio: {}, HWA: {:?})",
                self.media_id, start_number, transcode_video, video_codec, transcode_audio, hwa_type
            );
        }
        
        match (transcode_video, transcode_audio) {
            (true, true) => tracing::info!("🚀 Mode: FULL TRANSCODE"),
            (true, false) => tracing::info!("🎥 Mode: VIDEO TRANSCODE"),
            (false, true) => tracing::info!("⚡ Mode: AUDIO TRANSCODE"),
            (false, false) => tracing::info!("⏩ Mode: DIRECT STREAM"),
        }

        let args = TranscodeProfile::build_hls_command(
            &self.file_path,
            start_time_seconds,
            transcode_video,
            video_codec,
            transcode_audio,
            start_number,
            segment_pattern.to_str().unwrap(),
            ffmpeg_playlist.to_str().unwrap(),
            "init.mp4", // init file name
            hwa_type,
        );

        tracing::debug!("FFmpeg args: {:?}", args);

        let mut child = tokio::process::Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to start FFmpeg: {}", e)))?;

        // Spawn stderr reader (non-blocking)
        let stderr = child.stderr.take().unwrap();
        let media_id = self.media_id;
        
        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            
            while let Ok(Some(line)) = lines.next_line().await {
                if line.contains("Error") || line.contains("failed") {
                    tracing::error!("FFmpeg [{}]: {}", media_id, line);
                } else {
                    tracing::debug!("FFmpeg [{}]: {}", media_id, line);
                }
            }
        });

        // Return child handle for session management (kill on seek)
        Ok(child)
    }

    /// Wait for first segment to be ready
    pub async fn wait_for_first_segment(&self, start_number: usize) -> bool {
        let cfg = config();
        let init_file = self.init_path();
        let first_segment = self.output_dir.join(format!("segment_{:05}.m4s", start_number));
        
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < cfg.segment_wait_timeout as u64 {
            if init_file.exists() && first_segment.exists() {
                tracing::info!(
                    "First segment ready for media {} in {:?}",
                    self.media_id,
                    start.elapsed()
                );
                return true;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        
        tracing::error!(
            "Timeout waiting for first segment for media {} after {}s",
            self.media_id,
            cfg.segment_wait_timeout
        );
        false
    }
}

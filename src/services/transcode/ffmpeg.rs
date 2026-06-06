//! FFmpeg HLS generation
//!
//! Owns FFmpeg process lifecycle. FFmpeg is the source of truth for playlists.

use std::path::PathBuf;
use std::process::Stdio;
use crate::infrastructure::config;
use crate::error::AppError;
use super::profiles::TranscodingContext;

pub struct HlsGenerator {
    pub media_id: i64,
    pub file_path: String,
    pub output_dir: PathBuf,
}

impl HlsGenerator {
    pub fn new(media_id: i64, file_path: String) -> Self {
        let cfg = config();
        let output_dir = cfg.transcode_dir.join(media_id.to_string());

        Self {
            media_id,
            file_path,
            output_dir,
        }
    }

    pub fn playlist_path(&self) -> PathBuf {
        self.output_dir.join("stream.m3u8")
    }

    pub fn init_path(&self) -> PathBuf {
        self.output_dir.join("init.mp4")
    }

    pub async fn prepare(&self) -> Result<(), AppError> {
        if !self.output_dir.exists() {
            tokio::fs::create_dir_all(&self.output_dir)
                .await
                .map_err(|e| AppError::Internal(format!("Failed to create transcode dir: {}", e)))?;
        }
        Ok(())
    }

    pub async fn start(
        &self,
        context: TranscodingContext,
    ) -> Result<tokio::process::Child, AppError> {
        let segment_pattern = self.output_dir.join("segment_%05d.m4s");

        let args = super::profiles::TranscodeProfile::build_hls_command(
            &self.file_path,
            &context,
            segment_pattern.to_str().unwrap(),
            self.playlist_path().to_str().unwrap(),
            "init.mp4",
        );

        tracing::info!("Spawning FFmpeg: ffmpeg {}", args.join(" "));

        let mut child = tokio::process::Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| AppError::Internal(format!("Failed to start FFmpeg: {}", e)))?;

        let stderr = child.stderr.take().unwrap();
        let media_id = self.media_id;

        tokio::spawn(async move {
            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::debug!("FFmpeg [{}]: {}", media_id, line);
            }
        });

        Ok(child)
    }

    pub async fn wait_for_ready(&self) -> bool {
        let cfg = config();
        let start = std::time::Instant::now();

        while start.elapsed().as_secs() < cfg.segment_wait_timeout as u64 {
            if self.playlist_path().exists() && self.init_path().exists() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        false
    }
}
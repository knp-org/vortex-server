//! Codec detection and compatibility checking
//!
//! Handles media probing via FFprobe and determines browser compatibility.

use serde::Deserialize;
use std::process::Stdio;
use crate::error::AppError;
use crate::models::db::media_info::{MediaInfo, VideoStream, AudioStream, SubtitleStream};

/// Normalize codec names to Jellyfin/FFmpeg standard
pub fn normalize_codec(codec: &str) -> String {
    match codec.trim().to_lowercase().as_str() {
        "avc1" | "avc3" | "h264" => "h264".into(),
        "hev1" | "hvc1" | "h265" | "hevc" => "hevc".into(),
        "mp4a" | "aac" => "aac".into(),
        "dca" | "dts" => "dts".into(),
        "ac-3" | "ac3" => "ac3".into(),
        "ec-3" | "eac3" => "eac3".into(),
        "opus" => "opus".into(),
        "vorbis" => "vorbis".into(),
        "vp8" => "vp8".into(),
        "vp9" => "vp9".into(),
        "av1" => "av1".into(),
        "mp3" => "mp3".into(),
        "flac" => "flac".into(),
        other => other.to_string(),
    }
}

/// Normalize container names
pub fn normalize_container(container: &str) -> String {
    match container.trim().to_lowercase().as_str() {
        "matroska" => "mkv".into(),
        "mpegts" => "ts".into(),
        "mov" | "mp4" | "m4a" | "3gp" | "3g2" | "mj2" => "mp4".into(),
        other => other.to_string(),
    }
}

/// Result of probing a media file
#[derive(Debug, Clone)]
pub struct MediaProbeResult {
    pub video_codec: Option<String>,
    pub video_stream_index: usize,          // Absolute FFmpeg index
    pub audio_codec: Option<String>,
    pub audio_stream_index: Option<usize>,  // Absolute FFmpeg index
    pub container: Option<String>,
    pub duration_seconds: Option<f64>,
    pub media_info: MediaInfo,
}

/// FFprobe output
#[derive(Deserialize)]
struct FFprobeOutput {
    streams: Vec<FFprobeStream>,
    format: Option<FFprobeFormat>,
}

#[derive(Deserialize)]
struct FFprobeStream {
    index: i32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    codec_tag_string: Option<String>,

    // Video
    width: Option<i32>,
    height: Option<i32>,
    profile: Option<String>,
    pix_fmt: Option<String>,
    r_frame_rate: Option<String>,
    bit_rate: Option<String>,
    bits_per_raw_sample: Option<String>,

    // Audio
    channels: Option<i32>,
    channel_layout: Option<String>,
    sample_rate: Option<String>,

    // Metadata
    tags: Option<FFprobeTags>,
    disposition: Option<FFprobeDisposition>,

    // HDR / advanced
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    refs: Option<i32>,
}

#[derive(Deserialize)]
struct FFprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize)]
struct FFprobeDisposition {
    default: Option<i32>,
    forced: Option<i32>,
}

#[derive(Deserialize)]
struct FFprobeFormat {
    format_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    bit_rate: Option<String>,
}

/// Probe media file
pub async fn probe_media(file_path: &str) -> Result<MediaProbeResult, AppError> {
    if !std::path::Path::new(file_path).exists() {
        return Err(AppError::Internal(format!("Media file not found: {}", file_path)));
    }

    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Internal(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }

    let probe: FFprobeOutput =
        serde_json::from_slice(&output.stdout)
            .map_err(|e| AppError::Internal(format!("FFprobe parse error: {}", e)))?;

    let mut video_stream: Option<VideoStream> = None;
    let mut audio_streams = Vec::new();
    let mut subtitle_streams = Vec::new();

    for s in probe.streams {
        match s.codec_type.as_deref() {
            Some("video") if video_stream.is_none() => {
                video_stream = Some(VideoStream {
                    index: s.index,
                    codec: s.codec_name.unwrap_or_default(),
                    profile: s.profile,
                    width: s.width,
                    height: s.height,
                    aspect_ratio: None,
                    bit_rate: s.bit_rate.and_then(|v| v.parse().ok()),
                    frame_rate: s.r_frame_rate,
                    bit_depth: s.bits_per_raw_sample.and_then(|v| v.parse().ok()),
                    pixel_format: s.pix_fmt,
                    color_space: s.color_space,
                    color_transfer: s.color_transfer,
                    color_primaries: s.color_primaries,
                    ref_frames: s.refs,
                    codec_tag: s.codec_tag_string,
                });
            }
            Some("audio") => {
                audio_streams.push(AudioStream {
                    index: s.index,
                    codec: s.codec_name.unwrap_or_default(),
                    channels: s.channels,
                    channel_layout: s.channel_layout,
                    sample_rate: s.sample_rate.and_then(|v| v.parse().ok()),
                    bit_rate: s.bit_rate.and_then(|v| v.parse().ok()),
                    language: s.tags.as_ref().and_then(|t| t.language.clone()),
                    title: s.tags.as_ref().and_then(|t| t.title.clone()),
                    default: s.disposition.as_ref().map(|d| d.default == Some(1)).unwrap_or(false),
                    forced: s.disposition.as_ref().map(|d| d.forced == Some(1)).unwrap_or(false),
                });
            }
            Some("subtitle") => {
                subtitle_streams.push(SubtitleStream {
                    index: s.index,
                    codec: s.codec_name.unwrap_or_default(),
                    language: s.tags.as_ref().and_then(|t| t.language.clone()),
                    title: s.tags.as_ref().and_then(|t| t.title.clone()),
                    is_external: false,
                    is_forced: s.disposition.as_ref().map(|d| d.forced == Some(1)).unwrap_or(false),
                    is_default: s.disposition.as_ref().map(|d| d.default == Some(1)).unwrap_or(false),
                });
            }
            _ => {}
        }
    }

    // Prefer default audio stream
    audio_streams.sort_by_key(|a| !a.default);

    let container = probe.format
        .as_ref()
        .and_then(|f| f.format_name.clone());

    let duration_seconds = probe.format
        .as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse().ok());

    let size = probe.format
        .as_ref()
        .and_then(|f| f.size.as_ref())
        .and_then(|s| s.parse().ok());

    let bit_rate = probe.format
        .as_ref()
        .and_then(|f| f.bit_rate.as_ref())
        .and_then(|s| s.parse().ok());

    let media_info = MediaInfo {
        container: container.clone(),
        size,
        bit_rate,
        video: video_stream.clone(),
        audio: audio_streams.clone(),
        subtitles: subtitle_streams,
        duration: duration_seconds,
    };

    Ok(MediaProbeResult {
        video_codec: video_stream.as_ref().map(|v| v.codec.clone()),
        video_stream_index: video_stream.map(|v| v.index as usize).unwrap_or(0),
        audio_codec: audio_streams.first().map(|a| a.codec.clone()),
        audio_stream_index: audio_streams.first().map(|a| a.index as usize),
        container,
        duration_seconds,
        media_info,
    })
}
/// Method used to play the media
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
pub enum PlayMethod {
    DirectPlay,
    DirectStream,
    Transcode,
}

/// Reason why transcoding is required (Bitflags equivalent)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
pub enum TranscodeReason {
    ContainerNotSupported,
    VideoCodecNotSupported,
    AudioCodecNotSupported,
    ContainerBitrateExceedsLimit,
    AudioBitrateNotSupported,
    AudioChannelsNotSupported,
    VideoResolutionNotSupported,
    SubtitleCodecNotSupported,
    Unknown,
}

/// Defines a combination of Container+Codecs that can be Direct Played
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct DirectPlayProfile {
    pub container: String,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
}

impl DirectPlayProfile {
    pub fn supports_container(&self, container_list: &str) -> bool {
        let input_tokens: Vec<String> = container_list.split(',')
            .map(|s| normalize_container(s))
            .collect();

        self.container.split(',').any(|profile_c| {
            let p = normalize_container(profile_c);
            input_tokens.iter().any(|i| i == &p)
        })
    }

    pub fn supports_video_codec(&self, codec: &str) -> bool {
        let normalized = normalize_codec(codec);
        match &self.video_codec {
            Some(c) => c.split(',').any(|x| normalize_codec(x.trim()) == normalized),
            None => true,
        }
    }

    pub fn supports_audio_codec(&self, codec: &str) -> bool {
        let normalized = normalize_codec(codec);
        match &self.audio_codec {
            Some(c) => c.split(',').any(|x| normalize_codec(x.trim()) == normalized),
            None => true,
        }
    }
}

/// Hardware Acceleration Types supported by Jellyfin/Vortex
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HardwareAccelerationType {
    None,
    Vaapi,
    Nvenc,
    Qsv,
    Amf,
    VideoToolbox,
}

impl Default for HardwareAccelerationType {
    fn default() -> Self {
        Self::None
    }
}

/// Capabilities reported by the client device
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct DeviceProfile {
    pub name: Option<String>,
    pub max_streaming_bitrate: Option<i32>,
    pub max_static_bitrate: Option<i32>,
    pub music_streaming_transcoding_bitrate: Option<i32>,
    pub max_audio_channels: Option<i32>,
    pub hardware_acceleration: Option<HardwareAccelerationType>,

    #[serde(default)]
    pub direct_play_profiles: Vec<DirectPlayProfile>,
    #[serde(default)]
    pub transcoding_profiles: Vec<TranscodingProfile>
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct TranscodingProfile {
    pub container: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub protocol: String,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            name: Some("Generic Browser".to_string()),
            max_streaming_bitrate: Some(120_000_000),
            max_static_bitrate: Some(120_000_000),
            music_streaming_transcoding_bitrate: Some(192000),
            max_audio_channels: Some(2),
            hardware_acceleration: None,

            direct_play_profiles: vec![
                DirectPlayProfile {
                    container: "mp4,m4v".to_string(),
                    video_codec: Some("h264,vp8,vp9,av1".to_string()),
                    audio_codec: Some("aac,mp3,opus,flac".to_string()),
                },
                DirectPlayProfile {
                    container: "webm".to_string(),
                    video_codec: Some("vp8,vp9,av1".to_string()),
                    audio_codec: Some("opus,vorbis".to_string()),
                }
            ],
            transcoding_profiles: vec![
                TranscodingProfile {
                    container: "ts".to_string(),
                    video_codec: "h264".to_string(),
                    audio_codec: "aac".to_string(),
                    protocol: "hls".to_string(),
                }
            ],
        }
    }
}

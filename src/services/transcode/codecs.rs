//! Codec detection and compatibility checking
//! 
//! Handles media probing via FFprobe and determines browser compatibility.

use serde::Deserialize;
use std::process::Stdio;
use crate::error::AppError;
use crate::models::db::media::{MediaInfo, VideoStream, AudioStream, SubtitleStream};

/// Browser-compatible video codecs via MSE (fMP4)
/// Modern browsers support H264, HEVC, AV1, VP9 through Media Source Extensions
#[allow(dead_code)]
const BROWSER_VIDEO_CODECS: &[&str] = &["h264", "hevc", "av1", "vp9", "vp8"];

/// Browser-compatible audio codecs
/// Note: EAC3 (Dolby Digital+), DTS, TrueHD are NOT supported
#[allow(dead_code)]
const BROWSER_AUDIO_CODECS: &[&str] = &["aac", "mp3", "opus", "vorbis", "flac", "ac3"];

/// Result of probing a media file
#[derive(Debug, Clone)]
pub struct MediaProbeResult {
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub container: Option<String>,
    pub duration_seconds: Option<f64>,
    pub media_info: MediaInfo,
}

/// FFprobe output structure
#[derive(Deserialize, Debug)]
struct FFprobeOutput {
    streams: Vec<FFprobeStream>,
    format: Option<FFprobeFormat>,
}

#[derive(Deserialize, Debug)]
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
    tags: Option<FFprobeTags>,
    disposition: Option<FFprobeDisposition>,
    
    // Additional Video
    color_space: Option<String>,
    color_transfer: Option<String>,
    color_primaries: Option<String>,
    refs: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct FFprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(Deserialize, Debug)]
struct FFprobeDisposition {
    default: Option<i32>,
    forced: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct FFprobeFormat {
    format_name: Option<String>,
    #[allow(dead_code)]
    format_long_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    bit_rate: Option<String>,
}

/// Probe a media file to get codec information and duration
pub async fn probe_media(file_path: &str) -> Result<MediaProbeResult, AppError> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            file_path,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(|e| AppError::Internal(format!("Failed to run ffprobe: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Internal("FFprobe failed".to_string()));
    }

    let probe: FFprobeOutput = serde_json::from_slice(&output.stdout)
        .map_err(|e| AppError::Internal(format!("Failed to parse ffprobe output: {}", e)))?;

    let mut video_stream = None;
    let mut audio_streams = Vec::new();
    let mut subtitle_streams = Vec::new();

    for stream in probe.streams {
        if stream.codec_type.as_deref() == Some("video") && video_stream.is_none() {
            video_stream = Some(VideoStream {
                codec: stream.codec_name.unwrap_or_default(),
                profile: stream.profile,
                width: stream.width,
                height: stream.height,
                aspect_ratio: None,
                bit_rate: stream.bit_rate.clone().and_then(|s| s.parse().ok()),
                frame_rate: stream.r_frame_rate,
                bit_depth: stream.bits_per_raw_sample.and_then(|s| s.parse().ok()),
                pixel_format: stream.pix_fmt,
                color_space: stream.color_space,
                color_transfer: stream.color_transfer,
                color_primaries: stream.color_primaries,
                ref_frames: stream.refs,
                codec_tag: stream.codec_tag_string,
            });
        } else if stream.codec_type.as_deref() == Some("audio") {
            audio_streams.push(AudioStream {
                index: stream.index,
                codec: stream.codec_name.unwrap_or_default(),
                channels: stream.channels,
                channel_layout: stream.channel_layout,
                sample_rate: stream.sample_rate.and_then(|s| s.parse().ok()),
                bit_rate: stream.bit_rate.clone().and_then(|s| s.parse().ok()),
                language: stream.tags.as_ref().and_then(|t| t.language.clone()),
                title: stream.tags.as_ref().and_then(|t| t.title.clone()),
                default: stream.disposition.as_ref().map(|d| d.default == Some(1)).unwrap_or(false),
                forced: stream.disposition.as_ref().map(|d| d.forced == Some(1)).unwrap_or(false),
            });
        } else if stream.codec_type.as_deref() == Some("subtitle") {
            subtitle_streams.push(SubtitleStream {
                index: stream.index,
                codec: stream.codec_name.unwrap_or_default(),
                language: stream.tags.as_ref().and_then(|t| t.language.clone()),
                title: stream.tags.as_ref().and_then(|t| t.title.clone()),
                is_external: false, // FFprobe only sees embedded streams here
                is_forced: stream.disposition.as_ref().map(|d| d.forced == Some(1)).unwrap_or(false),
                is_default: stream.disposition.as_ref().map(|d| d.default == Some(1)).unwrap_or(false),
            });
        }
    }

    let container = probe.format.as_ref().and_then(|f| f.format_name.clone());
    let duration_seconds = probe.format.as_ref()
        .and_then(|f| f.duration.as_ref())
        .and_then(|d| d.parse::<f64>().ok());
    let size = probe.format.as_ref().and_then(|f| f.size.as_ref()).and_then(|s| s.parse().ok());
    let bit_rate = probe.format.as_ref().and_then(|f| f.bit_rate.as_ref()).and_then(|s| s.parse().ok());

    let media_info = MediaInfo {
        container: container.clone(),
        size,
        bit_rate,
        video: video_stream.clone(),
        audio: audio_streams.clone(),
        subtitles: subtitle_streams.clone(),
        duration: duration_seconds,
    };

    Ok(MediaProbeResult {
        video_codec: video_stream.map(|v| v.codec),
        audio_codec: audio_streams.first().map(|a| a.codec.clone()),
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
    pub container: String,          // e.g. "mp4,m4v"
    pub video_codec: Option<String>, // e.g. "h264,hevc"
    pub audio_codec: Option<String>, // e.g. "aac,ac3"
}

impl DirectPlayProfile {
    pub fn supports_container(&self, container: &str) -> bool {
        self.container.split(',').any(|c| c.trim().eq_ignore_ascii_case(container))
    }

    pub fn supports_video_codec(&self, codec: &str) -> bool {
        match &self.video_codec {
            Some(c) => c.split(',').any(|x| x.trim().eq_ignore_ascii_case(codec)),
            None => true, // If None, implies all are supported (or not applicable)
        }
    }

    pub fn supports_audio_codec(&self, codec: &str) -> bool {
        match &self.audio_codec {
            Some(c) => c.split(',').any(|x| x.trim().eq_ignore_ascii_case(codec)),
            None => true,
        }
    }
}

/// Hardware Acceleration Types supported by Jellyfin/Vortex
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
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
    
    // Hardware acceleration preference (usually set by server config, but can be part of profile context)
    pub hwa_type: Option<HardwareAccelerationType>,
    
    pub direct_play_profiles: Vec<DirectPlayProfile>,
    pub transcoding_profiles: Vec<TranscodingProfile>,
    
    // Legacy fields (kept for backward compatibility or simple checks)
    pub video_codecs: Vec<String>,
    pub audio_codecs: Vec<String>,
    pub containers: Vec<String>, 
}

#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct TranscodingProfile {
    pub container: String,
    pub video_codec: String,
    pub audio_codec: String,
    pub protocol: String, // "hls", "http"
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            name: Some("Generic Browser".to_string()),
            max_streaming_bitrate: Some(120_000_000), // 120Mbps
            max_static_bitrate: Some(120_000_000),
            music_streaming_transcoding_bitrate: Some(192000),
            hwa_type: None, // Default to software decoding
            
            direct_play_profiles: vec![
                DirectPlayProfile {
                    container: "mp4,m4v".to_string(),
                    video_codec: Some("h264,vp8,vp9,av1".to_string()), // Modern browser defaults
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
            
            video_codecs: vec!["h264".to_string(), "vp8".to_string(), "vp9".to_string()],
            audio_codecs: vec!["aac".to_string(), "mp3".to_string(), "opus".to_string(), "vorbis".to_string()],
            containers: vec!["mp4".to_string(), "webm".to_string()],
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_transcode_h264_aac() {
        let (video, audio) = needs_transcode(&Some("h264".to_string()), &Some("aac".to_string()));
        assert!(!video);
        assert!(!audio);
    }

    #[test]
    fn test_needs_transcode_av1_eac3() {
        let (video, audio) = needs_transcode(&Some("av1".to_string()), &Some("eac3".to_string()));
        assert!(!video); // AV1 is supported
        assert!(audio);  // EAC3 needs transcode
    }

    #[test]
    fn test_needs_transcode_mpeg4_dts() {
        let (video, audio) = needs_transcode(&Some("mpeg4".to_string()), &Some("dts".to_string()));
        assert!(video);  // MPEG4 not in list
        assert!(audio);  // DTS needs transcode
    }
}

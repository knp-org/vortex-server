use serde::{Deserialize, Serialize};

// Detailed media info produced by the ffprobe path in `services::transcode`.
// (The legacy wide `Media`/`PlaybackProgress` rows from the old single-table design
// have been removed; per-type rows now live in their own modules.)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MediaInfo {
    pub container: Option<String>,
    pub size: Option<i64>,
    pub bit_rate: Option<i64>,
    pub video: Option<VideoStream>,
    pub audio: Vec<AudioStream>,
    pub subtitles: Vec<SubtitleStream>,
    pub duration: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoStream {
    pub index: i32,
    pub codec: String,
    pub profile: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub aspect_ratio: Option<String>,
    pub bit_rate: Option<i64>,
    pub frame_rate: Option<String>,
    pub bit_depth: Option<i32>,
    pub pixel_format: Option<String>,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub ref_frames: Option<i32>,
    pub codec_tag: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioStream {
    pub index: i32,
    pub codec: String,
    pub channels: Option<i32>,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
    pub bit_rate: Option<i64>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleStream {
    pub index: i32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_external: bool,
    pub is_forced: bool,
    pub is_default: bool,
}

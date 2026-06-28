//! Transcoding service module
//! 
//! Provides media transcoding, codec detection, and HLS generation functionality.

pub mod codecs;
pub mod profiles;
pub mod stream_builder;
mod ffmpeg;
mod service;

pub use service::{TranscodeService, JobKey};

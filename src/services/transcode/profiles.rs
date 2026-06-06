//! FFmpeg Transcoding Profiles

use super::codecs::HardwareAccelerationType;
use crate::infrastructure::config;

pub struct TranscodingContext {
    pub hwa_type: HardwareAccelerationType,
    pub video_stream_index: usize,
    pub audio_stream_index: usize,
    pub max_video_bitrate: i64,
    pub audio_bitrate: i64,
    pub audio_channels: i32,
    pub start_time: f64,
    pub start_number: usize,
    pub is_video_transcode: bool,
    pub is_audio_transcode: bool,
}

pub struct TranscodeProfile;

impl TranscodeProfile {
    pub fn build_hls_command(
        file_path: &str,
        context: &TranscodingContext,
        segment_pattern: &str,
        playlist_out: &str,
        init_file: &str,
    ) -> Vec<String> {
        let cfg = config();
        let segment_time = cfg.hls_segment_time.to_string();
        let keyint = (cfg.hls_segment_time * 24).to_string();

        let mut args = vec![
            "-hide_banner".into(),
            "-loglevel".into(), "warning".into(),
            "-analyzeduration".into(), "200M".into(),
            "-probesize".into(), "200M".into(),
        ];

        if context.is_video_transcode {
            match context.hwa_type {
                HardwareAccelerationType::Nvenc => {
                    args.extend(vec![
                        "-hwaccel".into(), "cuda".into(),
                        "-hwaccel_output_format".into(), "cuda".into(),
                    ]);
                }
                HardwareAccelerationType::Vaapi => {
                    args.extend(vec![
                        "-hwaccel".into(), "vaapi".into(),
                        "-hwaccel_device".into(), "/dev/dri/renderD128".into(),
                        "-hwaccel_output_format".into(), "vaapi".into(),
                    ]);
                }
                HardwareAccelerationType::Qsv => {
                    args.extend(vec![
                        "-hwaccel".into(), "qsv".into(),
                        "-hwaccel_output_format".into(), "qsv".into(),
                    ]);
                }
                _ => {}
            }
        }

        if context.start_time > 0.0 {
            args.push("-ss".into());
            args.push(format!("{:.3}", context.start_time));
            if !context.is_video_transcode && !context.is_audio_transcode {
                args.push("-noaccurate_seek".into());
            }
        }

        args.push("-i".into());
        args.push(file_path.into());

        args.extend(vec![
            "-map".into(), format!("0:{}", context.video_stream_index),
            "-map".into(), format!("0:{}", context.audio_stream_index),
            "-threads".into(), "0".into(),
        ]);

        // --- Video ---
        if context.is_video_transcode {
            match context.hwa_type {
                HardwareAccelerationType::Nvenc => {
                    args.extend(vec![
                        "-c:v".into(), "h264_nvenc".into(),
                        "-preset".into(), "p4".into(),
                        "-profile:v".into(), "high".into(),
                        "-pix_fmt".into(), "yuv420p".into(),
                        "-b:v".into(), context.max_video_bitrate.to_string(),
                        "-maxrate".into(), context.max_video_bitrate.to_string(),
                        "-bufsize".into(), (context.max_video_bitrate * 2).to_string(),
                        "-g".into(), keyint.clone(),
                        "-keyint_min".into(), keyint.clone(),
                        "-sc_threshold".into(), "0".into(),
                    ]);
                }
                HardwareAccelerationType::Vaapi => {
                    args.extend(vec![
                        "-vf".into(), "format=nv12|vaapi,hwupload".into(),
                        "-c:v".into(), "h264_vaapi".into(),
                        "-profile:v".into(), "high".into(),
                        "-b:v".into(), context.max_video_bitrate.to_string(),
                        "-maxrate".into(), context.max_video_bitrate.to_string(),
                        "-bufsize".into(), (context.max_video_bitrate * 2).to_string(),
                        "-g".into(), keyint.clone(),
                        "-keyint_min".into(), keyint.clone(),
                        "-sc_threshold".into(), "0".into(),
                    ]);
                }
                HardwareAccelerationType::Qsv => {
                    args.extend(vec![
                        "-c:v".into(), "h264_qsv".into(),
                        "-preset".into(), "veryfast".into(),
                        "-profile:v".into(), "high".into(),
                        "-b:v".into(), context.max_video_bitrate.to_string(),
                        "-maxrate".into(), context.max_video_bitrate.to_string(),
                        "-bufsize".into(), (context.max_video_bitrate * 2).to_string(),
                        "-g".into(), keyint.clone(),
                        "-keyint_min".into(), keyint.clone(),
                        "-sc_threshold".into(), "0".into(),
                    ]);
                }
                _ => {
                    args.extend(vec![
                        "-c:v".into(), "libx264".into(),
                        "-preset".into(), "veryfast".into(),
                        "-profile:v".into(), "high".into(),
                        "-pix_fmt".into(), "yuv420p".into(),
                        "-b:v".into(), context.max_video_bitrate.to_string(),
                        "-maxrate".into(), context.max_video_bitrate.to_string(),
                        "-bufsize".into(), (context.max_video_bitrate * 2).to_string(),
                        "-g".into(), keyint.clone(),
                        "-keyint_min".into(), keyint.clone(),
                        "-sc_threshold".into(), "0".into(),
                    ]);
                }
            }
        } else {
            args.extend(vec!["-c:v".into(), "copy".into()]);
        }

        // --- Audio ---
        if context.is_audio_transcode {
            args.extend(vec![
                "-c:a".into(), "aac".into(),
                "-ac".into(), context.audio_channels.to_string(),
                "-b:a".into(), context.audio_bitrate.to_string(),
            ]);
        } else {
            args.extend(vec!["-c:a".into(), "copy".into()]);
        }

        // --- HLS ---
        args.extend(vec![
            "-f".into(), "hls".into(),
            "-hls_time".into(), segment_time,
            "-hls_segment_type".into(), "fmp4".into(),
            "-hls_fmp4_init_filename".into(), init_file.into(),
            "-start_number".into(), context.start_number.to_string(),
            "-hls_segment_filename".into(), segment_pattern.into(),
            "-hls_flags".into(), "independent_segments+temp_file".into(),
            "-movflags".into(), "+frag_discont+default_base_moof".into(),
            "-y".into(),
            playlist_out.into(),
        ]);

        args
    }
}

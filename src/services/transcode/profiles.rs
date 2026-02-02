//! FFmpeg Transcoding Profiles
//!
//! Strict logic port from Jellyfin's `DynamicHlsController.cs` and `EncodingHelper.cs`.
//! 
//! Template match: 
//! "{0} {1} -map_metadata -1 -map_chapters -1 -threads {2} {3} {4} {5} -copyts -avoid_negative_ts disabled -max_muxing_queue_size {6} -f hls -max_delay 5000000 -hls_time {7} -hls_segment_type {8} -start_number {9}{10} -hls_segment_filename \"{11}\" {12} -y \"{13}\""

use std::path::Path;

use super::codecs::HardwareAccelerationType;

pub struct TranscodeProfile;

impl TranscodeProfile {
    /// Build the full FFmpeg command string vectors matching Jellyfin's exact template.
    #[allow(clippy::too_many_arguments)]
    pub fn build_hls_command(
        file_path: &str,
        start_time: f64,
        is_video_transcode: bool,
        video_codec: &str, // Needed for BSF selection
        is_audio_transcode: bool,
        start_number: usize,
        segment_pattern: &str,
        playlist_out: &str,
        init_file_out: &str,
        hwa_type: HardwareAccelerationType,
    ) -> Vec<String> {
        let mut args = Vec::new();
        
        let is_remux = !is_video_transcode;

        // {0} Input Modifiers
        args.extend(Self::get_input_modifiers(start_time, is_remux));

        // HWA: Hardware Acceleration Args (before input)
        // Jellyfin: -hwaccel vaapi -hwaccel_output_format vaapi -vaapi_device /dev/dri/renderD128
        args.extend(Self::get_hwaccel_args(hwa_type));

        // {1} Input
        args.extend(vec!["-i".to_string(), file_path.to_string()]);

        // Global Flags (Hardcoded from template)
        args.extend(vec![
            "-map_metadata".to_string(), "-1".to_string(),
            "-map_chapters".to_string(), "-1".to_string(),
        ]);

        // {2} Threads
        args.extend(vec!["-threads".to_string(), "0".to_string()]);

        // {3} Maps (Implicit/Default for now)
        
        // {4} Video Arguments
        args.extend(Self::get_video_args(is_video_transcode, video_codec, hwa_type));

        // {5} Audio Arguments
        args.extend(Self::get_audio_args(is_audio_transcode));

        // Global Timing/Queue Flags (Hardcoded from template)
        args.extend(vec![
            "-copyts".to_string(),
            "-avoid_negative_ts".to_string(), "disabled".to_string(),
        ]);

        // {6} Max Muxing Queue Size
        args.extend(vec!["-max_muxing_queue_size".to_string(), "2048".to_string()]);

        // HLS Start (Hardcoded from template)
        args.extend(vec![
            "-f".to_string(), "hls".to_string(),
            "-max_delay".to_string(), "5000000".to_string(),
        ]);

        // {7} HLS Time
        args.extend(vec!["-hls_time".to_string(), "3".to_string()]);

        // {8} HLS Segment Type (fmp4 + init filename)
        args.extend(vec![
            "-hls_segment_type".to_string(), "fmp4".to_string(),
            "-hls_fmp4_init_filename".to_string(), init_file_out.to_string(),
        ]);

        // {9} Start Number
        args.extend(vec!["-start_number".to_string(), start_number.to_string()]);

        // {11} Segment Filename
        args.extend(vec!["-hls_segment_filename".to_string(), segment_pattern.to_string()]);

        // {12} HLS Extra Flags (Critical for fMP4 stability)
        args.extend(vec![
            "-hls_segment_options".to_string(), 
            "movflags=+frag_discont".to_string()
        ]);
        
        // Additional Jellyfin flags derived from DynamicHlsController logic
        args.extend(vec![
            "-hls_playlist_type".to_string(), "vod".to_string(),
            "-hls_flags".to_string(), "independent_segments".to_string(),
        ]);

        // {13} Playlist Output
        args.extend(vec!["-y".to_string(), playlist_out.to_string()]);

        args
    }

    /// {0} Input Modifiers (EncodingHelper.cs: GetInputModifier)
    fn get_input_modifiers(start_time: f64, is_remux: bool) -> Vec<String> {
        let mut args = Vec::new();

        // Analyzeduration/Probesize (Jellyfin Defaults)
        args.extend(vec![
            "-analyzeduration".to_string(), "200000000".to_string(),
            "-probesize".to_string(), "1000000000".to_string(),
        ]);

        // Input Modifiers
        if is_remux {
            args.extend(vec![
                "-fflags".to_string(), "+genpts+igndts".to_string(),
                "-noaccurate_seek".to_string(),
            ]);
        } else {
             args.extend(vec![
                "-fflags".to_string(), "+genpts".to_string(),
            ]);
        }

        // Fast Seek (-ss before -i)
        if start_time > 0.0 {
            args.extend(vec!["-ss".to_string(), format!("{:.3}", start_time)]);
        }

        args
    }

    /// Generate -hwaccel flags before input
    fn get_hwaccel_args(hwa: HardwareAccelerationType) -> Vec<String> {
        match hwa {
            HardwareAccelerationType::Vaapi => vec![
                "-hwaccel".to_string(), "vaapi".to_string(),
                "-hwaccel_output_format".to_string(), "vaapi".to_string(),
                "-vaapi_device".to_string(), "/dev/dri/renderD128".to_string(), // TODO: Configurable
            ],
            HardwareAccelerationType::Nvenc => vec![
                "-hwaccel".to_string(), "cuda".to_string(),
                "-hwaccel_output_format".to_string(), "cuda".to_string(),
            ],
            HardwareAccelerationType::Qsv => vec![
                "-hwaccel".to_string(), "qsv".to_string(),
                "-hwaccel_output_format".to_string(), "qsv".to_string(),
            ],
            _ => vec![],
        }
    }

    /// {4} Video Arguments (EncodingHelper.cs: GetVideoArguments)
    fn get_video_args(transcode: bool, codec: &str, hwa: HardwareAccelerationType) -> Vec<String> {
        if !transcode {
             let mut args = vec!["-c:v".to_string(), "copy".to_string()];
             // Critical: Add Bitstream Filter (BSF) when remuxing h264/hevc to fMP4/hls
             match codec.to_lowercase().as_str() {
                 "h264" | "avc1" => {
                     args.extend(vec!["-bsf:v".to_string(), "h264_mp4toannexb".to_string()]);
                 },
                 "hevc" | "h265" | "hev1" => {
                     args.extend(vec!["-bsf:v".to_string(), "hevc_mp4toannexb".to_string()]);
                 },
                 _ => {}
             }
             return args;
        }

        let mut args = Vec::new();

        match hwa {
            HardwareAccelerationType::Vaapi => {
                // Jellyfin VAAPI args (linux)
                args.extend(vec![
                    "-c:v".to_string(), "h264_vaapi".to_string(),
                    "-rc_mode".to_string(), "VBR".to_string(),
                    "-b:v".to_string(), "10M".to_string(),
                    "-maxrate".to_string(), "10M".to_string(),
                    "-bufsize".to_string(), "20M".to_string(),
                    // VAAPI scaling
                    "-vf".to_string(), "format=nv12|vaapi,hwupload,scale_vaapi=w=1920:h=1080:format=nv12".to_string(),
                ]);
            },
            HardwareAccelerationType::Nvenc => {
                // Jellyfin NVENC args
                args.extend(vec![
                    "-c:v".to_string(), "h264_nvenc".to_string(),
                    "-preset".to_string(), "p4".to_string(), // balanced
                    "-b:v".to_string(), "10M".to_string(),
                    "-maxrate".to_string(), "10M".to_string(),
                    "-bufsize".to_string(), "20M".to_string(),
                    // NVENC typically works fine with sw scaling or cuda scaling, sticking to default for safety or sw scale
                    "-vf".to_string(), "scale='min(1920,iw)':-2".to_string(), 
                ]);
            },
            HardwareAccelerationType::Qsv => {
                // Jellyfin QSV args
                args.extend(vec![
                    "-c:v".to_string(), "h264_qsv".to_string(),
                    "-low_power".to_string(), "1".to_string(), // Intel Low Power
                    "-preset".to_string(), "veryfast".to_string(),
                    "-b:v".to_string(), "10M".to_string(),
                    "-maxrate".to_string(), "10M".to_string(),
                    "-vf".to_string(), "vpp_qsv=w=1920:h=1080".to_string(),
                ]);
            },
            _ => {
                // Software Fallback (libx264) - Jellyfin "VeryFast" Logic
                args.extend(vec![
                    "-c:v".to_string(), "libx264".to_string(),
                    "-preset".to_string(), "veryfast".to_string(),
                    "-profile:v".to_string(), "high".to_string(),
                    "-level".to_string(), "4.1".to_string(),
                    "-crf".to_string(), "23".to_string(),
                    "-pix_fmt".to_string(), "yuv420p".to_string(),
                    "-maxrate".to_string(), "10M".to_string(),
                    "-bufsize".to_string(), "20M".to_string(),
                    "-vf".to_string(), "scale='min(1920,iw)':-2".to_string(),
                ]);
            }
        }

        // Keyframe forcing (Critical for HLS)
        // Note: VAAPI/NVENC/QSV usually handle -force_key_frames, but some old drivers struggle.
        // Jellyfin adds it for all usually.
        args.extend(vec![
            "-force_key_frames:0".to_string(), "expr:gte(t,n_forced*3)".to_string(),
        ]);
        
        if hwa == HardwareAccelerationType::None {
             args.extend(vec!["-sc_threshold:v:0".to_string(), "0".to_string()]);
        }

        args
    }

    /// {5} Audio Arguments (EncodingHelper.cs: GetAudioArguments)
    fn get_audio_args(transcode: bool) -> Vec<String> {
        if transcode {
             vec![
                "-c:a".to_string(), "aac".to_string(),
                "-ac".to_string(), "2".to_string(),
                "-ab".to_string(), "192k".to_string(),
            ]
        } else {
            vec!["-c:a".to_string(), "copy".to_string()]
        }
    }
}

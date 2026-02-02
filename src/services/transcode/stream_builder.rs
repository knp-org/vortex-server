use crate::models::db::media::{MediaInfo};
use super::codecs::{DeviceProfile, PlayMethod, TranscodeReason, DirectPlayProfile};

/// Logic for determining how to play a media item
/// Ported from Jellyfin's StreamBuilder.cs
pub struct StreamBuilder;

impl StreamBuilder {
    /// Determines the best play method (DirectPlay, DirectStream, Transcode)
    pub fn determine_play_method(
        media_info: &MediaInfo,
        profile: &DeviceProfile,
        max_bitrate: Option<i32>,
        subtitle_stream_index: Option<usize>
    ) -> (PlayMethod, TranscodeReason) {
        let mut _reasons = TranscodeReason::Unknown; // Placeholder, should be None/clean
        
        // 1. Check Subtitles (Highest Priority check for burn-in)
        if subtitle_stream_index.is_some() {
            // TODO: Smart check for text-based subs (SRT/VTT) that can be served externally
            // For now, assume we need to burn in any selected subtitle
            return (PlayMethod::Transcode, TranscodeReason::SubtitleCodecNotSupported);
        }
        
        // 2. Check Bitrate Limit
        if let Some(limit) = max_bitrate {
            if let Some(bitrate) = media_info.bit_rate {
                 // Convert string to i32 if needed, assumes bitrate is i64 or string in DB?
                 // The MediaInfo struct in codecs.rs has `bit_rate: Option<i64>`.
                 if (bitrate as i32) > limit {
                     // In Jellyfin valid for DirectStream (Remux) if video bitrate is OK?
                     // Usually global bitrate limit kills DirectPlay.
                     return (PlayMethod::Transcode, TranscodeReason::ContainerBitrateExceedsLimit);
                 }
            }
        }

        // 3. Check Direct Play (Container + Codecs)
        if let Some(_dp_profile) = Self::get_direct_play_profile(media_info, profile) {
            // Found a matching profile!
            return (PlayMethod::DirectPlay, TranscodeReason::Unknown); // Unknown == None/OK here
        }

        // 4. Check Direct Stream (Remux)
        // Can we copy video and audio into a supported transcoding container (like TS or MP4)?
        // Jellyfin checks "SupportsDirectStream".
        // For now, we assume if Video is compatible, we can Remux.
        
        let video_compatible = Self::is_video_compatible(media_info, profile);
        let audio_compatible = Self::is_audio_compatible(media_info, profile);

        if video_compatible && audio_compatible {
             // If both are compatible but container wasn't (step 2), we can DirectStream (Remux)
             return (PlayMethod::DirectStream, TranscodeReason::ContainerNotSupported);
        }

        // 5. Transcode
        // Calculate specific reasons
        if !video_compatible {
            return (PlayMethod::Transcode, TranscodeReason::VideoCodecNotSupported);
        }
        if !audio_compatible {
            // If video is fine but audio isn't, we might be able to DirectStream video + Transcode Audio
            // But Vortex 'Transcode' usually involves full FFmpeg pipeline.
            // Jellyfin calls this "Transcode" but with "copy" video codec.
            // We'll return Transcode with AudioCodecNotSupported reason.
            return (PlayMethod::Transcode, TranscodeReason::AudioCodecNotSupported);
        }

        (PlayMethod::Transcode, TranscodeReason::Unknown)
    }

    fn get_direct_play_profile<'a>(
        media: &MediaInfo,
        profile: &'a DeviceProfile
    ) -> Option<&'a DirectPlayProfile> {
        let container = media.container.as_deref().unwrap_or("");
        let video_codec = media.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
        let audio_codec = media.audio.first().map(|a| a.codec.as_str()).unwrap_or("");

        for dp in &profile.direct_play_profiles {
            if dp.supports_container(container)
                && dp.supports_video_codec(video_codec)
                && dp.supports_audio_codec(audio_codec) {
                return Some(dp);
            }
        }
        None
    }

    pub fn is_video_compatible(media: &MediaInfo, profile: &DeviceProfile) -> bool {
        let codec = media.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
        // Simple check against legacy list or we could scan TranscodingProfiles
        profile.video_codecs.iter().any(|c| c.eq_ignore_ascii_case(codec))
    }

    pub fn is_audio_compatible(media: &MediaInfo, profile: &DeviceProfile) -> bool {
        let codec = media.audio.first().map(|a| a.codec.as_str()).unwrap_or("");
        profile.audio_codecs.iter().any(|c| c.eq_ignore_ascii_case(codec))
    }
}

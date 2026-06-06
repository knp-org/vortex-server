use crate::models::db::media::{MediaInfo};
use super::codecs::{DeviceProfile, PlayMethod, TranscodeReason, DirectPlayProfile, normalize_codec};

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
        // Entry Log
        tracing::info!("StreamBuilder: Analyzing media playback decision for profile '{:?}'", profile.name);

        // 1. Check Subtitles (Highest Priority check for burn-in)
        if subtitle_stream_index.is_some() {
            // TODO: Smart check for text-based subs (SRT/VTT) that can be served externally
            // For now, assume we need to burn in any selected subtitle
            tracing::info!("StreamBuilder: Subtitle selected. Forcing burn-in (Transcode).");
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
            tracing::info!("StreamBuilder: Video incompatible. Transcoding.");
            return (PlayMethod::Transcode, TranscodeReason::VideoCodecNotSupported);
        }
        if !audio_compatible {
            tracing::info!("StreamBuilder: Audio incompatible. Transcoding.");
            return (PlayMethod::Transcode, TranscodeReason::AudioCodecNotSupported);
        }

        tracing::info!("StreamBuilder: Fallback to Transcode (Unknown reason).");
        (PlayMethod::Transcode, TranscodeReason::Unknown)
    }

    fn get_direct_play_profile<'a>(
        media: &MediaInfo,
        profile: &'a DeviceProfile
    ) -> Option<&'a DirectPlayProfile> {
        let container = media.container.as_deref().unwrap_or("");
        // Codecs.rs `supports_container` now handles splitting the container string
        // but we need to normalize codecs passed to `supports_video/audio_codec`? 
        // `DirectPlayProfile` methods now handle normalization internally too (Codecs Refactor).
        // So we just pass the raw strings.
        
        let video_codec = media.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
        let audio_codec = media.audio.first().map(|a| a.codec.as_str()).unwrap_or("");

        for dp in &profile.direct_play_profiles {
            if dp.supports_container(container)
                && dp.supports_video_codec(video_codec)
                && dp.supports_audio_codec(audio_codec) {
                tracing::info!("Found matching DirectPlayProfile: Container={:?}, Video={:?}, Audio={:?}", 
                    dp.container, dp.video_codec, dp.audio_codec);
                return Some(dp);
            }
        }
        None
    }

    pub fn is_video_compatible(media: &MediaInfo, profile: &DeviceProfile) -> bool {
        let codec = media.video.as_ref().map(|v| v.codec.as_str()).unwrap_or("");
        let normalized = normalize_codec(codec);
        
        // Check if this codec is supported in any Transcoding Profile (meaning we can remux it)
        for tp in &profile.transcoding_profiles {
            if normalize_codec(&tp.video_codec) == normalized {
                 tracing::info!("Video codec '{}' compatible via TranscodingProfile (Container: {}, Video: {})", 
                     codec, tp.container, tp.video_codec);
                 return true;
            }
        }
        false
    }

    pub fn is_audio_compatible(media: &MediaInfo, profile: &DeviceProfile) -> bool {
        let codec = media.audio.first().map(|a| a.codec.as_str()).unwrap_or("");
        let normalized = normalize_codec(codec);
        
        for tp in &profile.transcoding_profiles {
            if normalize_codec(&tp.audio_codec) == normalized {
                 tracing::info!("Audio codec '{}' compatible via TranscodingProfile (Container: {}, Audio: {})", 
                     codec, tp.container, tp.audio_codec);
                 return true;
            }
        }
        false
    }
}

//! Conversion settings sent from the frontend, and the FFmpeg values they map to.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConversionConfig {
    pub input_path: String,
    pub output_path: Option<String>,
    pub output_dir: Option<String>,
    pub container: Container,
    pub video_codec: Option<VideoCodec>,
    pub audio_codec: Option<AudioCodec>,
    pub resolution: Resolution,
    pub frame_rate: FrameRate,
    pub preset: Preset,
    pub crf: Option<u8>,
    pub video_bitrate: Option<String>,
    pub audio_bitrate: Option<String>,
    pub channels: Channels,
    pub sample_rate: SampleRate,
    pub hardware_acceleration: HardwareAcceleration,
    pub overwrite: bool,
    pub extra_arguments: Option<String>,
}

impl ConversionConfig {
    /// Settings used by the older `run_conversion` command: container defaults,
    /// source resolution / frame rate and CRF 23.
    pub fn for_legacy(input_path: String, output_dir: String, container: Container) -> Self {
        Self {
            input_path,
            output_path: None,
            output_dir: Some(output_dir),
            container,
            video_codec: container.default_video_codec(),
            audio_codec: container.default_audio_codec(),
            resolution: Resolution::Source,
            frame_rate: FrameRate::Source,
            preset: Preset::Medium,
            crf: Some(23),
            video_bitrate: None,
            audio_bitrate: None,
            channels: Channels::Source,
            sample_rate: SampleRate::Source,
            hardware_acceleration: HardwareAcceleration::Off,
            overwrite: false,
            extra_arguments: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Container {
    Mp4,
    Mov,
    Matroska,
    Webm,
    Avi,
    Mp3,
    Wav,
    Flac,
    Aac,
    M4a,
    Ogg,
    Opus,
    Png,
    Webp,
    Gif,
}

impl Container {
    pub const ALL: [Container; 15] = [
        Self::Mp4,
        Self::Mov,
        Self::Matroska,
        Self::Webm,
        Self::Avi,
        Self::Mp3,
        Self::Wav,
        Self::Flac,
        Self::Aac,
        Self::M4a,
        Self::Ogg,
        Self::Opus,
        Self::Png,
        Self::Webp,
        Self::Gif,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Matroska => "mkv",
            Self::Webm => "webm",
            Self::Avi => "avi",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::M4a => "m4a",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
        }
    }

    pub fn is_audio_only(self) -> bool {
        matches!(
            self,
            Self::Mp3 | Self::Wav | Self::Flac | Self::Aac | Self::M4a | Self::Ogg | Self::Opus
        )
    }

    pub fn is_image(self) -> bool {
        matches!(self, Self::Png | Self::Webp | Self::Gif)
    }

    /// The video encoder to use when the user has not picked one.
    ///
    /// Image formats return `None` so FFmpeg picks the only encoder their
    /// muxer accepts (png, libwebp, gif).
    pub fn default_video_codec(self) -> Option<VideoCodec> {
        match self {
            Self::Mp4 | Self::Mov | Self::Matroska | Self::Avi => Some(VideoCodec::Libx264),
            Self::Webm => Some(VideoCodec::LibvpxVp9),
            _ => None,
        }
    }

    /// Whether this container can hold `codec`. Only formats with a short,
    /// well-known list are restricted; the general purpose containers accept
    /// everything the UI offers.
    pub fn accepts_video_codec(self, codec: VideoCodec) -> bool {
        match self {
            Self::Webm => matches!(
                codec,
                VideoCodec::LibvpxVp9 | VideoCodec::Av1 | VideoCodec::Copy
            ),
            _ => true,
        }
    }

    /// See [`Container::accepts_video_codec`].
    pub fn accepts_audio_codec(self, codec: AudioCodec) -> bool {
        use AudioCodec::*;
        match self {
            Self::Webm | Self::Opus => matches!(codec, Opus | Copy),
            Self::Ogg => matches!(codec, Opus | Flac | Copy),
            Self::Mp3 => matches!(codec, Mp3 | Copy),
            Self::Wav => matches!(codec, Pcm | Copy),
            Self::Flac => matches!(codec, Flac | Copy),
            Self::Aac | Self::M4a => matches!(codec, Aac | Copy),
            Self::Mp4 => !matches!(codec, Pcm),
            _ => true,
        }
    }

    pub fn default_audio_codec(self) -> Option<AudioCodec> {
        match self {
            Self::Mp4 | Self::Mov | Self::Matroska | Self::Avi => Some(AudioCodec::Aac),
            Self::Webm => Some(AudioCodec::Opus),
            Self::Mp3 => Some(AudioCodec::Mp3),
            Self::Wav => Some(AudioCodec::Pcm),
            Self::Flac => Some(AudioCodec::Flac),
            Self::Aac | Self::M4a => Some(AudioCodec::Aac),
            Self::Ogg | Self::Opus => Some(AudioCodec::Opus),
            Self::Png | Self::Webp | Self::Gif => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VideoCodec {
    Libx264,
    Libx265,
    LibvpxVp9,
    Av1,
    Copy,
}

impl VideoCodec {
    pub fn as_ffmpeg_arg(self) -> &'static str {
        match self {
            Self::Libx264 => "libx264",
            Self::Libx265 => "libx265",
            Self::LibvpxVp9 => "libvpx-vp9",
            Self::Av1 => "libaom-av1",
            Self::Copy => "copy",
        }
    }

    pub fn friendly_name(self) -> &'static str {
        match self {
            Self::Libx264 => "H.264",
            Self::Libx265 => "H.265 / HEVC",
            Self::LibvpxVp9 => "VP9",
            Self::Av1 => "AV1",
            Self::Copy => "Copy",
        }
    }

    /// Whether `-preset` means anything to this encoder. libvpx and libaom
    /// ignore it (and FFmpeg warns), and stream copy has no encoder at all.
    pub fn supports_preset(self) -> bool {
        matches!(self, Self::Libx264 | Self::Libx265)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AudioCodec {
    Aac,
    Mp3,
    Opus,
    Flac,
    Pcm,
    Copy,
}

impl AudioCodec {
    pub fn as_ffmpeg_arg(self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Mp3 => "libmp3lame",
            Self::Opus => "libopus",
            Self::Flac => "flac",
            Self::Pcm => "pcm_s16le",
            Self::Copy => "copy",
        }
    }

    /// Lossless and copied audio has no target bitrate.
    pub fn supports_bitrate(self) -> bool {
        matches!(self, Self::Aac | Self::Mp3 | Self::Opus)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Resolution {
    Source,
    P720,
    P1080,
    P1440,
    P2160,
    Custom { width: u32, height: u32 },
}

impl Resolution {
    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match self {
            Self::Source => None,
            Self::P720 => Some((1280, 720)),
            Self::P1080 => Some((1920, 1080)),
            Self::P1440 => Some((2560, 1440)),
            Self::P2160 => Some((3840, 2160)),
            Self::Custom { width, height } => Some((*width, *height)),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FrameRate {
    Source,
    Fps24,
    Fps30,
    Fps60,
    Fps120,
}

impl FrameRate {
    pub fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Fps24 => Some("24"),
            Self::Fps30 => Some("30"),
            Self::Fps60 => Some("60"),
            Self::Fps120 => Some("120"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    Ultrafast,
    Superfast,
    Veryfast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    Veryslow,
}

impl Preset {
    pub fn as_ffmpeg_arg(self) -> &'static str {
        match self {
            Self::Ultrafast => "ultrafast",
            Self::Superfast => "superfast",
            Self::Veryfast => "veryfast",
            Self::Faster => "faster",
            Self::Fast => "fast",
            Self::Medium => "medium",
            Self::Slow => "slow",
            Self::Slower => "slower",
            Self::Veryslow => "veryslow",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Channels {
    Source,
    Mono,
    Stereo,
    Surround51,
}

impl Channels {
    pub fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Mono => Some("1"),
            Self::Stereo => Some("2"),
            Self::Surround51 => Some("6"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SampleRate {
    Source,
    Hz22050,
    Hz44100,
    Hz48000,
    Hz96000,
}

impl SampleRate {
    pub fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Hz22050 => Some("22050"),
            Self::Hz44100 => Some("44100"),
            Self::Hz48000 => Some("48000"),
            Self::Hz96000 => Some("96000"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HardwareAcceleration {
    Off,
    Auto,
}

/// Trimmed value of an optional string, or `None` when it is missing or blank.
pub fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

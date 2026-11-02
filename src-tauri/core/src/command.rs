//! Turning a [`ConversionConfig`] into an FFmpeg argument list.

use std::path::Path;

use crate::config::*;

/// Small fluent builder so each FFmpeg flag lives in one place.
#[derive(Debug, Default)]
pub struct FfmpegCommandBuilder {
    args: Vec<String>,
}

impl FfmpegCommandBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, flag: &str, value: impl Into<String>) {
        self.args.push(flag.to_string());
        self.args.push(value.into());
    }

    pub fn overwrite(mut self, overwrite: bool) -> Self {
        self.args
            .push(if overwrite { "-y" } else { "-n" }.to_string());
        self
    }

    /// Emit structured `key=value` progress on stdout and silence the
    /// interactive stats line on stderr.
    pub fn progress_pipe(mut self) -> Self {
        self.push("-progress", "pipe:1");
        self.args.push("-nostats".to_string());
        self
    }

    pub fn hardware_acceleration(mut self, value: HardwareAcceleration) -> Self {
        if value == HardwareAcceleration::Auto {
            self.push("-hwaccel", "auto");
        }
        self
    }

    pub fn input(mut self, input: &Path) -> Self {
        self.push("-i", input.to_string_lossy());
        self
    }

    pub fn no_video(mut self, enabled: bool) -> Self {
        if enabled {
            self.args.push("-vn".to_string());
        }
        self
    }

    pub fn no_audio(mut self, enabled: bool) -> Self {
        if enabled {
            self.args.push("-an".to_string());
        }
        self
    }

    pub fn video_codec(mut self, codec: Option<VideoCodec>) -> Self {
        if let Some(codec) = codec {
            self.push("-c:v", codec.as_ffmpeg_arg());
        }
        self
    }

    pub fn audio_codec(mut self, codec: Option<AudioCodec>) -> Self {
        if let Some(codec) = codec {
            self.push("-c:a", codec.as_ffmpeg_arg());
        }
        self
    }

    pub fn scale(mut self, resolution: &Resolution) -> Self {
        if let Some((width, height)) = resolution.dimensions() {
            self.push("-vf", format!("scale={}:{}", width, height));
        }
        self
    }

    pub fn frame_rate(mut self, frame_rate: FrameRate) -> Self {
        if let Some(value) = frame_rate.as_ffmpeg_arg() {
            self.push("-r", value);
        }
        self
    }

    /// Write exactly one video frame (needed for single-image outputs such as PNG,
    /// which FFmpeg otherwise refuses when the input has more than one frame).
    pub fn single_frame(mut self, enabled: bool) -> Self {
        if enabled {
            self.push("-frames:v", "1");
        }
        self
    }

    pub fn preset(mut self, preset: Preset, enabled: bool) -> Self {
        if enabled {
            self.push("-preset", preset.as_ffmpeg_arg());
        }
        self
    }

    pub fn crf(mut self, crf: Option<u8>, enabled: bool) -> Self {
        if let (true, Some(crf)) = (enabled, crf) {
            self.push("-crf", crf.to_string());
        }
        self
    }

    pub fn video_bitrate(mut self, bitrate: &Option<String>, enabled: bool) -> Self {
        if let (true, Some(bitrate)) = (enabled, non_empty(bitrate)) {
            self.push("-b:v", bitrate);
        }
        self
    }

    pub fn audio_bitrate(mut self, bitrate: &Option<String>, enabled: bool) -> Self {
        if let (true, Some(bitrate)) = (enabled, non_empty(bitrate)) {
            self.push("-b:a", bitrate);
        }
        self
    }

    pub fn channels(mut self, channels: Channels, enabled: bool) -> Self {
        if let (true, Some(value)) = (enabled, channels.as_ffmpeg_arg()) {
            self.push("-ac", value);
        }
        self
    }

    pub fn sample_rate(mut self, sample_rate: SampleRate, enabled: bool) -> Self {
        if let (true, Some(value)) = (enabled, sample_rate.as_ffmpeg_arg()) {
            self.push("-ar", value);
        }
        self
    }

    pub fn extra_arguments(mut self, value: &Option<String>) -> Self {
        if let Some(value) = non_empty(value) {
            self.args
                .extend(value.split_whitespace().map(str::to_string));
        }
        self
    }

    pub fn output(mut self, output: &Path) -> Self {
        self.args.push(output.to_string_lossy().into_owned());
        self
    }

    pub fn build(self) -> Vec<String> {
        self.args
    }
}

/// Everything needed to launch FFmpeg for one conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FfmpegPlan {
    pub args: Vec<String>,
    /// Friendly name of the video encoder, shown in the UI ("H.264", ...).
    pub encoder_label: Option<String>,
}

/// Build the FFmpeg arguments for `config`, reading from `input` and writing to `output`.
pub fn build_ffmpeg_args(config: &ConversionConfig, input: &Path, output: &Path) -> FfmpegPlan {
    let container = config.container;
    let video_enabled = !container.is_audio_only();
    let audio_enabled = !container.is_image();

    let video_codec = if !video_enabled {
        None
    } else if container.is_image() {
        // Image muxers only accept their own encoder (png, libwebp, gif); the
        // video codec dropdown is meant for video containers, so ignore it here.
        container.default_video_codec()
    } else {
        // The codec dropdowns keep their value when the user switches format,
        // so fall back to the container default if the pick cannot be stored.
        config
            .video_codec
            .filter(|c| container.accepts_video_codec(*c))
            .or_else(|| container.default_video_codec())
    };
    let audio_codec = if audio_enabled {
        config
            .audio_codec
            .filter(|c| container.accepts_audio_codec(*c))
            .or_else(|| container.default_audio_codec())
    } else {
        None
    };

    let encoding_video = video_enabled && video_codec != Some(VideoCodec::Copy);
    let encoding_audio = audio_enabled && audio_codec != Some(AudioCodec::Copy);
    let preset_enabled = video_codec.is_some_and(VideoCodec::supports_preset);
    let crf_enabled = video_codec.is_some_and(|c| c != VideoCodec::Copy);
    let audio_bitrate_enabled = audio_codec.is_some_and(AudioCodec::supports_bitrate);

    let args = FfmpegCommandBuilder::new()
        .overwrite(config.overwrite)
        .progress_pipe()
        .hardware_acceleration(config.hardware_acceleration)
        .input(input)
        .no_video(!video_enabled)
        .no_audio(!audio_enabled)
        .video_codec(video_codec)
        .audio_codec(audio_codec)
        .scale(if encoding_video {
            &config.resolution
        } else {
            &Resolution::Source
        })
        .frame_rate(if encoding_video {
            config.frame_rate
        } else {
            FrameRate::Source
        })
        .single_frame(container == Container::Png)
        .preset(config.preset, preset_enabled)
        .crf(config.crf, crf_enabled)
        .video_bitrate(
            &config.video_bitrate,
            encoding_video && !container.is_image(),
        )
        .audio_bitrate(&config.audio_bitrate, audio_bitrate_enabled)
        .channels(config.channels, encoding_audio)
        .sample_rate(config.sample_rate, encoding_audio)
        .extra_arguments(&config.extra_arguments)
        .output(output)
        .build();

    FfmpegPlan {
        args,
        encoder_label: video_codec.map(|c| c.friendly_name().to_string()),
    }
}

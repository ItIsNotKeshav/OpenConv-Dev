mod common;

use common::*;
use openconv_core::*;
use std::path::Path;

#[test]
fn mp4_defaults_produce_expected_command() {
    let mut cfg = config(Container::Mp4);
    cfg.crf = Some(23);
    cfg.audio_bitrate = Some("128k".into());
    let plan = build_ffmpeg_args(&cfg, Path::new("in.mov"), Path::new("out.mp4"));
    assert_eq!(
        plan.args,
        [
            "-n",
            "-progress",
            "pipe:1",
            "-nostats",
            "-i",
            "in.mov",
            "-c:v",
            "libx264",
            "-c:a",
            "aac",
            "-preset",
            "medium",
            "-crf",
            "23",
            "-b:a",
            "128k",
            "out.mp4"
        ]
    );
    assert_eq!(plan.encoder_label.as_deref(), Some("H.264"));
}

#[test]
fn overwrite_flag_switches_between_y_and_n() {
    let mut cfg = config(Container::Mp4);
    assert_eq!(args(&cfg)[0], "-n");
    cfg.overwrite = true;
    assert_eq!(args(&cfg)[0], "-y");
}

#[test]
fn input_and_output_come_in_the_right_place() {
    let cfg = config(Container::Mp4);
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-i"), Some("in.mp4"));
    assert_eq!(a.last().map(String::as_str), Some("out.file"));
}

#[test]
fn hardware_acceleration_goes_before_the_input() {
    let mut cfg = config(Container::Mp4);
    cfg.hardware_acceleration = HardwareAcceleration::Auto;
    let a = args(&cfg);
    let hw = a.iter().position(|x| x == "-hwaccel").expect("-hwaccel");
    let input = a.iter().position(|x| x == "-i").unwrap();
    assert!(hw < input, "-hwaccel must be an input option: {a:?}");
    assert_eq!(a[hw + 1], "auto");
}

#[test]
fn audio_only_containers_drop_video() {
    for container in Container::ALL.into_iter().filter(|c| c.is_audio_only()) {
        let mut cfg = config(container);
        // The UI always sends whatever is selected in the video dropdown.
        cfg.video_codec = Some(VideoCodec::Libx264);
        cfg.resolution = Resolution::P1080;
        cfg.crf = Some(23);
        let a = args(&cfg);
        assert!(has(&a, "-vn"), "{container:?}: {a:?}");
        assert!(!has(&a, "-c:v"), "{container:?}: {a:?}");
        assert!(!has(&a, "-vf"), "{container:?}: {a:?}");
        assert!(!has(&a, "-preset"), "{container:?}: {a:?}");
        assert!(!has(&a, "-crf"), "{container:?}: {a:?}");
        assert_eq!(
            flag_value(&a, "-c:a"),
            container
                .default_audio_codec()
                .map(AudioCodec::as_ffmpeg_arg),
            "{container:?}"
        );
    }
}

#[test]
fn image_containers_ignore_the_video_codec_dropdown_and_drop_audio() {
    for container in [Container::Png, Container::Gif, Container::Webp] {
        let mut cfg = config(container);
        cfg.video_codec = Some(VideoCodec::Libx264);
        cfg.audio_codec = Some(AudioCodec::Aac);
        cfg.crf = Some(23);
        cfg.audio_bitrate = Some("128k".into());
        cfg.channels = Channels::Stereo;
        let a = args(&cfg);
        assert!(has(&a, "-an"), "{container:?}: {a:?}");
        assert!(!has(&a, "-c:v"), "{container:?}: {a:?}");
        assert!(!has(&a, "-c:a"), "{container:?}: {a:?}");
        assert!(!has(&a, "-preset"), "{container:?}: {a:?}");
        assert!(!has(&a, "-crf"), "{container:?}: {a:?}");
        assert!(!has(&a, "-b:a"), "{container:?}: {a:?}");
        assert!(!has(&a, "-ac"), "{container:?}: {a:?}");
    }
}

#[test]
fn png_extracts_a_single_frame() {
    let a = args(&config(Container::Png));
    assert_eq!(flag_value(&a, "-frames:v"), Some("1"));
    assert!(!has(&args(&config(Container::Gif)), "-frames:v"));
}

#[test]
fn image_outputs_can_still_be_scaled() {
    let mut cfg = config(Container::Gif);
    cfg.resolution = Resolution::P720;
    cfg.frame_rate = FrameRate::Fps24;
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-vf"), Some("scale=1280:720"));
    assert_eq!(flag_value(&a, "-r"), Some("24"));
}

#[test]
fn video_copy_skips_encoder_only_options() {
    let mut cfg = config(Container::Matroska);
    cfg.video_codec = Some(VideoCodec::Copy);
    cfg.crf = Some(20);
    cfg.resolution = Resolution::P720;
    cfg.frame_rate = FrameRate::Fps30;
    cfg.video_bitrate = Some("2M".into());
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-c:v"), Some("copy"));
    // Filters and rate changes cannot be combined with stream copy.
    for flag in ["-preset", "-crf", "-vf", "-r", "-b:v"] {
        assert!(!has(&a, flag), "{flag} with copy: {a:?}");
    }
}

#[test]
fn audio_copy_skips_audio_encoder_options() {
    let mut cfg = config(Container::Mp4);
    cfg.audio_codec = Some(AudioCodec::Copy);
    cfg.audio_bitrate = Some("192k".into());
    cfg.channels = Channels::Mono;
    cfg.sample_rate = SampleRate::Hz44100;
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-c:a"), Some("copy"));
    for flag in ["-b:a", "-ac", "-ar"] {
        assert!(!has(&a, flag), "{flag} with copy: {a:?}");
    }
}

#[test]
fn preset_is_only_passed_to_x264_and_x265() {
    for (codec, expected) in [
        (VideoCodec::Libx264, true),
        (VideoCodec::Libx265, true),
        (VideoCodec::LibvpxVp9, false),
        (VideoCodec::Av1, false),
        (VideoCodec::Copy, false),
    ] {
        let mut cfg = config(Container::Matroska);
        cfg.video_codec = Some(codec);
        cfg.preset = Preset::Slow;
        let a = args(&cfg);
        assert_eq!(has(&a, "-preset"), expected, "{codec:?}: {a:?}");
    }
}

#[test]
fn lossless_audio_gets_no_bitrate() {
    for (container, codec) in [
        (Container::Wav, AudioCodec::Pcm),
        (Container::Flac, AudioCodec::Flac),
    ] {
        let mut cfg = config(container);
        cfg.audio_codec = Some(codec);
        cfg.audio_bitrate = Some("128k".into());
        assert!(!has(&args(&cfg), "-b:a"), "{container:?}");
    }
}

#[test]
fn resolution_presets_map_to_scale_filters() {
    for (res, expected) in [
        (Resolution::P720, "scale=1280:720"),
        (Resolution::P1080, "scale=1920:1080"),
        (Resolution::P1440, "scale=2560:1440"),
        (Resolution::P2160, "scale=3840:2160"),
        (
            Resolution::Custom {
                width: 854,
                height: 480,
            },
            "scale=854:480",
        ),
    ] {
        let mut cfg = config(Container::Mp4);
        cfg.resolution = res;
        assert_eq!(flag_value(&args(&cfg), "-vf"), Some(expected));
    }
    assert!(!has(&args(&config(Container::Mp4)), "-vf"));
}

#[test]
fn audio_options_map_to_ffmpeg_values() {
    let mut cfg = config(Container::Mp3);
    cfg.channels = Channels::Surround51;
    cfg.sample_rate = SampleRate::Hz48000;
    cfg.audio_bitrate = Some(" 320k ".into());
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-ac"), Some("6"));
    assert_eq!(flag_value(&a, "-ar"), Some("48000"));
    assert_eq!(flag_value(&a, "-b:a"), Some("320k"));
    assert_eq!(flag_value(&a, "-c:a"), Some("libmp3lame"));
}

#[test]
fn blank_bitrates_and_extra_arguments_are_ignored() {
    let mut cfg = config(Container::Mp4);
    cfg.video_bitrate = Some("   ".into());
    cfg.audio_bitrate = Some(String::new());
    cfg.extra_arguments = Some("  ".into());
    let a = args(&cfg);
    assert!(!has(&a, "-b:v"));
    assert!(!has(&a, "-b:a"));
    assert_eq!(a.last().map(String::as_str), Some("out.file"));
}

#[test]
fn extra_arguments_are_split_and_placed_before_the_output() {
    let mut cfg = config(Container::Mp4);
    cfg.extra_arguments = Some("-movflags  +faststart\n-tune film".into());
    let a = args(&cfg);
    let n = a.len();
    assert_eq!(
        &a[n - 5..],
        ["-movflags", "+faststart", "-tune", "film", "out.file"]
    );
}

#[test]
fn webm_uses_vp9_and_opus_by_default() {
    let a = args(&config(Container::Webm));
    assert_eq!(flag_value(&a, "-c:v"), Some("libvpx-vp9"));
    assert_eq!(flag_value(&a, "-c:a"), Some("libopus"));
}

#[test]
fn every_container_has_consistent_defaults() {
    for c in Container::ALL {
        assert!(
            !(c.is_audio_only() && c.is_image()),
            "{c:?} cannot be both audio-only and an image"
        );
        if c.is_audio_only() {
            assert!(c.default_video_codec().is_none(), "{c:?}");
            assert!(c.default_audio_codec().is_some(), "{c:?}");
        } else if c.is_image() {
            assert!(c.default_video_codec().is_none(), "{c:?}");
            assert!(c.default_audio_codec().is_none(), "{c:?}");
        } else {
            assert!(c.default_video_codec().is_some(), "{c:?}");
            assert!(c.default_audio_codec().is_some(), "{c:?}");
        }
    }
}

#[test]
fn legacy_config_uses_container_defaults() {
    let cfg = ConversionConfig::for_legacy("a.mov".into(), "/tmp".into(), Container::Flac);
    assert_eq!(cfg.video_codec, None);
    assert_eq!(cfg.audio_codec, Some(AudioCodec::Flac));
    assert_eq!(cfg.crf, Some(23));
    assert_eq!(cfg.output_dir.as_deref(), Some("/tmp"));
}

#[test]
fn incompatible_codec_choices_fall_back_to_the_container_default() {
    // WebM cannot hold H.264 or AAC.
    let mut cfg = config(Container::Webm);
    cfg.video_codec = Some(VideoCodec::Libx264);
    cfg.audio_codec = Some(AudioCodec::Aac);
    let a = args(&cfg);
    assert_eq!(flag_value(&a, "-c:v"), Some("libvpx-vp9"));
    assert_eq!(flag_value(&a, "-c:a"), Some("libopus"));

    // An MP3 file can only hold MP3 audio.
    let mut cfg = config(Container::Mp3);
    cfg.audio_codec = Some(AudioCodec::Aac);
    assert_eq!(flag_value(&args(&cfg), "-c:a"), Some("libmp3lame"));

    // Compatible choices are kept.
    let mut cfg = config(Container::Ogg);
    cfg.audio_codec = Some(AudioCodec::Flac);
    assert_eq!(flag_value(&args(&cfg), "-c:a"), Some("flac"));
    let mut cfg = config(Container::Matroska);
    cfg.video_codec = Some(VideoCodec::Libx265);
    assert_eq!(flag_value(&args(&cfg), "-c:v"), Some("libx265"));
}

#[test]
fn every_container_accepts_its_own_defaults() {
    for c in Container::ALL {
        if let Some(v) = c.default_video_codec() {
            assert!(c.accepts_video_codec(v), "{c:?} rejects default {v:?}");
        }
        if let Some(a) = c.default_audio_codec() {
            assert!(c.accepts_audio_codec(a), "{c:?} rejects default {a:?}");
        }
    }
}

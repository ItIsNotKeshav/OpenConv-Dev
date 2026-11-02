//! The JSON contract with the frontend, and error message handling.

use openconv_core::*;
use serde_json::json;

/// What `createDefaultConfig()` in `src/app.js` sends, after a file and format
/// have been chosen.
fn frontend_config() -> serde_json::Value {
    json!({
        "inputPath": "/videos/in.mov",
        "outputPath": null,
        "outputDir": "/videos/out",
        "container": "mp4",
        "videoCodec": "libx264",
        "audioCodec": "aac",
        "resolution": "source",
        "frameRate": "source",
        "preset": "medium",
        "crf": 23,
        "videoBitrate": null,
        "audioBitrate": "128k",
        "channels": "source",
        "sampleRate": "source",
        "hardwareAcceleration": "off",
        "overwrite": false,
        "extraArguments": "",
        // UI-only field; the backend must ignore it.
        "kind": "video"
    })
}

fn parse(value: serde_json::Value) -> Result<ConversionConfig, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

#[test]
fn deserializes_the_frontend_default_config() {
    let cfg = parse(frontend_config()).unwrap();
    assert_eq!(cfg.container, Container::Mp4);
    assert_eq!(cfg.video_codec, Some(VideoCodec::Libx264));
    assert_eq!(cfg.audio_codec, Some(AudioCodec::Aac));
    assert_eq!(cfg.crf, Some(23));
    assert_eq!(cfg.output_dir.as_deref(), Some("/videos/out"));
}

#[test]
fn accepts_every_value_the_frontend_can_send() {
    // Keep in sync with FORMAT_PRESETS and the <select> options in src/.
    let cases: &[(&str, &[&str])] = &[
        (
            "container",
            &[
                "mp4", "mov", "matroska", "webm", "avi", "mp3", "wav", "flac", "aac", "m4a", "ogg",
                "opus", "png", "webp", "gif",
            ],
        ),
        (
            "videoCodec",
            &["libx264", "libx265", "libvpx-vp9", "av1", "copy"],
        ),
        ("audioCodec", &["aac", "mp3", "opus", "flac", "pcm", "copy"]),
        ("resolution", &["source", "p720", "p1080", "p1440", "p2160"]),
        (
            "frameRate",
            &["source", "fps24", "fps30", "fps60", "fps120"],
        ),
        (
            "preset",
            &[
                "ultrafast",
                "superfast",
                "veryfast",
                "faster",
                "fast",
                "medium",
                "slow",
                "slower",
                "veryslow",
            ],
        ),
        ("channels", &["source", "mono", "stereo", "surround51"]),
        (
            "sampleRate",
            &["source", "hz22050", "hz44100", "hz48000", "hz96000"],
        ),
        ("hardwareAcceleration", &["off", "auto"]),
    ];
    for (field, values) in cases {
        for value in *values {
            let mut v = frontend_config();
            v[*field] = json!(value);
            parse(v).unwrap_or_else(|e| panic!("{field}={value}: {e}"));
        }
    }
}

#[test]
fn null_codecs_mean_use_the_container_default() {
    let mut v = frontend_config();
    v["videoCodec"] = json!(null);
    v["audioCodec"] = json!(null);
    let cfg = parse(v).unwrap();
    assert_eq!(cfg.video_codec, None);
    assert_eq!(cfg.audio_codec, None);
}

#[test]
fn custom_resolution_deserializes() {
    let mut v = frontend_config();
    v["resolution"] = json!({ "custom": { "width": 854, "height": 480 } });
    assert_eq!(
        parse(v).unwrap().resolution,
        Resolution::Custom {
            width: 854,
            height: 480
        }
    );
}

#[test]
fn rejects_unknown_values() {
    let mut v = frontend_config();
    v["container"] = json!("wmv");
    assert!(parse(v).is_err());

    let mut v = frontend_config();
    v["crf"] = json!(300);
    assert!(parse(v).is_err(), "crf must fit in a u8");
}

#[test]
fn config_round_trips_through_json() {
    let cfg = parse(frontend_config()).unwrap();
    let back = parse(serde_json::to_value(&cfg).unwrap()).unwrap();
    assert_eq!(cfg, back);
}

#[test]
fn ffmpeg_errors_become_readable_messages() {
    let cases = [
        ("Unknown encoder 'libfoo'", "not supported"),
        ("Encoder not found", "not supported"),
        ("out.mp4: Permission denied", "permission"),
        ("No space left on device", "disk is full"),
        ("File 'out.mp4' already exists. Exiting.", "already exists"),
        (
            "Could not find tag for codec pcm_s16le in stream #0, codec not currently supported in container",
            "cannot be stored",
        ),
        ("in.mp4: Invalid data found when processing input", "could not read the source"),
        ("Error: Invalid argument", "rejected one of the selected settings"),
        ("", "without reporting an error"),
    ];
    for (log, expected) in cases {
        let msg = readable_ffmpeg_error(log);
        assert!(
            msg.to_lowercase().contains(&expected.to_lowercase()),
            "{log:?} -> {msg:?}"
        );
    }
    assert_eq!(
        readable_ffmpeg_error("something odd\n"),
        "FFmpeg failed:\nsomething odd"
    );
}

#[test]
fn process_errors_are_explained() {
    use std::io::{Error, ErrorKind};
    assert!(friendly_process_error(&Error::from(ErrorKind::NotFound)).contains("PATH"));
    assert!(
        friendly_process_error(&Error::from(ErrorKind::PermissionDenied)).contains("permission")
    );
    assert!(friendly_process_error(&Error::other("boom")).contains("boom"));
}

#[test]
fn truncate_log_keeps_the_newest_text() {
    let mut log = "a".repeat(50);
    truncate_log(&mut log, 100, 10);
    assert_eq!(log.len(), 50, "under the limit nothing changes");

    let mut log = format!("{}{}", "a".repeat(100), "b".repeat(10));
    truncate_log(&mut log, 100, 10);
    assert_eq!(log, "b".repeat(10));
}

#[test]
fn truncate_log_never_splits_a_utf8_character() {
    // FFmpeg echoes file names, which can contain any Unicode.
    let mut log = "é".repeat(60); // 120 bytes, 2 bytes per char
    truncate_log(&mut log, 100, 11); // 11 would land mid-character
    assert!(log.len() <= 11 && log.chars().all(|c| c == 'é'), "{log:?}");
}

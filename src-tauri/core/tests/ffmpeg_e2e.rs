//! End-to-end checks that run the real `ffmpeg` binary with the arguments
//! OpenConv builds, for every output format the app offers.
//!
//! These tests are skipped (with a note on stderr) when `ffmpeg` is not on
//! PATH. Set `OPENCONV_REQUIRE_FFMPEG=1` to make a missing FFmpeg a failure,
//! as CI does.

mod common;

use common::*;
use openconv_core::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn ffmpeg_available() -> bool {
    let ok = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        if std::env::var_os("OPENCONV_REQUIRE_FFMPEG").is_some() {
            panic!("ffmpeg is required (OPENCONV_REQUIRE_FFMPEG is set) but was not found");
        }
        eprintln!("skipping: ffmpeg not found on PATH");
    }
    ok
}

/// A 2 second 320x240 test clip with a sine tone.
fn make_sample(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("sample.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=320x240:rate=25:duration=2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&path)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "could not create sample clip");
    path
}

struct RunResult {
    output: PathBuf,
    stdout: String,
    stderr: String,
}

/// Validate, resolve the output path and run FFmpeg exactly as the app does.
fn convert(input: &Path, out_dir: &Path, edit: impl FnOnce(&mut ConversionConfig)) -> RunResult {
    let mut cfg = config(Container::Mp4);
    cfg.input_path = input.to_string_lossy().into_owned();
    cfg.output_dir = Some(out_dir.to_string_lossy().into_owned());
    cfg.preset = Preset::Ultrafast;
    edit(&mut cfg);

    validate_config(&cfg).expect("config should be valid");
    let output = resolve_output_path(input, &cfg).expect("output path");
    let plan = build_ffmpeg_args(&cfg, input, &output);
    let out = Command::new("ffmpeg")
        .args(&plan.args)
        .output()
        .expect("run ffmpeg");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    assert!(
        out.status.success(),
        "{:?} failed\nargs: {:?}\n{}",
        cfg.container,
        plan.args,
        readable_ffmpeg_error(&stderr)
    );
    assert!(
        std::fs::metadata(&output)
            .map(|m| m.len() > 0)
            .unwrap_or(false),
        "{:?}: no output written at {output:?}",
        cfg.container
    );
    RunResult {
        output,
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr,
    }
}

/// `codec_type:codec_name` for each stream in `path`.
fn streams(path: &Path) -> Vec<String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type,codec_name",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .expect("run ffprobe");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| {
            let mut parts = l.split(',');
            let name = parts.next().unwrap_or("");
            let kind = parts.next().unwrap_or("");
            format!("{kind}:{name}")
        })
        .collect()
}

#[test]
fn every_container_converts_with_its_default_codecs() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-all");
    let input = make_sample(&dir);

    for container in Container::ALL {
        let result = convert(&input, dir.path(), |c| {
            c.container = container;
            // The codec dropdowns keep H.264 / AAC when the user switches
            // format; the backend must fall back to codecs the container takes.
            c.video_codec = Some(VideoCodec::Libx264);
            c.audio_codec = Some(AudioCodec::Aac);
            c.crf = Some(30);
            c.audio_bitrate = Some("96k".into());
        });
        assert_eq!(
            result.output.extension().unwrap(),
            container.extension(),
            "{container:?}"
        );

        let found = streams(&result.output);
        let has_video = found.iter().any(|s| s.starts_with("video:"));
        let has_audio = found.iter().any(|s| s.starts_with("audio:"));
        if container.is_audio_only() {
            assert!(has_audio && !has_video, "{container:?}: {found:?}");
        } else if container.is_image() {
            assert!(has_video && !has_audio, "{container:?}: {found:?}");
        } else if container != Container::Webm {
            assert!(has_video && has_audio, "{container:?}: {found:?}");
        }
    }
}

#[test]
fn webm_output_uses_vp9_and_opus() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-webm");
    let input = make_sample(&dir);
    let result = convert(&input, dir.path(), |c| {
        c.container = Container::Webm;
        c.video_codec = Some(VideoCodec::LibvpxVp9);
        c.audio_codec = Some(AudioCodec::Opus);
        c.extra_arguments = Some("-deadline realtime -cpu-used 8".into());
    });
    assert_eq!(streams(&result.output), ["video:vp9", "audio:opus"]);
}

#[test]
fn resolution_frame_rate_and_audio_settings_are_applied() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-settings");
    let input = make_sample(&dir);
    let result = convert(&input, dir.path(), |c| {
        c.container = Container::Matroska;
        c.resolution = Resolution::Custom {
            width: 160,
            height: 120,
        };
        c.frame_rate = FrameRate::Fps30;
        c.channels = Channels::Mono;
        c.sample_rate = SampleRate::Hz22050;
    });
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=width,height,r_frame_rate,channels,sample_rate",
            "-of",
            "default=nw=1",
        ])
        .arg(&result.output)
        .output()
        .unwrap();
    let info = String::from_utf8_lossy(&out.stdout);
    for expected in [
        "width=160",
        "height=120",
        "r_frame_rate=30/1",
        "channels=1",
        "sample_rate=22050",
    ] {
        assert!(info.contains(expected), "missing {expected} in:\n{info}");
    }
}

#[test]
fn video_stream_copy_works() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-copy");
    let input = make_sample(&dir);
    let result = convert(&input, dir.path(), |c| {
        c.container = Container::Matroska;
        c.video_codec = Some(VideoCodec::Copy);
        c.audio_codec = Some(AudioCodec::Copy);
        c.resolution = Resolution::P720; // must be ignored with copy
    });
    assert_eq!(streams(&result.output), ["video:h264", "audio:aac"]);
}

#[test]
fn second_conversion_gets_a_numbered_name() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-numbered");
    let input = make_sample(&dir);
    let first = convert(&input, dir.path(), |c| c.container = Container::Mp3);
    let second = convert(&input, dir.path(), |c| c.container = Container::Mp3);
    assert_eq!(first.output, dir.path().join("sample.mp3"));
    assert_eq!(second.output, dir.path().join("sample (1).mp3"));
}

#[test]
fn progress_output_reaches_100_percent() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-progress");
    let input = make_sample(&dir);
    let result = convert(&input, dir.path(), |c| c.container = Container::Matroska);

    let duration = result
        .stderr
        .lines()
        .find_map(parse_duration_ms)
        .expect("FFmpeg prints the input duration on stderr");
    assert!((duration - 2_000.0).abs() < 100.0, "{duration}");

    // Feed stdout through the same parser the app uses.
    let mut kv = HashMap::new();
    let mut updates = Vec::new();
    for line in result.stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            kv.insert(k.trim().to_string(), v.trim().to_string());
            if k.trim() == "progress" {
                updates.push(build_progress(&kv, Some(duration), 1.0, &None));
                kv.clear();
            }
        }
    }
    let last = updates.last().expect("at least one progress block");
    assert_eq!(last.percentage, 100.0);
    assert!(updates
        .iter()
        .all(|p| (0.0..=100.0).contains(&p.percentage)));
}

#[test]
fn broken_input_gives_a_readable_error() {
    if !ffmpeg_available() {
        return;
    }
    let dir = TempDir::new("e2e-broken");
    let input = dir.path().join("broken.mp4");
    std::fs::write(&input, b"this is not a video").unwrap();
    let mut cfg = config(Container::Mp3);
    cfg.input_path = input.to_string_lossy().into();
    cfg.output_dir = Some(dir.path().to_string_lossy().into());
    let output = resolve_output_path(&input, &cfg).unwrap();
    let plan = build_ffmpeg_args(&cfg, &input, &output);
    let out = Command::new("ffmpeg").args(&plan.args).output().unwrap();
    assert!(!out.status.success());
    let msg = readable_ffmpeg_error(&String::from_utf8_lossy(&out.stderr));
    assert!(msg.contains("could not read the source"), "{msg}");
}

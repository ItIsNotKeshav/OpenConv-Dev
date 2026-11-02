use openconv_core::*;
use std::collections::HashMap;

fn block(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn parses_duration_lines() {
    assert_eq!(
        parse_duration_ms("  Duration: 00:01:30.50, start: 0.000000, bitrate: 1205 kb/s"),
        Some(90_500.0)
    );
    assert_eq!(
        parse_duration_ms("  Duration: 01:00:00.00, start: 0"),
        Some(3_600_000.0)
    );
    // Long recordings have more than two hour digits.
    assert_eq!(
        parse_duration_ms("  Duration: 100:00:00.00, start: 0"),
        Some(360_000_000.0)
    );
    assert_eq!(parse_duration_ms("  Duration: N/A, bitrate: N/A"), None);
    assert_eq!(parse_duration_ms("Stream #0:0: Video: h264"), None);
}

#[test]
fn parses_timestamps() {
    assert_eq!(parse_timestamp_ms("00:00:02.500000"), Some(2_500.0));
    assert_eq!(parse_timestamp_ms("bad"), None);
    assert_eq!(parse_timestamp_ms("1:2"), None);
    assert_eq!(parse_timestamp_ms("-00:00:01.0"), None);
}

#[test]
fn percentage_uses_microsecond_out_time() {
    // Real block from `ffmpeg -progress pipe:1` 1.5 s into a 3 s file. Note that
    // FFmpeg's `out_time_ms` is (despite its name) in microseconds too.
    let kv = block(&[
        ("frame", "37"),
        ("fps", "25.00"),
        ("out_time_us", "1500000"),
        ("out_time_ms", "1500000"),
        ("out_time", "00:00:01.500000"),
        ("total_size", "2048"),
        ("bitrate", "1200.5kbits/s"),
        ("speed", "1.5x"),
        ("progress", "continue"),
    ]);
    let p = build_progress(&kv, Some(3_000.0), 10.0, &Some("H.264".into()));
    assert!((p.percentage - 50.0).abs() < 0.01, "{}", p.percentage);
    assert_eq!(p.remaining_secs.map(|s| s.round()), Some(10.0));
    assert_eq!(p.remaining, "00:10");
    assert_eq!(p.elapsed, "00:10");
    assert_eq!(p.frame, Some(37));
    assert_eq!(p.fps.as_deref(), Some("25.00"));
    assert_eq!(p.speed.as_deref(), Some("1.5x"));
    assert_eq!(p.output_size.as_deref(), Some("2 KB"));
    assert_eq!(p.bitrate.as_deref(), Some("1200.5 kbps"));
    assert_eq!(p.stage, "Encoding Video");
    assert_eq!(p.encoder.as_deref(), Some("H.264"));
}

#[test]
fn percentage_falls_back_to_out_time_ms_and_out_time() {
    let kv = block(&[("out_time_ms", "750000"), ("progress", "continue")]);
    let p = build_progress(&kv, Some(3_000.0), 1.0, &None);
    assert!((p.percentage - 25.0).abs() < 0.01);

    let kv = block(&[("out_time", "00:00:00.750000"), ("progress", "continue")]);
    let p = build_progress(&kv, Some(3_000.0), 1.0, &None);
    assert!((p.percentage - 25.0).abs() < 0.01);
}

#[test]
fn unknown_duration_or_na_values_are_handled() {
    let kv = block(&[
        ("out_time_us", "N/A"),
        ("fps", "0.00"),
        ("speed", "N/A"),
        ("bitrate", "N/A"),
        ("progress", "continue"),
    ]);
    let p = build_progress(&kv, None, 2.0, &None);
    assert_eq!(p.percentage, 0.0);
    assert_eq!(p.remaining, "Calculating…");
    assert_eq!(p.remaining_secs, None);
    assert_eq!(p.fps, None);
    assert_eq!(p.speed, None);
    assert_eq!(p.bitrate, None);
    assert_eq!(p.stage, "Analyzing");
}

#[test]
fn running_percentage_is_capped_below_100_until_the_end() {
    let kv = block(&[("out_time_us", "9000000"), ("progress", "continue")]);
    assert_eq!(
        build_progress(&kv, Some(3_000.0), 1.0, &None).percentage,
        99.0
    );

    let kv = block(&[
        ("out_time_us", "-9223372036854775807"),
        ("progress", "continue"),
    ]);
    assert_eq!(
        build_progress(&kv, Some(3_000.0), 1.0, &None).percentage,
        0.0
    );

    let kv = block(&[("out_time_us", "3000000"), ("progress", "end")]);
    let p = build_progress(&kv, Some(3_000.0), 4.0, &None);
    assert_eq!(p.percentage, 100.0);
    assert_eq!(p.remaining, "00:00");
    assert_eq!(p.stage, "Finalizing");
}

#[test]
fn stage_reflects_what_is_being_encoded() {
    let video = block(&[("frame", "10"), ("fps", "30")]);
    let audio = block(&[("frame", "10"), ("fps", "0.00")]);
    let none = block(&[]);
    assert_eq!(infer_stage(&video, 0.5), "Analyzing");
    assert_eq!(infer_stage(&video, 50.0), "Encoding Video");
    assert_eq!(infer_stage(&audio, 50.0), "Encoding Audio");
    assert_eq!(infer_stage(&none, 50.0), "Encoding");
    assert_eq!(infer_stage(&video, 99.0), "Finalizing");
}

#[test]
fn progress_serializes_in_camel_case_for_the_frontend() {
    let json = serde_json::to_value(FfmpegProgress::default()).unwrap();
    for key in [
        "percentage",
        "elapsed",
        "remaining",
        "remainingSecs",
        "frame",
        "fps",
        "speed",
        "outputSize",
        "bitrate",
        "stage",
        "encoder",
    ] {
        assert!(json.get(key).is_some(), "missing {key} in {json}");
    }
}

#[test]
fn formats_durations() {
    assert_eq!(format_duration(0.0), "00:00");
    assert_eq!(format_duration(59.4), "00:59");
    assert_eq!(format_duration(61.0), "01:01");
    assert_eq!(format_duration(3_661.0), "01:01:01");
    assert_eq!(format_duration(-5.0), "00:00");
    assert_eq!(format_duration(f64::NAN), "00:00");
    assert_eq!(format_duration(f64::INFINITY), "00:00");
}

#[test]
fn formats_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(1023), "1023 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1_572_864), "1.5 MB");
    assert_eq!(format_bytes(2_147_483_648), "2.00 GB");
}

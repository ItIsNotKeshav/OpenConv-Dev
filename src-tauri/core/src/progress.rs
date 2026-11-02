//! Parsing FFmpeg's `-progress pipe:1` output into what the UI shows.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegProgress {
    pub percentage: f32,
    pub elapsed: String,
    pub remaining: String,
    /// Seconds left, so the UI can show a wall-clock ETA in the user's own time zone.
    pub remaining_secs: Option<f64>,
    pub frame: Option<u64>,
    pub fps: Option<String>,
    pub speed: Option<String>,
    pub output_size: Option<String>,
    pub bitrate: Option<String>,
    pub stage: String,
    pub encoder: Option<String>,
}

/// Position in the output, in milliseconds, from one progress block.
///
/// FFmpeg reports `out_time_us` (microseconds) and, for historical reasons, an
/// `out_time_ms` field that is *also* in microseconds. `out_time` is the same
/// value as `HH:MM:SS.micro`.
fn out_time_ms(kv: &HashMap<String, String>) -> Option<f64> {
    let micros = |key: &str| {
        kv.get(key)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v >= 0.0)
            .map(|us| us / 1000.0)
    };
    micros("out_time_us")
        .or_else(|| micros("out_time_ms"))
        .or_else(|| kv.get("out_time").and_then(|v| parse_timestamp_ms(v)))
}

/// Build the progress payload from a completed `key=value` block.
pub fn build_progress(
    kv: &HashMap<String, String>,
    duration_ms: Option<f64>,
    elapsed_wall_secs: f64,
    encoder_label: &Option<String>,
) -> FfmpegProgress {
    let position_ms = out_time_ms(kv).unwrap_or(0.0);
    let finished = kv.get("progress").map(|v| v == "end").unwrap_or(false);

    let percentage = if finished {
        100.0
    } else {
        duration_ms
            .filter(|&d| d > 0.0)
            .map(|d| ((position_ms / d) * 100.0).clamp(0.0, 99.0) as f32)
            .unwrap_or(0.0)
    };

    let remaining_secs = if percentage > 0.0 && percentage < 100.0 {
        Some((elapsed_wall_secs / percentage as f64) * (100.0 - percentage as f64))
    } else {
        None
    };

    let frame = kv.get("frame").and_then(|v| v.parse::<u64>().ok());
    let fps = kv
        .get("fps")
        .filter(|v| v.parse::<f64>().map(|n| n > 0.0).unwrap_or(false))
        .cloned();
    let speed = kv
        .get("speed")
        .filter(|v| {
            v.trim_end_matches('x')
                .trim()
                .parse::<f64>()
                .map(|n| n > 0.0)
                .unwrap_or(false)
        })
        .map(|v| v.trim().to_string());
    let output_size = kv
        .get("total_size")
        .and_then(|v| v.parse::<u64>().ok())
        .map(format_bytes);
    let bitrate = kv
        .get("bitrate")
        .filter(|v| v.as_str() != "N/A")
        .map(|v| v.replace("kbits/s", " kbps"));

    FfmpegProgress {
        percentage,
        elapsed: format_duration(elapsed_wall_secs),
        remaining: match remaining_secs {
            Some(secs) => format_duration(secs),
            None if finished => format_duration(0.0),
            None => "Calculating…".to_string(),
        },
        remaining_secs,
        frame,
        fps,
        speed,
        output_size,
        bitrate,
        stage: infer_stage(kv, percentage),
        encoder: encoder_label.clone(),
    }
}

/// A human-readable encoding stage for the current progress data.
pub fn infer_stage(kv: &HashMap<String, String>, percentage: f32) -> String {
    if percentage < 1.0 {
        return "Analyzing".to_string();
    }
    if percentage > 98.0 {
        return "Finalizing".to_string();
    }
    let has_fps = kv
        .get("fps")
        .and_then(|v| v.parse::<f64>().ok())
        .map(|n| n > 0.0)
        .unwrap_or(false);
    let has_frames = kv
        .get("frame")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0)
        > 0;

    if has_frames && has_fps {
        "Encoding Video"
    } else if has_frames {
        "Encoding Audio"
    } else {
        "Encoding"
    }
    .to_string()
}

/// Parse the `Duration: HH:MM:SS.xx` line FFmpeg prints for each input.
pub fn parse_duration_ms(line: &str) -> Option<f64> {
    let marker = "Duration: ";
    let start = line.find(marker)? + marker.len();
    let rest = &line[start..];
    let end = rest.find(',').unwrap_or(rest.len());
    parse_timestamp_ms(&rest[..end])
}

/// Parse `HH:MM:SS(.frac)` into milliseconds.
pub fn parse_timestamp_ms(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.trim().parse::<f64>().ok()?;
    let minutes = parts.next()?.trim().parse::<f64>().ok()?;
    let seconds = parts.next()?.trim().parse::<f64>().ok()?;
    if parts.next().is_some()
        || [hours, minutes, seconds]
            .iter()
            .any(|v| v.is_sign_negative())
    {
        return None;
    }
    Some((hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0)
}

/// `MM:SS`, or `HH:MM:SS` once past an hour.
pub fn format_duration(secs: f64) -> String {
    let total = if secs.is_finite() && secs > 0.0 {
        secs.round() as u64
    } else {
        0
    };
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1_024;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

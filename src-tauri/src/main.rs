// Prevents terminal window popup on Windows systems when compiling a release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use openconv_core::{
    build_ffmpeg_args, build_progress, container_from_extension, friendly_process_error,
    parse_duration_ms, readable_ffmpeg_error, resolve_ffmpeg_path, resolve_output_path,
    safe_log_name, truncate_log, validate_config, AudioCodec, Channels, Container,
    ConversionConfig, FrameRate, HardwareAcceleration, Preset, Resolution, SampleRate, VideoCodec,
};
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State, Window};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;

// ---------------------------------------------------------------------------
// Event payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionStartedPayload {
    filename: String,
    output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionCompletePayload {
    output_path: String,
    original_size: u64,
    output_size: u64,
    elapsed_secs: f64,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionFailedPayload {
    message: String,
    stderr: String,
}

// Legacy result type kept for compat
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionResult {
    output_path: String,
    message: String,
}

// ---------------------------------------------------------------------------
// ConversionManager — Tauri managed state
// ---------------------------------------------------------------------------

/// Handles shared between the command that started a run and the task driving it.
#[derive(Clone, Default)]
struct RunControl {
    cancelled: Arc<AtomicBool>,
    cancel_signal: Arc<Notify>,
    finished: Arc<AtomicBool>,
}

#[derive(Default)]
struct ConversionManager {
    current: StdMutex<Option<RunControl>>,
}

impl ConversionManager {
    /// Register a new run, refusing if one is still going.
    fn begin(&self) -> Result<RunControl, String> {
        let mut current = self.current.lock().map_err(|e| e.to_string())?;
        if let Some(run) = current.as_ref() {
            if !run.finished.load(Ordering::SeqCst) {
                return Err(
                    "A conversion is already running. Cancel it or wait for it to finish."
                        .to_string(),
                );
            }
        }
        let run = RunControl::default();
        *current = Some(run.clone());
        Ok(run)
    }

    /// Ask the running conversion (if any) to stop; its task kills FFmpeg.
    fn cancel(&self) {
        if let Ok(current) = self.current.lock() {
            if let Some(run) = current.as_ref() {
                run.cancelled.store(true, Ordering::SeqCst);
                // notify_one stores a permit, so this works even if the task
                // is not waiting yet.
                run.cancel_signal.notify_one();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// Start an async conversion. Returns immediately; progress arrives via events.
#[tauri::command]
async fn start_conversion(
    window: Window,
    app: AppHandle,
    manager: State<'_, ConversionManager>,
    config: ConversionConfig,
) -> Result<(), String> {
    validate_config(&config)?;

    let input_path = PathBuf::from(&config.input_path);
    let output_path = resolve_output_path(&input_path, &config)?;
    let plan = build_ffmpeg_args(&config, &input_path, &output_path);
    let ffmpeg_path = resolve_ffmpeg_path();

    let run = manager.begin()?;

    let mut child = match Command::new(&ffmpeg_path)
        .args(&plan.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            run.finished.store(true, Ordering::SeqCst);
            return Err(friendly_process_error(&e));
        }
    };

    // Both pipes were requested above, so these are always present.
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        run.finished.store(true, Ordering::SeqCst);
        return Err("Could not capture FFmpeg output.".to_string());
    };

    let filename = input_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("file")
        .to_string();
    let _ = window.emit(
        "conversion_started",
        ConversionStartedPayload {
            filename: filename.clone(),
            output_path: output_path.to_string_lossy().into_owned(),
        },
    );

    let input_size = fs::metadata(&input_path).map(|m| m.len()).unwrap_or(0);
    let log_path = conversion_log_path(&app, &filename);
    let encoder_label = plan.encoder_label.clone();
    let args = plan.args;

    tokio::spawn(async move {
        let start_time = Instant::now();
        // Written by the stderr reader, read by the progress reader.
        let duration_ms: Arc<StdMutex<Option<f64>>> = Arc::new(StdMutex::new(None));

        let mut log_file = log_path.as_ref().and_then(|p| {
            fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).ok()?;
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
        if let Some(f) = log_file.as_mut() {
            let _ = writeln!(f, "[{}] ffmpeg {}\n", log_timestamp(), args.join(" "));
        }

        // stdout: `key=value` progress blocks from `-progress pipe:1`
        let window_p = window.clone();
        let duration_p = Arc::clone(&duration_ms);
        let progress_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            let mut kv: HashMap<String, String> = HashMap::new();
            while let Ok(Some(line)) = lines.next_line().await {
                let Some((key, value)) = line.split_once('=') else {
                    continue;
                };
                let key = key.trim();
                kv.insert(key.to_string(), value.trim().to_string());
                if key == "progress" {
                    let duration = duration_p.lock().ok().and_then(|d| *d);
                    let progress = build_progress(
                        &kv,
                        duration,
                        start_time.elapsed().as_secs_f64(),
                        &encoder_label,
                    );
                    let _ = window_p.emit("ffmpeg_progress", &progress);
                    kv.clear();
                }
            }
        });

        // stderr: diagnostics; also where FFmpeg prints the input duration
        let duration_e = Arc::clone(&duration_ms);
        let stderr_task = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut log = String::new();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(mut duration) = duration_e.lock() {
                    if duration.is_none() {
                        *duration = parse_duration_ms(&line);
                    }
                }
                if let Some(f) = log_file.as_mut() {
                    let _ = writeln!(f, "{}", line);
                }
                if !line.trim().is_empty() {
                    log.push_str(&line);
                    log.push('\n');
                    truncate_log(&mut log, 10_000, 8_000);
                }
            }
            log
        });

        let status = tokio::select! {
            status = child.wait() => status.ok(),
            _ = run.cancel_signal.notified() => {
                let _ = child.kill().await;
                None
            }
        };

        let _ = progress_task.await;
        let stderr_log = stderr_task.await.unwrap_or_default();
        let elapsed_secs = start_time.elapsed().as_secs_f64();

        if run.cancelled.load(Ordering::SeqCst) {
            // Remove the partial output file
            if output_path.exists() {
                let _ = fs::remove_file(&output_path);
            }
            run.finished.store(true, Ordering::SeqCst);
            let _ = window.emit("conversion_cancelled", ());
            return;
        }

        run.finished.store(true, Ordering::SeqCst);
        match status {
            Some(s) if s.success() => {
                let output_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
                let _ = window.emit(
                    "conversion_complete",
                    ConversionCompletePayload {
                        output_path: output_path.to_string_lossy().into_owned(),
                        original_size: input_size,
                        output_size,
                        elapsed_secs,
                        message: format!("Saved to: {}", output_path.to_string_lossy()),
                    },
                );
            }
            _ => {
                let _ = window.emit(
                    "conversion_failed",
                    ConversionFailedPayload {
                        message: readable_ffmpeg_error(&stderr_log),
                        stderr: stderr_log,
                    },
                );
            }
        }
    });

    Ok(())
}

/// Cancel the running conversion.
#[tauri::command]
async fn cancel_conversion(manager: State<'_, ConversionManager>) -> Result<(), String> {
    manager.cancel();
    Ok(())
}

/// Open the folder containing a file in the OS file manager.
#[tauri::command]
async fn open_in_explorer(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let folder = if p.is_file() {
        p.parent().map(PathBuf::from).unwrap_or(p)
    } else {
        p
    };
    open_with_system(folder.as_os_str())
}

/// Open a file with the default OS application.
#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    open_with_system(OsStr::new(&path))
}

fn open_with_system(target: &OsStr) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        // `explorer` handles both folders and files (opening files with
        // their default app) without going through `cmd` quoting rules.
        let mut c = std::process::Command::new("explorer");
        c.arg(target);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(target);
        c
    };
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(target);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Return the directory where conversion logs are stored.
#[tauri::command]
async fn get_log_dir(app: AppHandle) -> Result<String, String> {
    let dir = app
        .path_resolver()
        .app_log_dir()
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(dir.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Legacy compatibility wrappers
// ---------------------------------------------------------------------------

/// Compatibility wrapper – delegates to start_conversion via event system.
/// Kept so any old callers don't break; the frontend now uses start_conversion.
#[tauri::command]
async fn run_ffmpeg_config(
    window: Window,
    app: AppHandle,
    manager: State<'_, ConversionManager>,
    config: ConversionConfig,
) -> Result<ConversionResult, String> {
    validate_config(&config)?;
    let input_path = PathBuf::from(&config.input_path);
    let output_path = resolve_output_path(&input_path, &config)?;
    let output_path_str = output_path.to_string_lossy().into_owned();

    start_conversion(window, app, manager, config).await?;

    Ok(ConversionResult {
        output_path: output_path_str.clone(),
        message: format!("Conversion started. Output: {}", output_path_str),
    })
}

#[tauri::command]
async fn run_conversion(
    window: Window,
    app: AppHandle,
    manager: State<'_, ConversionManager>,
    input_path: String,
    target_dir: String,
    target_ext: String,
) -> Result<String, String> {
    let container = container_from_extension(&target_ext);
    let config = ConversionConfig::for_legacy(input_path, target_dir, container);
    run_ffmpeg_config(window, app, manager, config)
        .await
        .map(|r| r.message)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
async fn run_compression(
    window: Window,
    app: AppHandle,
    manager: State<'_, ConversionManager>,
    input_path: String,
    target_dir: String,
    crf: String,
    resolution: String,
    bitrate: String,
    mono: bool,
) -> Result<String, String> {
    let resolution = match resolution.as_str() {
        "1920" => Resolution::P1080,
        "1280" => Resolution::P720,
        "854" => Resolution::Custom {
            width: 854,
            height: 480,
        },
        _ => Resolution::Source,
    };
    let copy_audio = bitrate == "copy";
    let config = ConversionConfig {
        input_path,
        output_path: None,
        output_dir: Some(target_dir),
        container: Container::Mp4,
        video_codec: Some(VideoCodec::Libx264),
        audio_codec: Some(if copy_audio {
            AudioCodec::Copy
        } else {
            AudioCodec::Aac
        }),
        resolution,
        frame_rate: FrameRate::Source,
        preset: Preset::Medium,
        crf: crf.parse::<u8>().ok(),
        video_bitrate: None,
        audio_bitrate: if copy_audio { None } else { Some(bitrate) },
        channels: if mono {
            Channels::Mono
        } else {
            Channels::Source
        },
        sample_rate: SampleRate::Source,
        hardware_acceleration: HardwareAcceleration::Off,
        overwrite: false,
        extra_arguments: None,
    };
    run_ffmpeg_config(window, app, manager, config)
        .await
        .map(|r| r.message)
}

// ---------------------------------------------------------------------------
// Logging helpers
// ---------------------------------------------------------------------------

/// `HH:MM:SS` (UTC) for log lines.
fn log_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(
        "{:02}:{:02}:{:02} UTC",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Return a path for the conversion log file.
fn conversion_log_path(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    let log_dir = app.path_resolver().app_log_dir()?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Some(log_dir.join(format!("conversion_{}_{}.log", ts, safe_log_name(filename))))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        .manage(ConversionManager::default())
        .invoke_handler(tauri::generate_handler![
            start_conversion,
            cancel_conversion,
            open_in_explorer,
            open_file,
            get_log_dir,
            run_ffmpeg_config,
            run_conversion,
            run_compression,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

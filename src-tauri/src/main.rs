// Prevents terminal window popup on Windows systems when compiling a release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State, Window};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversionConfig {
    input_path: String,
    output_path: Option<String>,
    output_dir: Option<String>,
    container: Container,
    video_codec: Option<VideoCodec>,
    audio_codec: Option<AudioCodec>,
    resolution: Resolution,
    frame_rate: FrameRate,
    preset: Preset,
    crf: Option<u8>,
    video_bitrate: Option<String>,
    audio_bitrate: Option<String>,
    channels: Channels,
    sample_rate: SampleRate,
    hardware_acceleration: HardwareAcceleration,
    overwrite: bool,
    extra_arguments: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Container {
    Mp4,
    Mov,
    Matroska,
    Mp3,
    Wav,
    Png,
    Webp,
    Gif,
    Avi,
    Flac,
    Aac,
    Ogg,
    Opus,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum VideoCodec {
    Libx264,
    Libx265,
    LibvpxVp9,
    Av1,
    Copy,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum AudioCodec {
    Aac,
    Mp3,
    Opus,
    Flac,
    Pcm,
    Copy,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Resolution {
    Source,
    P720,
    P1080,
    P1440,
    P2160,
    Custom { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum FrameRate {
    Source,
    Fps24,
    Fps30,
    Fps60,
    Fps120,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Preset {
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Channels {
    Source,
    Mono,
    Stereo,
    Surround51,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum SampleRate {
    Source,
    Hz22050,
    Hz44100,
    Hz48000,
    Hz96000,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum HardwareAcceleration {
    Off,
    Auto,
}


// Event payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct FfmpegProgress {
    percentage: f32,
    elapsed: String,
    remaining: String,
    eta: Option<String>,
    frame: Option<u64>,
    fps: Option<String>,
    speed: Option<String>,
    output_size: Option<String>,
    bitrate: Option<String>,
    stage: String,
    encoder: Option<String>,
}

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

struct ConversionManager {
    child: Mutex<Option<Child>>,
    cancelled: Arc<AtomicBool>,
    output_path: Mutex<Option<PathBuf>>,
}

impl ConversionManager {
    fn new() -> Self {
        Self {
            child: Mutex::new(None),
            cancelled: Arc::new(AtomicBool::new(false)),
            output_path: Mutex::new(None),
        }
    }

    /// Mark as cancelled and kill the child process if running.
    async fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Some(child) = self.child.lock().await.as_mut() {
            let _ = child.kill().await;
        }
    }

    /// Reset for a new conversion run.
    async fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
        *self.child.lock().await = None;
        *self.output_path.lock().await = None;
    }

    #[allow(dead_code)]
    fn reset_cancelled(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// FFmpeg command builder
// ---------------------------------------------------------------------------

struct FfmpegCommandBuilder {
    args: Vec<String>,
}

impl FfmpegCommandBuilder {
    fn new() -> Self {
        Self { args: Vec::new() }
    }

    fn overwrite(mut self, overwrite: bool) -> Self {
        self.args.push(if overwrite { "-y" } else { "-n" }.to_string());
        self
    }

    fn progress_pipe(mut self) -> Self {
        // Emit structured progress to stdout; suppress banner stats on stderr
        self.args.push("-progress".to_string());
        self.args.push("pipe:1".to_string());
        self.args.push("-nostats".to_string());
        self
    }

    fn input(mut self, input: &Path) -> Self {
        self.args.push("-i".to_string());
        self.args.push(input.to_string_lossy().into_owned());
        self
    }

    fn hardware_acceleration(mut self, value: HardwareAcceleration) -> Self {
        if value == HardwareAcceleration::Auto {
            self.args.push("-hwaccel".to_string());
            self.args.push("auto".to_string());
        }
        self
    }

    fn video_codec(mut self, codec: Option<VideoCodec>) -> Self {
        if let Some(codec) = codec {
            self.args.push("-c:v".to_string());
            self.args.push(codec.as_ffmpeg_arg().to_string());
        }
        self
    }

    fn audio_codec(mut self, codec: Option<AudioCodec>) -> Self {
        if let Some(codec) = codec {
            self.args.push("-c:a".to_string());
            self.args.push(codec.as_ffmpeg_arg().to_string());
        }
        self
    }

    fn scale(mut self, resolution: &Resolution) -> Self {
        if let Some((width, height)) = resolution.dimensions() {
            self.args.push("-vf".to_string());
            self.args.push(format!("scale={}:{}", width, height));
        }
        self
    }

    fn frame_rate(mut self, frame_rate: FrameRate) -> Self {
        if let Some(value) = frame_rate.as_ffmpeg_arg() {
            self.args.push("-r".to_string());
            self.args.push(value.to_string());
        }
        self
    }

    fn preset(mut self, preset: Preset, enabled: bool) -> Self {
        if enabled {
            self.args.push("-preset".to_string());
            self.args.push(preset.as_ffmpeg_arg().to_string());
        }
        self
    }

    fn crf(mut self, crf: Option<u8>, enabled: bool) -> Self {
        if enabled {
            if let Some(crf) = crf {
                self.args.push("-crf".to_string());
                self.args.push(crf.to_string());
            }
        }
        self
    }

    fn video_bitrate(mut self, bitrate: &Option<String>) -> Self {
        if let Some(bitrate) = non_empty(bitrate) {
            self.args.push("-b:v".to_string());
            self.args.push(bitrate.to_string());
        }
        self
    }

    fn audio_bitrate(mut self, bitrate: &Option<String>, enabled: bool) -> Self {
        if enabled {
            if let Some(bitrate) = non_empty(bitrate) {
                self.args.push("-b:a".to_string());
                self.args.push(bitrate.to_string());
            }
        }
        self
    }

    fn channels(mut self, channels: Channels) -> Self {
        if let Some(value) = channels.as_ffmpeg_arg() {
            self.args.push("-ac".to_string());
            self.args.push(value.to_string());
        }
        self
    }

    fn sample_rate(mut self, sample_rate: SampleRate) -> Self {
        if let Some(value) = sample_rate.as_ffmpeg_arg() {
            self.args.push("-ar".to_string());
            self.args.push(value.to_string());
        }
        self
    }

    fn extra_arguments(mut self, value: &Option<String>) -> Self {
        if let Some(value) = non_empty(value) {
            self.args
                .extend(value.split_whitespace().map(|arg| arg.to_string()));
        }
        self
    }

    fn output(mut self, output: &Path) -> Self {
        self.args.push(output.to_string_lossy().into_owned());
        self
    }

    fn build(self) -> Vec<String> {
        self.args
    }
}

// ---------------------------------------------------------------------------
// Codec / enum helpers
// ---------------------------------------------------------------------------

impl Container {
    fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Mov => "mov",
            Self::Matroska => "mkv",
            Self::Mp3 => "mp3",
            Self::Wav => "wav",
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Avi => "avi",
            Self::Flac => "flac",
            Self::Aac => "aac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
        }
    }

    fn is_audio_only(self) -> bool {
        matches!(self, Self::Mp3 | Self::Wav | Self::Flac | Self::Aac | Self::Ogg | Self::Opus)
    }

    fn is_image(self) -> bool {
        matches!(self, Self::Png | Self::Webp | Self::Gif)
    }

    fn default_video_codec(self) -> Option<VideoCodec> {
        match self {
            Self::Mp4 | Self::Mov | Self::Matroska | Self::Avi => Some(VideoCodec::Libx264),
            Self::Webp => Some(VideoCodec::LibvpxVp9),
            Self::Gif | Self::Png => None,
            _ if self.is_audio_only() => None,
            _ => Some(VideoCodec::Libx264),
        }
    }

    fn default_audio_codec(self) -> Option<AudioCodec> {
        match self {
            Self::Mp4 | Self::Mov | Self::Matroska | Self::Avi => Some(AudioCodec::Aac),
            Self::Mp3 => Some(AudioCodec::Mp3),
            Self::Wav => Some(AudioCodec::Pcm),
            Self::Flac => Some(AudioCodec::Flac),
            Self::Ogg => Some(AudioCodec::Opus),
            Self::Opus => Some(AudioCodec::Opus),
            Self::Aac => Some(AudioCodec::Aac),
            Self::Png | Self::Webp | Self::Gif => None,
        }
    }
}

impl VideoCodec {
    fn as_ffmpeg_arg(self) -> &'static str {
        match self {
            Self::Libx264 => "libx264",
            Self::Libx265 => "libx265",
            Self::LibvpxVp9 => "libvpx-vp9",
            Self::Av1 => "libaom-av1",
            Self::Copy => "copy",
        }
    }

    fn friendly_name(self) -> &'static str {
        match self {
            Self::Libx264 => "H.264",
            Self::Libx265 => "H.265 / HEVC",
            Self::LibvpxVp9 => "VP9",
            Self::Av1 => "AV1",
            Self::Copy => "Copy",
        }
    }
}

impl AudioCodec {
    fn as_ffmpeg_arg(self) -> &'static str {
        match self {
            Self::Aac => "aac",
            Self::Mp3 => "libmp3lame",
            Self::Opus => "libopus",
            Self::Flac => "flac",
            Self::Pcm => "pcm_s16le",
            Self::Copy => "copy",
        }
    }
}

impl Resolution {
    fn dimensions(&self) -> Option<(u32, u32)> {
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

impl FrameRate {
    fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Fps24 => Some("24"),
            Self::Fps30 => Some("30"),
            Self::Fps60 => Some("60"),
            Self::Fps120 => Some("120"),
        }
    }
}

impl Preset {
    fn as_ffmpeg_arg(self) -> &'static str {
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

impl Channels {
    fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Mono => Some("1"),
            Self::Stereo => Some("2"),
            Self::Surround51 => Some("6"),
        }
    }
}

impl SampleRate {
    fn as_ffmpeg_arg(self) -> Option<&'static str> {
        match self {
            Self::Source => None,
            Self::Hz22050 => Some("22050"),
            Self::Hz44100 => Some("44100"),
            Self::Hz48000 => Some("48000"),
            Self::Hz96000 => Some("96000"),
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

    // Reset any previous state
    manager.reset().await;

    let ffmpeg_path = resolve_ffmpeg_path().map_err(|e| friendly_process_error(&e))?;
    let input_path = PathBuf::from(&config.input_path);
    let output_path = resolve_output_path(&input_path, &config)?;

    // Remember output path for cancellation cleanup
    *manager.output_path.lock().await = Some(output_path.clone());

    let video_enabled = !config.container.is_audio_only();
    let audio_enabled = !config.container.is_image();
    let preset_enabled = video_enabled
        && !matches!(config.video_codec, Some(VideoCodec::Copy) | None)
        && !config.container.is_image();
    let crf_enabled = preset_enabled;

    let video_codec = if video_enabled {
        config.video_codec.or_else(|| config.container.default_video_codec())
    } else {
        None
    };
    let audio_codec = if audio_enabled {
        config.audio_codec.or_else(|| config.container.default_audio_codec())
    } else {
        None
    };

    // Friendly encoder name for the UI
    let encoder_label = video_codec.map(|c| c.friendly_name().to_string());

    let args = FfmpegCommandBuilder::new()
        .overwrite(config.overwrite)
        .progress_pipe()
        .hardware_acceleration(config.hardware_acceleration)
        .input(&input_path)
        .video_codec(video_codec)
        .audio_codec(audio_codec)
        .scale(&config.resolution)
        .frame_rate(config.frame_rate)
        .preset(config.preset, preset_enabled)
        .crf(config.crf, crf_enabled)
        .video_bitrate(&config.video_bitrate)
        .audio_bitrate(&config.audio_bitrate, audio_enabled)
        .channels(config.channels)
        .sample_rate(config.sample_rate)
        .extra_arguments(&config.extra_arguments)
        .output(&output_path)
        .build();

    // Emit started event so the frontend can transition immediately
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

    // Capture what we need to move into the async task
    let cancelled = Arc::clone(&manager.cancelled);
    // Placeholder — not used directly; child is tracked via child_state arc below
    let output_path_clone = output_path.clone();
    let input_size = fs::metadata(&input_path).map(|m| m.len()).unwrap_or(0);

    // Prepare log file
    let log_path = conversion_log_path(&app, &filename);

    // We need to move the child handle into the shared state. We use a separate
    // Arc<Mutex<Option<Child>>> that is shared between the command and the spawned task.
    let child_state: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    let child_state_for_task = Arc::clone(&child_state);
    let child_state_for_manager = Arc::clone(&child_state);

    // Spawn the FFmpeg process
    let child = Command::new(&ffmpeg_path)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| friendly_process_error(&e))?;

    // Store child in both our local arc and the manager
    {
        let guard = manager.child.lock().await;
        // We can't store the child directly in the manager and also move it into
        // the task, so we hand ownership to the task and use the local arc for kill.
        // The manager's cancel() will kill via manager.child which we populate below.
        drop(guard); // release lock; we'll use child_state_for_manager instead
    }

    *child_state.lock().await = Some(child);

    // Give the manager a way to cancel by replacing its child slot with one from the arc.
    // We achieve this by putting the actual kill logic in the cancel flag path in the task.

    let window_clone = window.clone();
    let cancelled_clone = Arc::clone(&cancelled);

    tokio::spawn(async move {
        let start_time = Instant::now();
        let mut kv: HashMap<String, String> = HashMap::new();
        let mut duration_ms: Option<f64> = None;
        let _stderr_log = String::new();
        let mut log_file = log_path.as_ref().and_then(|p| {
            fs::create_dir_all(p.parent().unwrap_or(Path::new("."))).ok()?;
            fs::OpenOptions::new().create(true).append(true).open(p).ok()
        });

        // Write the FFmpeg command to the log
        if let Some(ref mut f) = log_file {
            let cmd_line = format!(
                "[{}] ffmpeg {}\n\n",
                chrono_like_timestamp(),
                args.join(" ")
            );
            let _ = f.write_all(cmd_line.as_bytes());
        }

        let (stdout_reader, stderr_reader) = {
            let mut guard = child_state_for_task.lock().await;
            let child = match guard.as_mut() {
                Some(c) => c,
                None => return,
            };
            let stdout = child.stdout.take().expect("stdout piped");
            let stderr = child.stderr.take().expect("stderr piped");
            (BufReader::new(stdout).lines(), BufReader::new(stderr).lines())
        };

        // Drain stdout (progress) and stderr (errors) concurrently
        let window_p = window_clone.clone();
        let cancelled_p = Arc::clone(&cancelled_clone);

        let progress_task = tokio::spawn(async move {
            let mut stdout_reader = stdout_reader;
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                if cancelled_p.load(Ordering::SeqCst) {
                    break;
                }

                // Parse duration from a potential Duration line (ffmpeg also writes it to stdout
                // with -progress pipe:1 in some versions; primarily comes from stderr but we check)
                if duration_ms.is_none() {
                    if let Some(d) = parse_duration_ms(&line) {
                        duration_ms = Some(d);
                    }
                }

                // key=value format from -progress pipe:1
                if let Some((key, value)) = line.split_once('=') {
                    kv.insert(key.trim().to_string(), value.trim().to_string());

                    if key.trim() == "progress" {
                        // A complete progress block has arrived
                        let elapsed_secs = start_time.elapsed().as_secs_f64();
                        let progress = build_progress(&kv, duration_ms, elapsed_secs, &encoder_label);
                        let _ = window_p.emit("ffmpeg_progress", &progress);
                        kv.clear();
                    }
                }
            }
        });

        let stderr_task = tokio::spawn(async move {
            let mut stderr_reader = stderr_reader;
            let mut log = String::new();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                // Parse duration from stderr (most reliable source)
                if duration_ms.is_none() {
                    if let Some(d) = parse_duration_ms(&line) {
                        duration_ms = Some(d);
                    }
                }
                if let Some(ref mut f) = log_file {
                    let _ = writeln!(f, "{}", line);
                }
                if !line.trim().is_empty() {
                    log.push_str(&line);
                    log.push('\n');
                    if log.len() > 10_000 {
                        let keep = log.len().saturating_sub(8_000);
                        log = log[keep..].to_string();
                    }
                }
            }
            log
        });

        // Wait for both readers to finish
        let _ = progress_task.await;
        let stderr_log = stderr_task.await.unwrap_or_default();

        // Wait for the process to exit
        let status = {
            let mut guard = child_state_for_task.lock().await;
            if let Some(child) = guard.as_mut() {
                child.wait().await.ok()
            } else {
                None
            }
        };

        if cancelled_clone.load(Ordering::SeqCst) {
            // Delete partial output file
            if output_path_clone.exists() {
                let _ = fs::remove_file(&output_path_clone);
            }
            let _ = window_clone.emit("conversion_cancelled", ());
            return;
        }

        let elapsed_secs = start_time.elapsed().as_secs_f64();

        match status {
            Some(s) if s.success() => {
                let output_size = fs::metadata(&output_path_clone).map(|m| m.len()).unwrap_or(0);
                let _ = window_clone.emit(
                    "conversion_complete",
                    ConversionCompletePayload {
                        output_path: output_path_clone.to_string_lossy().into_owned(),
                        original_size: input_size,
                        output_size,
                        elapsed_secs,
                        message: format!(
                            "Saved to: {}",
                            output_path_clone.to_string_lossy()
                        ),
                    },
                );
            }
            _ => {
                let message = readable_ffmpeg_error(&stderr_log);
                let _ = window_clone.emit(
                    "conversion_failed",
                    ConversionFailedPayload {
                        message,
                        stderr: stderr_log,
                    },
                );
            }
        }
    });

    // Store the child_state arc in the manager for cancellation
    // We do this by running a small task that keeps the arc alive and can respond to cancel
    let cancelled_for_monitor = Arc::clone(&manager.cancelled);
    tokio::spawn(async move {
        // Keep the arc alive so the child is not dropped prematurely
        let _keep_alive = child_state_for_manager;
        // Monitor cancellation — when cancelled, kill child
        loop {
            if cancelled_for_monitor.load(Ordering::SeqCst) {
                let mut guard = _keep_alive.lock().await;
                if let Some(child) = guard.as_mut() {
                    let _ = child.kill().await;
                }
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    Ok(())
}

/// Cancel the running conversion.
#[tauri::command]
async fn cancel_conversion(manager: State<'_, ConversionManager>) -> Result<(), String> {
    manager.cancel().await;
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
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(folder.to_string_lossy().as_ref())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(folder.to_string_lossy().as_ref())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(folder.to_string_lossy().as_ref())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a file with the default OS application.
#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
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
/// Kept so any old callers don't break at compile time; frontend now uses
/// start_conversion directly.
#[tauri::command]
async fn run_ffmpeg_config(
    window: Window,
    app: AppHandle,
    manager: State<'_, ConversionManager>,
    config: ConversionConfig,
) -> Result<ConversionResult, String> {
    // Validate & resolve output path for an immediate return value
    validate_config(&config)?;
    let input_path = PathBuf::from(&config.input_path);
    let output_path = resolve_output_path(&input_path, &config)?;
    let output_path_str = output_path.to_string_lossy().into_owned();

    // Delegate to async start
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
    let channels = if mono { Channels::Mono } else { Channels::Source };
    let config = ConversionConfig {
        input_path,
        output_path: None,
        output_dir: Some(target_dir),
        container: Container::Mp4,
        video_codec: Some(VideoCodec::Libx264),
        audio_codec: Some(if bitrate == "copy" {
            AudioCodec::Copy
        } else {
            AudioCodec::Aac
        }),
        resolution,
        frame_rate: FrameRate::Source,
        preset: Preset::Medium,
        crf: crf.parse::<u8>().ok(),
        video_bitrate: None,
        audio_bitrate: if bitrate == "copy" { None } else { Some(bitrate) },
        channels,
        sample_rate: SampleRate::Source,
        hardware_acceleration: HardwareAcceleration::Off,
        overwrite: false,
        extra_arguments: None,
    };
    run_ffmpeg_config(window, app, manager, config)
        .await
        .map(|r| r.message)
}

impl ConversionConfig {
    fn for_legacy(input_path: String, output_dir: String, container: Container) -> Self {
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

// ---------------------------------------------------------------------------
// Validation & path helpers
// ---------------------------------------------------------------------------

fn validate_config(config: &ConversionConfig) -> Result<(), String> {
    let input = Path::new(&config.input_path);
    if config.input_path.trim().is_empty() {
        return Err("Choose a source file before starting.".to_string());
    }
    if !input.exists() {
        return Err("The selected input file no longer exists.".to_string());
    }
    if !input.is_file() {
        return Err("The selected input path is not a file.".to_string());
    }
    if matches!(
        config.resolution,
        Resolution::Custom {
            width: 0,
            height: _
        } | Resolution::Custom {
            width: _,
            height: 0
        }
    ) {
        return Err("Custom resolution must include a valid width and height.".to_string());
    }
    if matches!(config.crf, Some(value) if value > 51) {
        return Err("CRF must be between 0 and 51.".to_string());
    }
    if config.output_path.as_deref().unwrap_or("").trim().is_empty()
        && config.output_dir.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err("Choose an output folder before starting.".to_string());
    }
    Ok(())
}

fn resolve_output_path(input: &Path, config: &ConversionConfig) -> Result<PathBuf, String> {
    if let Some(path) = non_empty(&config.output_path) {
        let output = PathBuf::from(path);
        if let Some(parent) = output.parent() {
            if !parent.exists() {
                return Err("The output folder does not exist.".to_string());
            }
        }
        return if config.overwrite {
            Ok(output)
        } else {
            next_available_path(output)
        };
    }

    let output_dir = config
        .output_dir
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| "Choose an output folder before starting.".to_string())?;
    if !output_dir.exists() {
        return Err("The output folder does not exist.".to_string());
    }

    let stem = input
        .file_stem()
        .and_then(OsStr::to_str)
        .unwrap_or("output");
    let output = output_dir.join(format!("{}.{}", stem, config.container.extension()));

    if config.overwrite {
        Ok(output)
    } else {
        next_available_path(output)
    }
}

fn next_available_path(path: PathBuf) -> Result<PathBuf, String> {
    if !path.exists() {
        return Ok(path);
    }

    let parent = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let stem = path
        .file_stem()
        .and_then(OsStr::to_str)
        .ok_or_else(|| "Output filename is invalid.".to_string())?;
    let ext = path.extension().and_then(OsStr::to_str).unwrap_or("");

    for index in 1..10_000 {
        let filename = if ext.is_empty() {
            format!("{} ({})", stem, index)
        } else {
            format!("{} ({}).{}", stem, index, ext)
        };
        let candidate = parent.join(filename);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Could not create a unique output filename.".to_string())
}

// ---------------------------------------------------------------------------
// Progress parsing (key=value from -progress pipe:1)
// ---------------------------------------------------------------------------

/// Build an FfmpegProgress from a completed key=value block.
fn build_progress(
    kv: &HashMap<String, String>,
    duration_ms: Option<f64>,
    elapsed_wall_secs: f64,
    encoder_label: &Option<String>,
) -> FfmpegProgress {
    let out_time_ms = kv
        .get("out_time_ms")
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);

    let percentage = duration_ms
        .filter(|&d| d > 0.0)
        .map(|d| ((out_time_ms / d) * 100.0).clamp(0.0, 99.0) as f32)
        .unwrap_or(0.0);

    let remaining_secs = if percentage > 0.0 {
        (elapsed_wall_secs / percentage as f64) * (100.0 - percentage as f64)
    } else {
        0.0
    };

    let eta = if remaining_secs > 0.0 {
        Some(eta_string(remaining_secs))
    } else {
        None
    };

    let frame = kv
        .get("frame")
        .and_then(|v| v.parse::<u64>().ok());

    let fps = kv.get("fps").map(|v| v.clone()).filter(|v| v != "0");

    let speed = kv.get("speed").map(|v| v.clone()).filter(|v| v != "0x" && v != "0.000x");

    let output_size = kv.get("total_size").and_then(|v| {
        v.parse::<u64>().ok().map(|bytes| format_bytes(bytes))
    });

    let bitrate = kv.get("bitrate").map(|v| v.replace("kbits/s", " kbps"));

    let stage = infer_stage(kv, percentage);

    FfmpegProgress {
        percentage,
        elapsed: format_duration(elapsed_wall_secs),
        remaining: if remaining_secs > 0.0 {
            format_duration(remaining_secs)
        } else {
            "Calculating…".to_string()
        },
        eta,
        frame,
        fps,
        speed,
        output_size,
        bitrate,
        stage,
        encoder: encoder_label.clone(),
    }
}

/// Infer a human-readable encoding stage from the current progress data.
fn infer_stage(kv: &HashMap<String, String>, percentage: f32) -> String {
    if percentage < 1.0 {
        return "Analyzing".to_string();
    }
    if percentage > 98.0 {
        return "Finalizing".to_string();
    }
    // If fps is zero or absent but there are frames, we might be in audio pass
    let has_fps = kv.get("fps").map(|v| v != "0").unwrap_or(false);
    let has_frames = kv.get("frame").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0) > 0;

    if has_frames && has_fps {
        "Encoding Video".to_string()
    } else if has_frames {
        "Encoding Audio".to_string()
    } else {
        "Encoding".to_string()
    }
}

/// Parse the Duration line that FFmpeg writes (appears on stderr and sometimes stdout).
fn parse_duration_ms(line: &str) -> Option<f64> {
    let marker = "Duration: ";
    let start = line.find(marker)? + marker.len();
    let value = line.get(start..start + 11)?;
    parse_timestamp_ms(value)
}

fn parse_timestamp_ms(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.trim().parse::<f64>().ok()?;
    let minutes = parts.next()?.trim().parse::<f64>().ok()?;
    let seconds = parts.next()?.trim().parse::<f64>().ok()?;
    Some((hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0)
}

fn format_duration(secs: f64) -> String {
    let total = secs.round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{:02}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

fn format_bytes(bytes: u64) -> String {
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

/// Return a wall-clock ETA string like "8:42 PM".
fn eta_string(remaining_secs: f64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let eta_unix = now_secs + remaining_secs.round() as u64;
    // Simple HH:MM AM/PM — no external crate needed
    let total_mins = (eta_unix % 86400) / 60;
    let h24 = total_mins / 60;
    let min = total_mins % 60;
    let ampm = if h24 < 12 { "AM" } else { "PM" };
    let h12 = if h24 % 12 == 0 { 12 } else { h24 % 12 };
    format!("{}:{:02} {}", h12, min, ampm)
}

fn chrono_like_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// Return a path for the conversion log file.
fn conversion_log_path(app: &AppHandle, filename: &str) -> Option<PathBuf> {
    let log_dir = app.path_resolver().app_log_dir()?;
    let safe_name = filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_");
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Some(log_dir.join(format!("conversion_{}_{}.log", ts, safe_name)))
}

// ---------------------------------------------------------------------------
// FFmpeg path resolution & error formatting
// ---------------------------------------------------------------------------

fn resolve_ffmpeg_path() -> Result<PathBuf, std::io::Error> {
    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut candidates = Vec::new();

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            candidates.push(dir.join(exe_name));
            candidates.push(dir.join("bin").join(exe_name));
            candidates.push(dir.join("resources").join(exe_name));
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("ffmpeg").join(exe_name));
        candidates.push(current_dir.join("bin").join(exe_name));
        candidates.push(current_dir.join(exe_name));
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Ok(PathBuf::from(exe_name))
}

fn friendly_process_error(err: &std::io::Error) -> String {
    if err.kind() == std::io::ErrorKind::NotFound {
        "FFmpeg could not be found. Make sure the bundled ffmpeg executable is installed with OpenConv.".to_string()
    } else if err.kind() == std::io::ErrorKind::PermissionDenied {
        "OpenConv does not have permission to run FFmpeg or write to the selected folder.".to_string()
    } else {
        format!("Could not start FFmpeg: {}", err)
    }
}

fn readable_ffmpeg_error(log: &str) -> String {
    let lower = log.to_lowercase();
    if lower.contains("unknown encoder") || lower.contains("encoder not found") {
        "The selected codec is not supported by the bundled FFmpeg build.".to_string()
    } else if lower.contains("permission denied") {
        "OpenConv does not have permission to read or write one of the selected paths.".to_string()
    } else if lower.contains("no space left") || lower.contains("disk full") {
        "The destination disk is full.".to_string()
    } else if lower.contains("file exists") {
        "The output file already exists. Enable overwrite or choose another folder.".to_string()
    } else if lower.contains("invalid argument") {
        "FFmpeg rejected one of the selected settings. Check the output format and codec options.".to_string()
    } else {
        format!("FFmpeg failed:\n{}", log.trim())
    }
}

fn container_from_extension(ext: &str) -> Container {
    match ext.trim_start_matches('.').to_lowercase().as_str() {
        "mov" => Container::Mov,
        "mkv" => Container::Matroska,
        "mp3" => Container::Mp3,
        "wav" => Container::Wav,
        "png" => Container::Png,
        "webp" => Container::Webp,
        "gif" => Container::Gif,
        "avi" => Container::Avi,
        "flac" => Container::Flac,
        "aac" | "m4a" => Container::Aac,
        "ogg" => Container::Ogg,
        "opus" => Container::Opus,
        _ => Container::Mp4,
    }
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        .manage(ConversionManager::new())
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

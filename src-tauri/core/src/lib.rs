//! Window-independent logic for OpenConv.
//!
//! Everything in here is plain Rust with no Tauri dependency, so it can be unit
//! tested on any machine (`cargo test -p openconv-core`). The Tauri app in
//! `src-tauri/src/main.rs` wires these pieces up to the UI.

pub mod command;
pub mod config;
pub mod errors;
pub mod paths;
pub mod progress;

pub use command::{build_ffmpeg_args, FfmpegCommandBuilder, FfmpegPlan};
pub use config::*;
pub use errors::{friendly_process_error, readable_ffmpeg_error, truncate_log};
pub use paths::{
    container_from_extension, next_available_path, resolve_ffmpeg_path, resolve_output_path,
    safe_log_name, validate_config,
};
pub use progress::{
    build_progress, format_bytes, format_duration, infer_stage, parse_duration_ms,
    parse_timestamp_ms, FfmpegProgress,
};

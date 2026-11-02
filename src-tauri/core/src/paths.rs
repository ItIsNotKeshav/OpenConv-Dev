//! Input validation and output / FFmpeg path resolution.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::config::{non_empty, Container, ConversionConfig, Resolution};

/// Check a config before anything touches the disk or spawns FFmpeg.
pub fn validate_config(config: &ConversionConfig) -> Result<(), String> {
    if config.input_path.trim().is_empty() {
        return Err("Choose a source file before starting.".to_string());
    }
    let input = Path::new(&config.input_path);
    if !input.exists() {
        return Err("The selected input file no longer exists.".to_string());
    }
    if !input.is_file() {
        return Err("The selected input path is not a file.".to_string());
    }
    if let Resolution::Custom { width, height } = config.resolution {
        if width == 0 || height == 0 {
            return Err("Custom resolution must include a valid width and height.".to_string());
        }
    }
    if matches!(config.crf, Some(value) if value > 51) {
        return Err("CRF must be between 0 and 51.".to_string());
    }
    if non_empty(&config.output_path).is_none() && non_empty(&config.output_dir).is_none() {
        return Err("Choose an output folder before starting.".to_string());
    }
    Ok(())
}

/// Work out where the output file goes. Unless `overwrite` is set, an existing
/// file is never clobbered: `name (1).ext`, `name (2).ext`, ... are tried instead.
pub fn resolve_output_path(input: &Path, config: &ConversionConfig) -> Result<PathBuf, String> {
    let output = if let Some(path) = non_empty(&config.output_path) {
        let output = PathBuf::from(path);
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            if !parent.is_dir() {
                return Err("The output folder does not exist.".to_string());
            }
        }
        output
    } else {
        let output_dir = non_empty(&config.output_dir)
            .map(PathBuf::from)
            .ok_or_else(|| "Choose an output folder before starting.".to_string())?;
        if !output_dir.is_dir() {
            return Err("The output folder does not exist.".to_string());
        }
        let stem = input
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or("output");
        output_dir.join(format!("{}.{}", stem, config.container.extension()))
    };

    if same_file(input, &output) {
        return if config.overwrite {
            Err(
                "The output would overwrite the source file. Choose another folder or format."
                    .to_string(),
            )
        } else {
            next_available_path(output)
        };
    }

    if config.overwrite {
        Ok(output)
    } else {
        next_available_path(output)
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Return `path` if it is free, otherwise the first free `stem (N).ext` beside it.
pub fn next_available_path(path: PathBuf) -> Result<PathBuf, String> {
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

/// Map a file extension (with or without a leading dot) to a container.
/// Unknown extensions fall back to MP4.
pub fn container_from_extension(ext: &str) -> Container {
    let ext = ext.trim().trim_start_matches('.').to_lowercase();
    Container::ALL
        .into_iter()
        .find(|c| c.extension() == ext)
        .unwrap_or(match ext.as_str() {
            "matroska" => Container::Matroska,
            _ => Container::Mp4,
        })
}

/// Make a file name safe to embed in a log file name on every OS.
pub fn safe_log_name(filename: &str) -> String {
    filename.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_")
}

/// Find FFmpeg: next to the executable, in a `bin`/`resources` folder, in the
/// working directory, and finally fall back to whatever `ffmpeg` is on PATH.
pub fn resolve_ffmpeg_path() -> PathBuf {
    let exe_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
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

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(exe_name))
}

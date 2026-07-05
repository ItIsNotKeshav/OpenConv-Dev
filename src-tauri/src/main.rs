// Prevents terminal window popup on Windows systems when compiling a release build
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;
use std::process::Command;

/// NATIVE HANDLER A: Transcodes format structures smoothly
#[tauri::command]
fn run_conversion(
    input_path: String,
    target_dir: String,
    target_ext: String,
) -> Result<String, String> {
    let input = Path::new(&input_path);
    let file_stub = input
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "Converted_File".into());

    // Construct target destination file absolute path string
    let output_filename = format!("{}_converted.{}", file_stub, target_ext);
    let output_path = Path::new(&target_dir).join(output_filename);
    let output_str = output_path.to_string_lossy().into_owned();

    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(&input_path)
        .arg(&output_str)
        .output();

    match output {
        Ok(res) if res.status.success() => Ok(format!("Saved to: {}", output_str)),
        Ok(res) => Err(format!(
            "
            Transcode Error: {}",
            String::from_utf8_lossy(&res.stderr)
        )),
        Err(err) => Err(format!(
            "Could not look up local FFmpeg path installation: {}",
            err
        )),
    }
}

/// NATIVE HANDLER B: Compresses files using multi-pass parameters mapped from UI elements
#[tauri::command]
fn run_compression(
    input_path: String,
    target_dir: String,
    crf: String,
    resolution: String,
    bitrate: String,
    mono: bool,
) -> Result<String, String> {
    let input = Path::new(&input_path);
    let file_stub = input
        .file_stem()
        .map(|s| s.to_string_lossy())
        .unwrap_or_else(|| "Compressed_File".into());
    let original_ext = input
        .extension()
        .map(|e| e.to_string_lossy())
        .unwrap_or_else(|| "mp4".into());

    let output_filename = format!("{}_compressed.{}", file_stub, original_ext);
    let output_path = Path::new(&target_dir).join(output_filename);
    let output_str = output_path.to_string_lossy().into_owned();

    // Begin building arguments vector dynamically matching the parameters
    let mut ffmpeg = Command::new("ffmpeg");
    ffmpeg.arg("-i").arg(&input_path);

    // Apply Video Parameters safely
    ffmpeg.arg("-c:v").arg("libx264").arg("-crf").arg(&crf);
    if resolution != "auto" {
        ffmpeg
            .arg("-vf")
            .arg(format!("scale='min({},iw)':-2", resolution));
    }

    // Apply Audio Parameters safely
    if bitrate == "copy" {
        ffmpeg.arg("-c:a").arg("copy");
    } else {
        ffmpeg.arg("-c:a").arg("aac").arg("-b:a").arg(&bitrate);
        if mono {
            ffmpeg.arg("-ac").arg("1");
        }
    }

    let output = ffmpeg.arg(&output_str).output();

    match output {
        Ok(res) if res.status.success() => Ok(format!(
            "Compressed file successfully saved to: {}",
            output_str
        )),
        Ok(res) => Err(format!(
            "FFmpeg Compression Error: {}",
            String::from_utf8_lossy(&res.stderr)
        )),
        Err(err) => Err(format!(
            "Failed running process thread layout constraints: {}",
            err
        )),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run_conversion, run_compression])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

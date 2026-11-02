//! Turning process and FFmpeg failures into messages a user can act on.

/// Message for a failure to *start* FFmpeg.
pub fn friendly_process_error(err: &std::io::Error) -> String {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            "FFmpeg could not be found. Install FFmpeg and make sure `ffmpeg` is on your PATH, or place it next to OpenConv.".to_string()
        }
        std::io::ErrorKind::PermissionDenied => {
            "OpenConv does not have permission to run FFmpeg or write to the selected folder."
                .to_string()
        }
        _ => format!("Could not start FFmpeg: {}", err),
    }
}

/// Summarise FFmpeg's stderr after a failed run.
pub fn readable_ffmpeg_error(log: &str) -> String {
    let lower = log.to_lowercase();
    if lower.contains("unknown encoder") || lower.contains("encoder not found") {
        "The selected codec is not supported by the installed FFmpeg build.".to_string()
    } else if lower.contains("permission denied") {
        "OpenConv does not have permission to read or write one of the selected paths.".to_string()
    } else if lower.contains("no space left") || lower.contains("disk full") {
        "The destination disk is full.".to_string()
    } else if lower.contains("already exists") || lower.contains("file exists") {
        "The output file already exists. Enable overwrite or choose another folder.".to_string()
    } else if lower.contains("could not find tag for codec")
        || lower.contains("codec not currently supported in container")
        || lower.contains("only supports")
    {
        "The selected codec cannot be stored in this output format. Pick a different codec or format.".to_string()
    } else if lower.contains("invalid data found when processing input") {
        "FFmpeg could not read the source file. It may be damaged or not a media file.".to_string()
    } else if lower.contains("invalid argument") {
        "FFmpeg rejected one of the selected settings. Check the output format and codec options."
            .to_string()
    } else if log.trim().is_empty() {
        "FFmpeg stopped unexpectedly without reporting an error.".to_string()
    } else {
        format!("FFmpeg failed:\n{}", log.trim())
    }
}

/// Keep at most `max_len` bytes of `log`; when it grows past that, drop the
/// oldest text so roughly `keep_len` bytes remain. Never splits a UTF-8 character.
pub fn truncate_log(log: &mut String, max_len: usize, keep_len: usize) {
    if log.len() <= max_len {
        return;
    }
    let mut cut = log.len().saturating_sub(keep_len);
    while !log.is_char_boundary(cut) {
        cut += 1;
    }
    log.drain(..cut);
}

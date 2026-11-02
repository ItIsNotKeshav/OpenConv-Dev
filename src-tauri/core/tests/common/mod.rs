#![allow(dead_code)]

use openconv_core::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh, empty directory under the system temp dir, removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "openconv-test-{}-{}-{}",
            label,
            std::process::id(),
            n
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn touch(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, b"x").expect("write file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A config with every option at its "keep source" default.
pub fn config(container: Container) -> ConversionConfig {
    ConversionConfig {
        input_path: "in.mp4".to_string(),
        output_path: None,
        output_dir: Some(".".to_string()),
        container,
        video_codec: None,
        audio_codec: None,
        resolution: Resolution::Source,
        frame_rate: FrameRate::Source,
        preset: Preset::Medium,
        crf: None,
        video_bitrate: None,
        audio_bitrate: None,
        channels: Channels::Source,
        sample_rate: SampleRate::Source,
        hardware_acceleration: HardwareAcceleration::Off,
        overwrite: false,
        extra_arguments: None,
    }
}

/// Build args for `config` with fixed input/output names.
pub fn args(config: &ConversionConfig) -> Vec<String> {
    build_ffmpeg_args(config, Path::new("in.mp4"), Path::new("out.file")).args
}

/// The value following `flag`, if the flag is present.
pub fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

pub fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

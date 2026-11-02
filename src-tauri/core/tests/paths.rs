mod common;

use common::*;
use openconv_core::*;
use std::path::PathBuf;

fn valid_config(dir: &TempDir) -> ConversionConfig {
    let input = dir.touch("clip.mov");
    let mut cfg = config(Container::Mp4);
    cfg.input_path = input.to_string_lossy().into_owned();
    cfg.output_dir = Some(dir.path().to_string_lossy().into_owned());
    cfg
}

#[test]
fn validate_accepts_a_good_config() {
    let dir = TempDir::new("validate-ok");
    assert_eq!(validate_config(&valid_config(&dir)), Ok(()));
}

#[test]
fn validate_rejects_missing_or_bad_input() {
    let dir = TempDir::new("validate-input");
    let mut cfg = valid_config(&dir);

    cfg.input_path = "   ".into();
    assert_eq!(
        validate_config(&cfg).unwrap_err(),
        "Choose a source file before starting."
    );

    cfg.input_path = dir.path().join("missing.mp4").to_string_lossy().into();
    assert_eq!(
        validate_config(&cfg).unwrap_err(),
        "The selected input file no longer exists."
    );

    cfg.input_path = dir.path().to_string_lossy().into();
    assert_eq!(
        validate_config(&cfg).unwrap_err(),
        "The selected input path is not a file."
    );
}

#[test]
fn validate_rejects_bad_settings() {
    let dir = TempDir::new("validate-settings");

    let mut cfg = valid_config(&dir);
    cfg.resolution = Resolution::Custom {
        width: 0,
        height: 720,
    };
    assert!(validate_config(&cfg)
        .unwrap_err()
        .contains("Custom resolution"));
    cfg.resolution = Resolution::Custom {
        width: 1280,
        height: 0,
    };
    assert!(validate_config(&cfg)
        .unwrap_err()
        .contains("Custom resolution"));

    let mut cfg = valid_config(&dir);
    cfg.crf = Some(52);
    assert_eq!(
        validate_config(&cfg).unwrap_err(),
        "CRF must be between 0 and 51."
    );
    cfg.crf = Some(51);
    assert!(validate_config(&cfg).is_ok());

    let mut cfg = valid_config(&dir);
    cfg.output_dir = Some("  ".into());
    cfg.output_path = None;
    assert_eq!(
        validate_config(&cfg).unwrap_err(),
        "Choose an output folder before starting."
    );
}

#[test]
fn output_goes_into_the_chosen_folder_with_the_container_extension() {
    let dir = TempDir::new("out-basic");
    let cfg = valid_config(&dir);
    let input = PathBuf::from(&cfg.input_path);
    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap(),
        dir.path().join("clip.mp4")
    );
}

#[test]
fn existing_outputs_are_not_overwritten_by_default() {
    let dir = TempDir::new("out-unique");
    let cfg = valid_config(&dir);
    let input = PathBuf::from(&cfg.input_path);
    dir.touch("clip.mp4");
    dir.touch("clip (1).mp4");
    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap(),
        dir.path().join("clip (2).mp4")
    );
}

#[test]
fn overwrite_reuses_the_existing_name() {
    let dir = TempDir::new("out-overwrite");
    let mut cfg = valid_config(&dir);
    cfg.overwrite = true;
    let input = PathBuf::from(&cfg.input_path);
    dir.touch("clip.mp4");
    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap(),
        dir.path().join("clip.mp4")
    );
}

#[test]
fn converting_into_the_same_format_never_overwrites_the_source() {
    let dir = TempDir::new("out-same");
    let mut cfg = valid_config(&dir);
    cfg.container = Container::Mov;
    let input = PathBuf::from(&cfg.input_path);

    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap(),
        dir.path().join("clip (1).mov")
    );

    cfg.overwrite = true;
    let err = resolve_output_path(&input, &cfg).unwrap_err();
    assert!(err.contains("overwrite the source"), "{err}");
}

#[test]
fn missing_output_folder_is_reported() {
    let dir = TempDir::new("out-missing");
    let mut cfg = valid_config(&dir);
    let input = PathBuf::from(&cfg.input_path);
    cfg.output_dir = Some(dir.path().join("nope").to_string_lossy().into());
    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap_err(),
        "The output folder does not exist."
    );
}

#[test]
fn explicit_output_path_wins_over_output_dir() {
    let dir = TempDir::new("out-explicit");
    let mut cfg = valid_config(&dir);
    let input = PathBuf::from(&cfg.input_path);
    let target = dir.path().join("custom name.mkv");
    cfg.output_path = Some(target.to_string_lossy().into());
    cfg.output_dir = Some("/definitely/not/here".into());
    assert_eq!(resolve_output_path(&input, &cfg).unwrap(), target);

    cfg.output_path = Some(
        dir.path()
            .join("missing")
            .join("x.mkv")
            .to_string_lossy()
            .into(),
    );
    assert_eq!(
        resolve_output_path(&input, &cfg).unwrap_err(),
        "The output folder does not exist."
    );
}

#[test]
fn next_available_path_handles_files_without_extension() {
    let dir = TempDir::new("next-noext");
    let p = dir.touch("README");
    assert_eq!(
        next_available_path(p).unwrap(),
        dir.path().join("README (1)")
    );
}

#[test]
fn container_from_extension_handles_case_dots_and_unknowns() {
    for c in Container::ALL {
        assert_eq!(container_from_extension(c.extension()), c);
        assert_eq!(
            container_from_extension(&format!(".{}", c.extension().to_uppercase())),
            c
        );
    }
    assert_eq!(container_from_extension("matroska"), Container::Matroska);
    assert_eq!(container_from_extension("wmv"), Container::Mp4);
    assert_eq!(container_from_extension(""), Container::Mp4);
}

#[test]
fn every_container_extension_is_unique() {
    let mut exts: Vec<_> = Container::ALL.iter().map(|c| c.extension()).collect();
    exts.sort();
    exts.dedup();
    assert_eq!(exts.len(), Container::ALL.len());
}

#[test]
fn safe_log_name_strips_path_and_reserved_characters() {
    assert_eq!(
        safe_log_name(r#"a/b\c:d*e?f"g<h>i|j.mp4"#),
        "a_b_c_d_e_f_g_h_i_j.mp4"
    );
    assert_eq!(safe_log_name("plain.mp4"), "plain.mp4");
}

#[test]
fn resolve_ffmpeg_path_falls_back_to_path_lookup() {
    let path = resolve_ffmpeg_path();
    let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
    assert!(name == "ffmpeg" || name == "ffmpeg.exe", "{path:?}");
}

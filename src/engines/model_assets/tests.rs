use super::*;
use bzip2::{Compression, write::BzEncoder};
use std::{
    fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
};
use tar::Builder;
use tempfile::tempdir;

#[test]
fn validates_required_parakeet_files() {
    let dir = tempdir().unwrap();
    for file in REQUIRED_PARAKEET_FILES {
        fs::write(dir.path().join(file), "x").unwrap();
    }

    validate_parakeet_model_dir(dir.path()).unwrap();
    fs::remove_file(dir.path().join("tokens.txt")).unwrap();
    assert!(validate_parakeet_model_dir(dir.path()).is_err());
}

#[test]
fn strips_archive_root_and_rejects_unsafe_paths() {
    assert_eq!(
        strip_archive_root(Path::new("root/encoder.int8.onnx")).unwrap(),
        PathBuf::from("encoder.int8.onnx")
    );
    assert!(strip_archive_root(Path::new("root/../bad")).is_err());
}

#[test]
fn extracts_archive_without_top_level_directory() {
    let source = make_test_archive(&[
        ("model/encoder.int8.onnx", "encoder"),
        ("model/decoder.int8.onnx", "decoder"),
        ("model/joiner.int8.onnx", "joiner"),
        ("model/tokens.txt", "tokens"),
    ]);
    let dir = tempdir().unwrap();
    let archive_path = dir.path().join("model.tar.bz2");
    fs::write(&archive_path, source).unwrap();
    let dest = dir.path().join("dest");
    fs::create_dir(&dest).unwrap();

    safe_extract_tar_bz2(&archive_path, &dest).unwrap();
    validate_parakeet_model_dir(&dest).unwrap();
    assert!(dest.join("encoder.int8.onnx").is_file());
    assert!(!dest.join("model").exists());
}

#[test]
fn cancel_marks_active_download_as_cancelling() {
    let id = PARAKEET_MODELS[0].id;
    reset_parakeet_download_for_test(id);

    set_parakeet_install_state_for_test(
        id,
        ParakeetInstallState::Downloading {
            downloaded_bytes: 1024,
            total_bytes: Some(2048),
        },
    );

    cancel_parakeet_download(id).unwrap();

    assert_eq!(
        parakeet_download_state_for_test(id),
        Some(ParakeetInstallState::Cancelling {
            downloaded_bytes: 1024,
            total_bytes: Some(2048),
        })
    );

    reset_parakeet_download_for_test(id);
}

#[test]
fn apple_speech_status_uses_reservation_as_installed_state() {
    let statuses = apple_speech_models_status();
    let installed = statuses
        .iter()
        .find(|status| status.definition.id == "speechanalyzer-en_US")
        .unwrap();
    let available = statuses
        .iter()
        .find(|status| status.definition.id == "speechanalyzer-fr_FR")
        .unwrap();

    assert_eq!(installed.state, AppleSpeechInstallState::Installed);
    assert_eq!(available.state, AppleSpeechInstallState::NotInstalled);
}

#[test]
fn apple_speech_unavailable_reason_is_cleared_by_refresh() {
    set_apple_speech_models_unavailable_reason(Some("permission denied".to_string()));
    assert_eq!(
        apple_speech_models_unavailable_reason(),
        Some("permission denied".to_string())
    );

    refresh_apple_model_assets();

    assert_eq!(apple_speech_models_unavailable_reason(), None);
}

#[test]
fn cancel_marks_active_apple_speech_download_as_cancelling() {
    let id = "speechanalyzer-fr_FR";
    reset_apple_speech_download_for_test(id);

    set_apple_speech_download_state_for_test(
        id,
        AppleSpeechInstallState::Downloading {
            progress: Some(0.5),
        },
    );

    cancel_apple_speech_model_download(id).unwrap();

    assert_eq!(
        apple_speech_download_state_for_test(id),
        Some(AppleSpeechInstallState::Cancelling)
    );

    reset_apple_speech_download_for_test(id);
}

#[test]
#[ignore = "requires macOS 26 Apple Speech assets; set GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID=speechanalyzer-<locale>"]
fn attempts_real_apple_speech_download_then_cancel() {
    let id = std::env::var("GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID")
        .expect("set GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID=speechanalyzer-<locale>");
    assert!(
        apple_speech_locale_id(&id).is_some(),
        "test model id must use speechanalyzer-<locale>"
    );

    reset_apple_speech_download_for_test(&id);

    start_apple_speech_model_download(&id).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(750));
    cancel_apple_speech_model_download(&id).unwrap();

    let state = apple_speech_download_state_for_test(&id);
    assert!(
        matches!(state, Some(AppleSpeechInstallState::Cancelling)) || state.is_none(),
        "expected cancellation to be requested or the OS-managed install to finish quickly; got {state:?}"
    );

    for _ in 0..20 {
        if !apple_speech_has_active_downloads() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    assert!(
        !matches!(
            apple_speech_download_state_for_test(&id),
            Some(AppleSpeechInstallState::Downloading { .. })
        ),
        "Apple Speech download remained active after best-effort cancellation"
    );

    reset_apple_speech_download_for_test(&id);
}

fn make_test_archive(files: &[(&str, &str)]) -> Vec<u8> {
    let mut tar_data = Vec::new();
    {
        let cursor = Cursor::new(&mut tar_data);
        let mut builder = Builder::new(cursor);
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_cksum();
            builder
                .append_data(&mut header, *path, contents.as_bytes())
                .unwrap();
        }
        builder.finish().unwrap();
    }

    let mut encoder = BzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&tar_data).unwrap();
    encoder.finish().unwrap()
}

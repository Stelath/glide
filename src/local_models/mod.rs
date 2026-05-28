mod apple;
mod parakeet;

#[allow(unused_imports)]
pub use apple::{
    APPLE_FOUNDATION_MODEL_ID, APPLE_SPEECH_MODEL_ID, APPLE_SPEECH_MODEL_PREFIX,
    AppleFoundationModelStatus, AppleSpeechInstallState, AppleSpeechModelDefinition,
    AppleSpeechModelStatus, apple_foundation_models_status, apple_speech_has_active_downloads,
    apple_speech_install_state, apple_speech_locale_id, apple_speech_model_id,
    apple_speech_models_status, apple_speech_models_unavailable_reason,
    cancel_apple_speech_model_download, first_available_apple_foundation_model,
    first_installed_apple_speech_model, is_legacy_apple_speech_model, refresh_apple_local_models,
    release_apple_speech_model, resolve_apple_foundation_model_id, resolve_apple_speech_model_id,
    start_apple_speech_model_download,
};
#[allow(unused_imports)]
pub use parakeet::{
    LocalModelInstallState, PARAKEET_MODELS, ParakeetModelDefinition, ParakeetModelStatus,
    cancel_parakeet_download, delete_parakeet_model, parakeet_definition, parakeet_install_state,
    parakeet_model_dir, parakeet_models_status, start_parakeet_download,
    validate_parakeet_model_dir,
};

#[cfg(test)]
use apple::{
    apple_speech_download_state, clear_apple_speech_download_cancellation,
    clear_apple_speech_download_child, clear_apple_speech_download_state,
    is_apple_speech_download_cancelled, set_apple_speech_download_state,
    set_apple_speech_models_unavailable_reason,
};
#[cfg(test)]
pub(crate) use parakeet::set_parakeet_install_state_for_test;
#[cfg(test)]
use parakeet::{
    REQUIRED_PARAKEET_FILES, clear_download_cancellation, clear_download_state, download_state,
    is_download_cancelled, safe_extract_tar_bz2, set_download_state, strip_archive_root,
};

#[cfg(test)]
mod tests {
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
        clear_download_state(id);
        clear_download_cancellation(id);

        set_download_state(
            id,
            LocalModelInstallState::Downloading {
                downloaded_bytes: 1024,
                total_bytes: Some(2048),
            },
        );

        cancel_parakeet_download(id).unwrap();

        assert_eq!(
            download_state(id),
            Some(LocalModelInstallState::Cancelling {
                downloaded_bytes: 1024,
                total_bytes: Some(2048),
            })
        );
        assert!(is_download_cancelled(id));

        clear_download_state(id);
        clear_download_cancellation(id);
    }

    #[test]
    fn apple_speech_model_ids_round_trip_locale() {
        let id = apple_speech_model_id("en_US");
        assert_eq!(id, "speechanalyzer-en_US");
        assert_eq!(apple_speech_locale_id(&id), Some("en_US"));
        assert!(apple_speech_locale_id(APPLE_SPEECH_MODEL_ID).is_none());
        assert!(is_legacy_apple_speech_model(APPLE_SPEECH_MODEL_ID));
    }

    #[test]
    fn legacy_apple_speech_model_resolves_to_first_installed_locale() {
        assert_eq!(
            resolve_apple_speech_model_id(APPLE_SPEECH_MODEL_ID),
            Some("speechanalyzer-en_US".to_string())
        );
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

        refresh_apple_local_models();

        assert_eq!(apple_speech_models_unavailable_reason(), None);
    }

    #[test]
    fn apple_foundation_models_report_default_model_only() {
        let statuses = apple_foundation_models_status();
        let model = statuses
            .iter()
            .find(|status| status.id == APPLE_FOUNDATION_MODEL_ID)
            .unwrap();

        assert_eq!(statuses.len(), 1);
        assert!(model.available);
        assert_eq!(
            resolve_apple_foundation_model_id(APPLE_FOUNDATION_MODEL_ID),
            Some(APPLE_FOUNDATION_MODEL_ID.to_string())
        );
        assert_eq!(
            resolve_apple_foundation_model_id("apple-foundation-rewrite"),
            None
        );
    }

    #[test]
    fn cancel_marks_active_apple_speech_download_as_cancelling() {
        let id = "speechanalyzer-fr_FR";
        clear_apple_speech_download_state(id);
        clear_apple_speech_download_cancellation(id);

        set_apple_speech_download_state(
            id,
            AppleSpeechInstallState::Downloading {
                progress: Some(0.5),
            },
        );

        cancel_apple_speech_model_download(id).unwrap();

        assert_eq!(
            apple_speech_download_state(id),
            Some(AppleSpeechInstallState::Cancelling)
        );
        assert!(is_apple_speech_download_cancelled(id));

        clear_apple_speech_download_state(id);
        clear_apple_speech_download_cancellation(id);
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

        clear_apple_speech_download_state(&id);
        clear_apple_speech_download_cancellation(&id);
        clear_apple_speech_download_child(&id);

        start_apple_speech_model_download(&id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(750));
        cancel_apple_speech_model_download(&id).unwrap();

        let state = apple_speech_download_state(&id);
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
                apple_speech_download_state(&id),
                Some(AppleSpeechInstallState::Downloading { .. })
            ),
            "Apple Speech download remained active after best-effort cancellation"
        );

        clear_apple_speech_download_state(&id);
        clear_apple_speech_download_cancellation(&id);
        clear_apple_speech_download_child(&id);
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
}

//! On-device model *asset management* — not an inference stage. Handles the
//! lifecycle of local model files (download, install, validate, status,
//! cancellation) for Apple Speech and Parakeet. The STT/LLM providers that run
//! locally depend on this to know whether the files are present and ready.

mod apple;
mod parakeet;
pub(crate) mod prewarm;

#[cfg(test)]
pub use apple::apple_speech_model_id;
#[allow(unused_imports)]
pub use apple::{
    APPLE_FOUNDATION_MODEL_ID, APPLE_SPEECH_MODEL_ID, APPLE_SPEECH_MODEL_PREFIX,
    AppleFoundationModelStatus, AppleSpeechInstallState, AppleSpeechModelDefinition,
    AppleSpeechModelStatus, apple_foundation_models_status, apple_speech_has_active_downloads,
    apple_speech_install_state, apple_speech_locale_id, apple_speech_models_status,
    apple_speech_models_unavailable_reason, cancel_apple_speech_model_download,
    first_available_apple_foundation_model, first_installed_apple_speech_model,
    is_legacy_apple_speech_model, refresh_apple_local_models, release_apple_speech_model,
    resolve_apple_foundation_model_id, resolve_apple_speech_model_id,
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
mod tests;

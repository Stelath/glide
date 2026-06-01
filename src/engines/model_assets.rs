//! On-device model asset management — not an inference stage. Handles catalog,
//! install, validation, status, cancellation, and release for Parakeet files and
//! Apple OS-managed model assets. The STT/LLM providers depend on this only to
//! know whether a model asset is present and ready.

mod apple;
mod lifecycle;
mod parakeet;

#[allow(unused_imports)]
pub use apple::{
    APPLE_FOUNDATION_MODEL_ID, APPLE_SPEECH_MODEL_PREFIX, AppleFoundationModelStatus,
    AppleSpeechInstallState, AppleSpeechModelDefinition, AppleSpeechModelStatus,
    apple_foundation_models_status, apple_speech_has_active_downloads, apple_speech_install_state,
    apple_speech_locale_id, apple_speech_models_status, apple_speech_models_unavailable_reason,
    cancel_apple_speech_model_download, first_available_apple_foundation_model,
    first_installed_apple_speech_model, refresh_apple_model_assets, release_apple_speech_model,
    resolve_apple_foundation_model_id, start_apple_speech_model_download,
};
#[allow(unused_imports)]
pub use parakeet::{
    PARAKEET_MODELS, ParakeetInstallState, ParakeetModelDefinition, ParakeetModelStatus,
    cancel_parakeet_download, delete_parakeet_model, parakeet_definition, parakeet_install_state,
    parakeet_model_dir, parakeet_models_status, start_parakeet_download,
    validate_parakeet_model_dir,
};

#[cfg(test)]
use apple::{
    apple_speech_download_state_for_test, reset_apple_speech_download_for_test,
    set_apple_speech_download_state_for_test, set_apple_speech_models_unavailable_reason,
};
#[cfg(test)]
pub(crate) use parakeet::set_parakeet_install_state_for_test;
#[cfg(test)]
use parakeet::{
    REQUIRED_PARAKEET_FILES, parakeet_download_state_for_test, reset_parakeet_download_for_test,
    safe_extract_tar_bz2, strip_archive_root,
};

#[cfg(test)]
mod tests;

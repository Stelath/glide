mod downloads;
mod foundation;
mod speech;
mod types;

pub use downloads::{
    apple_speech_has_active_downloads, cancel_apple_speech_model_download,
    release_apple_speech_model, start_apple_speech_model_download,
};
pub use foundation::{
    apple_foundation_models_status, first_available_apple_foundation_model,
    resolve_apple_foundation_model_id,
};
pub use speech::{
    apple_speech_install_state, apple_speech_locale_id, apple_speech_models_status,
    apple_speech_models_unavailable_reason, first_installed_apple_speech_model,
};
pub use types::{
    APPLE_FOUNDATION_MODEL_ID, APPLE_SPEECH_MODEL_PREFIX, AppleFoundationModelStatus,
    AppleSpeechInstallState, AppleSpeechModelDefinition, AppleSpeechModelStatus,
};

#[cfg(test)]
pub(in crate::engines::model_assets) use downloads::{
    apple_speech_download_state_for_test, reset_apple_speech_download_for_test,
    set_apple_speech_download_state_for_test,
};
#[cfg(test)]
pub(in crate::engines::model_assets) use speech::set_apple_speech_models_unavailable_reason;

pub fn refresh_apple_model_assets() {
    speech::invalidate_apple_speech_model_cache();
    foundation::invalidate_apple_foundation_model_cache();
    crate::engines::apple_bridge::invalidate_capabilities_cache();
}

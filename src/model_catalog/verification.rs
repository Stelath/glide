use std::sync::{Mutex, OnceLock};

use crate::{
    config::Provider,
    local_models::{self, LocalModelInstallState},
};

pub(super) static PROVIDER_VERIFIED: OnceLock<Mutex<[bool; 5]>> = OnceLock::new();

pub(super) fn set_remote_provider_verified(provider: Provider, verified: bool) {
    if let Some(index) = provider.remote_index() {
        let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 5]));
        cache.lock().unwrap()[index] = verified;
    }
}

pub(super) fn any_remote_provider_verified() -> bool {
    Provider::REMOTE.into_iter().any(provider_verified)
}

fn apple_speech_available() -> bool {
    #[cfg(test)]
    {
        local_models::first_installed_apple_speech_model().is_some()
    }
    #[cfg(not(test))]
    {
        local_models::first_installed_apple_speech_model().is_some()
    }
}

fn apple_foundation_available() -> bool {
    local_models::first_available_apple_foundation_model().is_some()
}

pub fn provider_verified(provider: Provider) -> bool {
    if let Some(index) = provider.remote_index() {
        let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 5]));
        return cache.lock().unwrap()[index];
    }

    match provider {
        Provider::AppleLocal => apple_speech_available() || apple_foundation_available(),
        Provider::Parakeet => local_models::parakeet_models_status()
            .iter()
            .any(|model| matches!(model.state, LocalModelInstallState::Installed { .. })),
        Provider::OpenAi
        | Provider::Groq
        | Provider::Cerebras
        | Provider::Fireworks
        | Provider::ElevenLabs => false,
    }
}

pub fn any_provider_verified() -> bool {
    Provider::ALL.into_iter().any(provider_verified)
}

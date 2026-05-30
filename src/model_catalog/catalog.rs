use std::sync::{Mutex, OnceLock};

use crate::{
    config::Provider,
    local_models::{self, LocalModelInstallState},
};

use super::{
    types::ModelInfo,
    verification::{any_remote_provider_verified, provider_verified},
};

pub(super) static CACHED_STT_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
pub(super) static CACHED_LLM_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();

pub(super) fn fallback_stt_models() -> Vec<ModelInfo> {
    let mut all = vec![
        model_info(Provider::OpenAi, "whisper-1", false),
        model_info(Provider::Groq, "whisper-large-v3", false),
        model_info(Provider::Groq, "whisper-large-v3-turbo", false),
        model_info(Provider::Fireworks, "whisper-v3-turbo", false),
        model_info(Provider::Fireworks, "whisper-v3", false),
        model_info_with_display(Provider::ElevenLabs, "scribe_v2", "Scribe v2", false),
        model_info_with_display(Provider::ElevenLabs, "scribe_v1", "Scribe v1", false),
    ];
    all.extend(apple_speech_model_infos());
    all.extend(
        local_models::parakeet_models_status()
            .into_iter()
            .filter_map(|status| {
                let installed = matches!(status.state, LocalModelInstallState::Installed { .. });
                installed.then(|| model_info(Provider::Parakeet, status.definition.id, true))
            }),
    );
    filter_models_by_verified_providers(all)
}

pub(super) fn fallback_llm_models() -> Vec<ModelInfo> {
    let all = vec![
        model_info(Provider::OpenAi, "gpt-5.4-nano", false),
        model_info(Provider::OpenAi, "gpt-4o-mini", false),
        model_info(Provider::OpenAi, "gpt-4o", false),
        model_info(Provider::OpenAi, "gpt-4-turbo", false),
        model_info(
            Provider::Groq,
            "meta-llama/llama-4-scout-17b-16e-instruct",
            false,
        ),
        model_info(Provider::Groq, "llama-3.3-70b-versatile", false),
        model_info(Provider::Groq, "llama-3.1-8b-instant", false),
        model_info(Provider::Groq, "mixtral-8x7b-32768", false),
        model_info(
            Provider::Fireworks,
            "accounts/fireworks/models/gpt-oss-20b",
            false,
        ),
        model_info(
            Provider::Fireworks,
            "accounts/fireworks/models/gpt-oss-120b",
            false,
        ),
        model_info(Provider::Cerebras, "gpt-oss-120b", false),
        model_info(Provider::Cerebras, "llama-4-scout-17b-16e-instruct", false),
    ];
    let mut all = all;
    all.extend(apple_foundation_model_infos());
    filter_models_by_verified_providers(all)
}

fn filter_models_by_verified_providers(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    if !any_remote_provider_verified()
        && !provider_verified(Provider::AppleLocal)
        && !provider_verified(Provider::Parakeet)
    {
        return models;
    }
    models
        .into_iter()
        .filter(|m| {
            Provider::from_model_info_provider(&m.provider)
                .map(|provider| {
                    if provider.is_local() {
                        m.installed && provider_verified(provider)
                    } else {
                        provider_verified(provider)
                    }
                })
                .unwrap_or(false)
        })
        .collect()
}

pub(super) fn model_info(provider: Provider, id: impl Into<String>, installed: bool) -> ModelInfo {
    let id = id.into();
    model_info_with_display(provider, id.clone(), id, installed)
}

pub(super) fn model_info_with_display(
    provider: Provider,
    id: impl Into<String>,
    display_name: impl Into<String>,
    installed: bool,
) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        display_name: display_name.into(),
        provider: provider.label().into(),
        logo: provider.logo().into(),
        installed,
    }
}

pub fn cached_stt_models() -> Vec<ModelInfo> {
    let cache = CACHED_STT_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_stt_models()
    } else {
        let mut models = locked.clone();
        models.extend(local_stt_models());
        filter_models_by_verified_providers(models)
    }
}

pub fn cached_llm_models() -> Vec<ModelInfo> {
    let cache = CACHED_LLM_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_llm_models()
    } else {
        let mut models = locked.clone();
        models.extend(local_llm_models());
        filter_models_by_verified_providers(models)
    }
}

fn local_stt_models() -> Vec<ModelInfo> {
    let mut models = Vec::new();
    models.extend(apple_speech_model_infos());
    models.extend(
        local_models::parakeet_models_status()
            .into_iter()
            .filter_map(|status| {
                matches!(status.state, LocalModelInstallState::Installed { .. })
                    .then(|| model_info(Provider::Parakeet, status.definition.id, true))
            }),
    );
    models
}

fn apple_speech_model_infos() -> Vec<ModelInfo> {
    local_models::apple_speech_models_status()
        .into_iter()
        .filter_map(|status| {
            (status.state == local_models::AppleSpeechInstallState::Installed).then(|| {
                model_info_with_display(
                    Provider::AppleLocal,
                    status.definition.id,
                    status.definition.display_name,
                    true,
                )
            })
        })
        .collect()
}

pub(super) fn local_llm_models() -> Vec<ModelInfo> {
    apple_foundation_model_infos()
}

fn apple_foundation_model_infos() -> Vec<ModelInfo> {
    local_models::apple_foundation_models_status()
        .into_iter()
        .filter_map(|model| {
            model.available.then(|| {
                model_info_with_display(Provider::AppleLocal, model.id, model.display_name, true)
            })
        })
        .collect()
}

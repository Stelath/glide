use crate::{
    config::{GlideConfig, ModelSelection, Provider},
    local_models::{self, LocalModelInstallState},
};

use super::verification::{any_provider_verified, provider_verified};

fn stt_selection_available(selection: &ModelSelection) -> bool {
    match selection.provider {
        Provider::OpenAi | Provider::Groq | Provider::Fireworks | Provider::ElevenLabs => {
            provider_verified(selection.provider)
        }
        Provider::Cerebras => false,
        Provider::AppleLocal => local_models::resolve_apple_speech_model_id(&selection.model)
            .map(|model_id| {
                local_models::apple_speech_install_state(&model_id)
                    == local_models::AppleSpeechInstallState::Installed
            })
            .unwrap_or(false),
        Provider::Parakeet => matches!(
            local_models::parakeet_install_state(&selection.model),
            LocalModelInstallState::Installed { .. }
        ),
    }
}

fn llm_selection_available(selection: &ModelSelection) -> bool {
    match selection.provider {
        Provider::OpenAi | Provider::Groq | Provider::Cerebras | Provider::Fireworks => {
            provider_verified(selection.provider)
        }
        Provider::ElevenLabs => false,
        Provider::AppleLocal => {
            local_models::resolve_apple_foundation_model_id(&selection.model).is_some()
        }
        Provider::Parakeet => false,
    }
}

pub fn smart_stt_default() -> Option<ModelSelection> {
    if provider_verified(Provider::Groq) {
        Some(ModelSelection {
            provider: Provider::Groq,
            model: "whisper-large-v3-turbo".to_string(),
        })
    } else if provider_verified(Provider::OpenAi) {
        Some(ModelSelection {
            provider: Provider::OpenAi,
            model: "whisper-1".to_string(),
        })
    } else if provider_verified(Provider::Fireworks) {
        Some(ModelSelection {
            provider: Provider::Fireworks,
            model: "whisper-v3-turbo".to_string(),
        })
    } else if provider_verified(Provider::ElevenLabs) {
        Some(ModelSelection {
            provider: Provider::ElevenLabs,
            model: "scribe_v2".to_string(),
        })
    } else if let Some(model) = local_models::parakeet_models_status()
        .into_iter()
        .find(|model| matches!(model.state, LocalModelInstallState::Installed { .. }))
    {
        Some(ModelSelection {
            provider: Provider::Parakeet,
            model: model.definition.id.to_string(),
        })
    } else if let Some(model) = local_models::first_installed_apple_speech_model() {
        Some(ModelSelection {
            provider: Provider::AppleLocal,
            model: model.definition.id,
        })
    } else {
        None
    }
}

pub fn smart_llm_default() -> Option<ModelSelection> {
    if provider_verified(Provider::Groq) {
        Some(ModelSelection {
            provider: Provider::Groq,
            model: "meta-llama/llama-4-scout-17b-16e-instruct".to_string(),
        })
    } else if provider_verified(Provider::OpenAi) {
        Some(ModelSelection {
            provider: Provider::OpenAi,
            model: "gpt-5.4-nano".to_string(),
        })
    } else if provider_verified(Provider::Fireworks) {
        Some(ModelSelection {
            provider: Provider::Fireworks,
            model: "accounts/fireworks/models/gpt-oss-20b".to_string(),
        })
    } else if provider_verified(Provider::Cerebras) {
        Some(ModelSelection {
            provider: Provider::Cerebras,
            model: "gpt-oss-120b".to_string(),
        })
    } else if let Some(model) = local_models::first_available_apple_foundation_model() {
        Some(ModelSelection {
            provider: Provider::AppleLocal,
            model: model.id,
        })
    } else {
        None
    }
}

pub fn apply_smart_defaults(config: &mut GlideConfig) {
    resolve_legacy_apple_speech_selections(config);

    if !any_provider_verified() {
        return;
    }

    if !stt_selection_available(&config.dictation.stt)
        && let Some(smart) = smart_stt_default()
    {
        config.dictation.stt = smart;
    }

    if let Some(ref llm) = config.dictation.llm
        && !llm_selection_available(llm)
    {
        config.dictation.llm = smart_llm_default();
    }
}

fn resolve_legacy_apple_speech_selections(config: &mut GlideConfig) {
    if config.dictation.stt.provider == Provider::AppleLocal
        && local_models::is_legacy_apple_speech_model(&config.dictation.stt.model)
        && let Some(model) = local_models::first_installed_apple_speech_model()
    {
        config.dictation.stt.model = model.definition.id;
    }

    for style in &mut config.dictation.styles {
        if let Some(stt) = &mut style.stt
            && stt.provider == Provider::AppleLocal
            && local_models::is_legacy_apple_speech_model(&stt.model)
            && let Some(model) = local_models::first_installed_apple_speech_model()
        {
            stt.model = model.definition.id;
        }
    }
}

/// Like `apply_smart_defaults` but also auto-enables LLM if currently disabled.
/// Full auto-enable only runs once; subsequent calls fall through to `apply_smart_defaults`.
pub fn apply_smart_defaults_initial(config: &mut GlideConfig) {
    if config.dictation.smart_defaults_applied {
        apply_smart_defaults(config);
        return;
    }

    apply_smart_defaults(config);

    if config.dictation.llm.is_none() {
        config.dictation.llm = smart_llm_default();
    }

    config.dictation.smart_defaults_applied = true;
}

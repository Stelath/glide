use std::sync::{Mutex, OnceLock};

use super::{
    downloads::apple_speech_transient_install_state,
    types::{
        APPLE_SPEECH_MODEL_PREFIX, AppleSpeechInstallState, AppleSpeechModelDefinition,
        AppleSpeechModelStatus,
    },
};

#[cfg(not(test))]
static APPLE_SPEECH_MODELS: OnceLock<Mutex<Option<Vec<AppleSpeechModelDefinition>>>> =
    OnceLock::new();
static APPLE_SPEECH_MODELS_UNAVAILABLE_REASON: OnceLock<Mutex<Option<String>>> = OnceLock::new();

#[cfg(test)]
fn apple_speech_model_id(locale_id: &str) -> String {
    format!("{APPLE_SPEECH_MODEL_PREFIX}{locale_id}")
}

pub fn apple_speech_locale_id(model_id: &str) -> Option<&str> {
    model_id
        .strip_prefix(APPLE_SPEECH_MODEL_PREFIX)
        .filter(|locale| !locale.trim().is_empty())
}

pub fn first_installed_apple_speech_model() -> Option<AppleSpeechModelStatus> {
    apple_speech_models_status()
        .into_iter()
        .find(|model| model.state == AppleSpeechInstallState::Installed)
}

pub fn apple_speech_models_status() -> Vec<AppleSpeechModelStatus> {
    apple_speech_model_definitions()
        .into_iter()
        .map(|definition| {
            let state = apple_speech_install_state_for_definition(&definition);
            AppleSpeechModelStatus { definition, state }
        })
        .collect()
}

pub fn apple_speech_models_unavailable_reason() -> Option<String> {
    APPLE_SPEECH_MODELS_UNAVAILABLE_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|reason| reason.clone())
}

pub fn apple_speech_install_state(id: &str) -> AppleSpeechInstallState {
    if let Some(state) = apple_speech_transient_install_state(id) {
        return state;
    }

    apple_speech_model_definition(id)
        .map(|definition| apple_speech_install_state_for_definition(&definition))
        .unwrap_or(AppleSpeechInstallState::NotInstalled)
}

pub(super) fn apple_speech_model_definition(id: &str) -> Option<AppleSpeechModelDefinition> {
    apple_speech_model_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
}

fn apple_speech_install_state_for_definition(
    definition: &AppleSpeechModelDefinition,
) -> AppleSpeechInstallState {
    if let Some(state) = apple_speech_transient_install_state(&definition.id) {
        return state;
    }

    if definition.reserved {
        AppleSpeechInstallState::Installed
    } else {
        AppleSpeechInstallState::NotInstalled
    }
}

#[cfg(test)]
pub(super) fn apple_speech_model_definitions() -> Vec<AppleSpeechModelDefinition> {
    let mut models = vec![
        AppleSpeechModelDefinition {
            id: apple_speech_model_id("en_US"),
            display_name: "English (United States)".to_string(),
            locale_id: "en_US".to_string(),
            asset_status: "installed".to_string(),
            reserved: true,
        },
        AppleSpeechModelDefinition {
            id: apple_speech_model_id("fr_FR"),
            display_name: "French (France)".to_string(),
            locale_id: "fr_FR".to_string(),
            asset_status: "supported".to_string(),
            reserved: false,
        },
    ];

    if let Some(model) = test_cancel_model_definition(&models) {
        models.push(model);
    }

    models
}

#[cfg(test)]
fn test_cancel_model_definition(
    existing: &[AppleSpeechModelDefinition],
) -> Option<AppleSpeechModelDefinition> {
    let id = std::env::var("GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID").ok()?;
    let locale_id = apple_speech_locale_id(&id)?.to_string();
    if existing.iter().any(|model| model.id == id) {
        return None;
    }

    Some(AppleSpeechModelDefinition {
        id,
        display_name: format!("Test locale {locale_id}"),
        locale_id,
        asset_status: "supported".to_string(),
        reserved: false,
    })
}

#[cfg(not(test))]
pub(super) fn apple_speech_model_definitions() -> Vec<AppleSpeechModelDefinition> {
    let cache = APPLE_SPEECH_MODELS.get_or_init(|| Mutex::new(None));
    if let Ok(mut cached) = cache.lock() {
        if let Some(models) = cached.clone() {
            return models;
        }

        let models = match crate::engines::apple_bridge::speech_models() {
            Ok(models) => models
                .into_iter()
                .map(|model| AppleSpeechModelDefinition {
                    id: model.id,
                    display_name: model.display_name,
                    locale_id: model.locale_id,
                    asset_status: model.status,
                    reserved: model.reserved,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                set_apple_speech_models_unavailable_reason(Some(error.to_string()));
                *cached = None;
                return Vec::new();
            }
        };

        if models.is_empty() {
            set_apple_speech_models_unavailable_reason(Some(
                "Apple Speech returned no supported locales".to_string(),
            ));
            *cached = None;
        } else {
            set_apple_speech_models_unavailable_reason(None);
            *cached = Some(models.clone());
        }
        models
    } else {
        Vec::new()
    }
}

#[cfg(test)]
pub(super) fn invalidate_apple_speech_model_cache() {
    set_apple_speech_models_unavailable_reason(None);
}

#[cfg(not(test))]
pub(super) fn invalidate_apple_speech_model_cache() {
    if let Ok(mut cache) = APPLE_SPEECH_MODELS.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
    set_apple_speech_models_unavailable_reason(None);
}

pub(in crate::engines::model_assets) fn set_apple_speech_models_unavailable_reason(
    reason: Option<String>,
) {
    if let Ok(mut locked) = APPLE_SPEECH_MODELS_UNAVAILABLE_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *locked = reason;
    }
}

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::GlideConfig;
use super::providers::{Provider, ProvidersConfig};
#[cfg(test)]
use crate::local_models::APPLE_FOUNDATION_MODEL_ID;
use crate::local_models::LocalModelInstallState;

const DEFAULT_PROMPT: &str = include_str!("prompts/default.md");
const PROFESSIONAL_PROMPT: &str = include_str!("prompts/professional.md");
const MESSAGING_PROMPT: &str = include_str!("prompts/messaging.md");
const CODING_PROMPT: &str = include_str!("prompts/coding.md");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: Provider,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DictationConfig {
    pub stt: ModelSelection,
    pub llm: Option<ModelSelection>,
    pub system_prompt: String,
    pub styles: Vec<Style>,
    #[serde(default)]
    pub smart_defaults_applied: bool,
}

impl Default for DictationConfig {
    fn default() -> Self {
        Self {
            stt: ModelSelection {
                provider: Provider::OpenAi,
                model: "whisper-1".to_string(),
            },
            llm: None,
            smart_defaults_applied: false,
            system_prompt: DEFAULT_PROMPT.trim_end().to_string(),
            styles: vec![
                Style {
                    name: "Professional".to_string(),
                    apps: vec![],
                    prompt: PROFESSIONAL_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Messaging".to_string(),
                    apps: vec![],
                    prompt: MESSAGING_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Coding".to_string(),
                    apps: vec![],
                    prompt: CODING_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    #[serde(default)]
    pub apps: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub stt: Option<ModelSelection>,
    #[serde(default)]
    pub llm: Option<ModelSelection>,
}

// ---------------------------------------------------------------------------
// Dictionary config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DictionaryConfig {
    pub vocabulary: Vec<String>,
    pub replacements: Vec<ReplacementRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementRule {
    pub find: String,
    pub replace: String,
    #[serde(default)]
    pub case_sensitive: bool,
}

// ---------------------------------------------------------------------------
// Model caching & fetching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub logo: String,
    pub local: bool,
    pub installed: bool,
}

static CACHED_STT_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
static CACHED_LLM_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
pub(crate) static PROVIDER_VERIFIED: OnceLock<Mutex<[bool; 2]>> = OnceLock::new();

fn provider_verified_index(provider: Provider) -> Option<usize> {
    match provider {
        Provider::OpenAi => Some(0),
        Provider::Groq => Some(1),
        Provider::AppleLocal | Provider::Parakeet => None,
    }
}

fn any_remote_provider_verified() -> bool {
    Provider::REMOTE.into_iter().any(provider_verified)
}

fn apple_speech_available() -> bool {
    #[cfg(test)]
    {
        crate::local_models::first_installed_apple_speech_model().is_some()
    }
    #[cfg(not(test))]
    {
        crate::local_models::first_installed_apple_speech_model().is_some()
    }
}

fn apple_foundation_available() -> bool {
    crate::local_models::first_available_apple_foundation_model().is_some()
}

pub fn provider_verified(provider: Provider) -> bool {
    let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
    let locked = cache.lock().unwrap();
    match provider {
        Provider::OpenAi => locked[0],
        Provider::Groq => locked[1],
        Provider::AppleLocal => apple_speech_available() || apple_foundation_available(),
        Provider::Parakeet => crate::local_models::parakeet_models_status()
            .iter()
            .any(|model| matches!(model.state, LocalModelInstallState::Installed { .. })),
    }
}

pub fn any_provider_verified() -> bool {
    Provider::ALL.into_iter().any(provider_verified)
}

fn stt_selection_available(selection: &ModelSelection) -> bool {
    match selection.provider {
        Provider::OpenAi | Provider::Groq => provider_verified(selection.provider),
        Provider::AppleLocal => {
            crate::local_models::resolve_apple_speech_model_id(&selection.model)
                .map(|model_id| {
                    crate::local_models::apple_speech_install_state(&model_id)
                        == crate::local_models::AppleSpeechInstallState::Installed
                })
                .unwrap_or(false)
        }
        Provider::Parakeet => matches!(
            crate::local_models::parakeet_install_state(&selection.model),
            LocalModelInstallState::Installed { .. }
        ),
    }
}

fn llm_selection_available(selection: &ModelSelection) -> bool {
    match selection.provider {
        Provider::OpenAi | Provider::Groq => provider_verified(selection.provider),
        Provider::AppleLocal => {
            crate::local_models::resolve_apple_foundation_model_id(&selection.model).is_some()
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
    } else if let Some(model) = crate::local_models::parakeet_models_status()
        .into_iter()
        .find(|model| matches!(model.state, LocalModelInstallState::Installed { .. }))
    {
        Some(ModelSelection {
            provider: Provider::Parakeet,
            model: model.definition.id.to_string(),
        })
    } else if let Some(model) = crate::local_models::first_installed_apple_speech_model() {
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
    } else if let Some(model) = crate::local_models::first_available_apple_foundation_model() {
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
        && crate::local_models::is_legacy_apple_speech_model(&config.dictation.stt.model)
        && let Some(model) = crate::local_models::first_installed_apple_speech_model()
    {
        config.dictation.stt.model = model.definition.id;
    }

    for style in &mut config.dictation.styles {
        if let Some(stt) = &mut style.stt
            && stt.provider == Provider::AppleLocal
            && crate::local_models::is_legacy_apple_speech_model(&stt.model)
            && let Some(model) = crate::local_models::first_installed_apple_speech_model()
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

fn fallback_stt_models() -> Vec<ModelInfo> {
    let mut all = vec![
        model_info(Provider::OpenAi, "whisper-1", false, false),
        model_info(Provider::Groq, "whisper-large-v3", false, false),
        model_info(Provider::Groq, "whisper-large-v3-turbo", false, false),
    ];
    all.extend(apple_speech_model_infos());
    all.extend(
        crate::local_models::parakeet_models_status()
            .into_iter()
            .filter_map(|status| {
                let installed = matches!(status.state, LocalModelInstallState::Installed { .. });
                installed.then(|| model_info(Provider::Parakeet, status.definition.id, true, true))
            }),
    );
    filter_models_by_verified_providers(all)
}

fn fallback_llm_models() -> Vec<ModelInfo> {
    let all = vec![
        model_info(Provider::OpenAi, "gpt-5.4-nano", false, false),
        model_info(Provider::OpenAi, "gpt-4o-mini", false, false),
        model_info(Provider::OpenAi, "gpt-4o", false, false),
        model_info(Provider::OpenAi, "gpt-4-turbo", false, false),
        model_info(
            Provider::Groq,
            "meta-llama/llama-4-scout-17b-16e-instruct",
            false,
            false,
        ),
        model_info(Provider::Groq, "llama-3.3-70b-versatile", false, false),
        model_info(Provider::Groq, "llama-3.1-8b-instant", false, false),
        model_info(Provider::Groq, "mixtral-8x7b-32768", false, false),
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

fn model_info(
    provider: Provider,
    id: impl Into<String>,
    local: bool,
    installed: bool,
) -> ModelInfo {
    let id = id.into();
    model_info_with_display(provider, id.clone(), id, local, installed)
}

fn model_info_with_display(
    provider: Provider,
    id: impl Into<String>,
    display_name: impl Into<String>,
    local: bool,
    installed: bool,
) -> ModelInfo {
    ModelInfo {
        id: id.into(),
        display_name: display_name.into(),
        provider: provider.label().into(),
        logo: provider.logo().into(),
        local,
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
        crate::local_models::parakeet_models_status()
            .into_iter()
            .filter_map(|status| {
                matches!(status.state, LocalModelInstallState::Installed { .. })
                    .then(|| model_info(Provider::Parakeet, status.definition.id, true, true))
            }),
    );
    models
}

fn apple_speech_model_infos() -> Vec<ModelInfo> {
    crate::local_models::apple_speech_models_status()
        .into_iter()
        .filter_map(|status| {
            (status.state == crate::local_models::AppleSpeechInstallState::Installed).then(|| {
                model_info_with_display(
                    Provider::AppleLocal,
                    status.definition.id,
                    status.definition.display_name,
                    true,
                    true,
                )
            })
        })
        .collect()
}

fn local_llm_models() -> Vec<ModelInfo> {
    apple_foundation_model_infos()
}

fn apple_foundation_model_infos() -> Vec<ModelInfo> {
    crate::local_models::apple_foundation_models_status()
        .into_iter()
        .filter_map(|model| {
            model.available.then(|| {
                model_info_with_display(
                    Provider::AppleLocal,
                    model.id,
                    model.display_name,
                    true,
                    true,
                )
            })
        })
        .collect()
}

fn excluded_remote_llm_model(provider: Provider, id_lower: &str) -> bool {
    let excluded_by_family = id_lower.contains("embedding")
        || id_lower.contains("tts")
        || id_lower.contains("dall-e")
        || id_lower.contains("moderation")
        || id_lower.starts_with("ft:")
        || id_lower.contains("realtime")
        || id_lower.contains("-audio-")
        || id_lower.contains("davinci")
        || id_lower.contains("babbage")
        || id_lower.contains("canary")
        || id_lower.contains("search")
        || id_lower.contains("similarity")
        || id_lower.starts_with("text-")
        || id_lower.starts_with("code-")
        || id_lower.contains("omni-")
        || id_lower.contains("orpheus");

    let excluded_openai_generation_model = provider == Provider::OpenAi
        && (matches!(id_lower, "sora-2" | "sora-2-pro")
            || id_lower.starts_with("gpt-image")
            || id_lower.starts_with("gpt-audio"));

    excluded_by_family || excluded_openai_generation_model
}

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelsResponseEntry>,
}

#[derive(serde::Deserialize)]
struct ModelsResponseEntry {
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    owned_by: String,
    #[serde(default)]
    active: Option<bool>,
}

pub fn fetch_all_models(providers: &ProvidersConfig) {
    let openai = providers.openai.clone();
    let groq = providers.groq.clone();

    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let mut stt = Vec::new();
        let mut llm = Vec::new();

        for (provider, creds) in [(Provider::OpenAi, &openai), (Provider::Groq, &groq)] {
            let verified_cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
            let idx = provider_verified_index(provider).expect("remote provider has an index");

            if creds.api_key.trim().is_empty() || creds.base_url.trim().is_empty() {
                verified_cache.lock().unwrap()[idx] = false;
                continue;
            }

            let url = format!("{}/models", creds.base_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .bearer_auth(&creds.api_key)
                .send()
                .and_then(|r| r.json::<ModelsResponse>());

            if let Ok(resp) = resp {
                verified_cache.lock().unwrap()[idx] = true;
                let logo = provider.logo().to_string();
                let label = provider.label().to_string();
                for entry in resp.data {
                    if entry.active == Some(false) {
                        continue;
                    }

                    let id_lower = entry.id.to_lowercase();

                    let is_stt =
                        id_lower.contains("whisper") || id_lower.contains("distil-whisper");

                    let info = ModelInfo {
                        id: entry.id.clone(),
                        display_name: entry.id,
                        provider: label.clone(),
                        logo: logo.clone(),
                        local: false,
                        installed: false,
                    };

                    if is_stt {
                        stt.push(info);
                    } else {
                        if !excluded_remote_llm_model(provider, &id_lower) {
                            llm.push(info);
                        }
                    }
                }
            } else {
                verified_cache.lock().unwrap()[idx] = false;
            }
        }

        stt.sort_by(|a, b| (&a.provider, &a.id).cmp(&(&b.provider, &b.id)));
        llm.sort_by(|a, b| (&a.provider, &a.id).cmp(&(&b.provider, &b.id)));

        if !stt.is_empty() {
            let cache = CACHED_STT_MODELS.get_or_init(|| Mutex::new(Vec::new()));
            *cache.lock().unwrap() = stt;
        }
        if !llm.is_empty() {
            let cache = CACHED_LLM_MODELS.get_or_init(|| Mutex::new(Vec::new()));
            *cache.lock().unwrap() = llm;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::super::providers::ProviderCredentials;
    use super::*;
    use std::sync::Mutex;

    static PROVIDER_LOCK: Mutex<()> = Mutex::new(());

    fn set_provider_verified(provider: Provider, verified: bool) {
        let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
        let mut locked = cache.lock().unwrap();
        match provider {
            Provider::OpenAi => locked[0] = verified,
            Provider::Groq => locked[1] = verified,
            Provider::AppleLocal | Provider::Parakeet => {}
        }
    }

    fn reset_providers_verified() {
        let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
        let mut locked = cache.lock().unwrap();
        *locked = [false; 2];
    }

    #[test]
    fn test_provider_variants() {
        assert_eq!(Provider::ALL.len(), 4);
        assert_eq!(Provider::OpenAi.label(), "OpenAI");
        assert_eq!(Provider::Groq.label(), "Groq");
        assert_eq!(Provider::AppleLocal.label(), "Apple Intelligence");
        assert_eq!(Provider::Parakeet.label(), "Parakeet");
        assert!(!Provider::OpenAi.default_base_url().is_empty());
        assert!(Provider::AppleLocal.default_base_url().is_empty());
    }

    #[test]
    fn test_resolve_api_key_from_credentials() {
        let mut creds = ProviderCredentials::default();
        creds.api_key = "direct-key".to_string();
        let resolved = creds.resolve_api_key("test").unwrap();
        assert_eq!(resolved, "direct-key");
    }

    #[test]
    fn test_resolve_api_key_fails_when_missing() {
        let creds = ProviderCredentials::default();
        assert!(creds.resolve_api_key("test").is_err());
    }

    #[test]
    fn test_any_provider_verified_none() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        assert!(any_provider_verified());
    }

    #[test]
    fn test_any_provider_verified_openai() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        assert!(any_provider_verified());
        reset_providers_verified();
    }

    #[test]
    fn test_any_provider_verified_groq() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        assert!(any_provider_verified());
        reset_providers_verified();
    }

    #[test]
    fn test_smart_stt_default_no_providers() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let sel = smart_stt_default().unwrap();
        assert_eq!(sel.provider, Provider::AppleLocal);
        assert_eq!(sel.model, "speechanalyzer-en_US");
    }

    #[test]
    fn test_smart_stt_default_openai_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let sel = smart_stt_default().unwrap();
        assert_eq!(sel.provider, Provider::OpenAi);
        assert_eq!(sel.model, "whisper-1");
        reset_providers_verified();
    }

    #[test]
    fn test_smart_stt_default_groq_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let sel = smart_stt_default().unwrap();
        assert_eq!(sel.provider, Provider::Groq);
        assert_eq!(sel.model, "whisper-large-v3-turbo");
        reset_providers_verified();
    }

    #[test]
    fn test_smart_stt_default_both_prefers_groq() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        set_provider_verified(Provider::Groq, true);
        let sel = smart_stt_default().unwrap();
        assert_eq!(sel.provider, Provider::Groq);
        assert_eq!(sel.model, "whisper-large-v3-turbo");
        reset_providers_verified();
    }

    #[test]
    fn test_smart_llm_default_no_providers() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let sel = smart_llm_default().unwrap();
        assert_eq!(sel.provider, Provider::AppleLocal);
        assert_eq!(sel.model, APPLE_FOUNDATION_MODEL_ID);
    }

    #[test]
    fn test_smart_llm_default_openai_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let sel = smart_llm_default().unwrap();
        assert_eq!(sel.provider, Provider::OpenAi);
        assert_eq!(sel.model, "gpt-5.4-nano");
        reset_providers_verified();
    }

    #[test]
    fn test_smart_llm_default_groq_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let sel = smart_llm_default().unwrap();
        assert_eq!(sel.provider, Provider::Groq);
        assert_eq!(sel.model, "meta-llama/llama-4-scout-17b-16e-instruct");
        reset_providers_verified();
    }

    #[test]
    fn test_smart_llm_default_both_prefers_groq() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        set_provider_verified(Provider::Groq, true);
        let sel = smart_llm_default().unwrap();
        assert_eq!(sel.provider, Provider::Groq);
        assert_eq!(sel.model, "meta-llama/llama-4-scout-17b-16e-instruct");
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_no_providers() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let mut config = GlideConfig::default();
        apply_smart_defaults(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::AppleLocal);
        assert_eq!(config.dictation.stt.model, "speechanalyzer-en_US");
        assert!(config.dictation.llm.is_none());
    }

    #[test]
    fn test_apply_smart_defaults_groq_verified_fixes_stt_but_does_not_enable_llm() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let mut config = GlideConfig::default();
        apply_smart_defaults(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::Groq);
        assert_eq!(config.dictation.stt.model, "whisper-large-v3-turbo");
        assert!(config.dictation.llm.is_none());
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_initial_enables_llm() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let mut config = GlideConfig::default();
        apply_smart_defaults_initial(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::Groq);
        assert_eq!(config.dictation.stt.model, "whisper-large-v3-turbo");
        let llm = config.dictation.llm.as_ref().unwrap();
        assert_eq!(llm.provider, Provider::Groq);
        assert_eq!(llm.model, "meta-llama/llama-4-scout-17b-16e-instruct");
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_initial_does_not_re_enable_llm_after_flag_set() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let mut config = GlideConfig::default();
        apply_smart_defaults_initial(&mut config);
        assert!(config.dictation.llm.is_some());
        assert!(config.dictation.smart_defaults_applied);
        config.dictation.llm = None;
        apply_smart_defaults_initial(&mut config);
        assert!(config.dictation.llm.is_none());
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_openai_verified_keeps_stt() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let mut config = GlideConfig::default();
        apply_smart_defaults(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::OpenAi);
        assert_eq!(config.dictation.stt.model, "whisper-1");
        assert!(config.dictation.llm.is_none());
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_initial_openai_enables_llm() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let mut config = GlideConfig::default();
        apply_smart_defaults_initial(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::OpenAi);
        assert_eq!(config.dictation.stt.model, "whisper-1");
        let llm = config.dictation.llm.as_ref().unwrap();
        assert_eq!(llm.provider, Provider::OpenAi);
        assert_eq!(llm.model, "gpt-5.4-nano");
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_fixes_unverified_llm_provider() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let mut config = GlideConfig::default();
        config.dictation.llm = Some(ModelSelection {
            provider: Provider::OpenAi,
            model: "gpt-4o".to_string(),
        });
        apply_smart_defaults(&mut config);
        let llm = config.dictation.llm.as_ref().unwrap();
        assert_eq!(llm.provider, Provider::Groq);
        assert_eq!(llm.model, "meta-llama/llama-4-scout-17b-16e-instruct");
        reset_providers_verified();
    }

    #[test]
    fn test_apply_smart_defaults_preserves_verified_selections() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        set_provider_verified(Provider::Groq, true);
        let mut config = GlideConfig::default();
        config.dictation.stt = ModelSelection {
            provider: Provider::OpenAi,
            model: "whisper-1".to_string(),
        };
        config.dictation.llm = Some(ModelSelection {
            provider: Provider::OpenAi,
            model: "gpt-4o".to_string(),
        });
        apply_smart_defaults(&mut config);
        assert_eq!(config.dictation.stt.provider, Provider::OpenAi);
        assert_eq!(config.dictation.stt.model, "whisper-1");
        let llm = config.dictation.llm.as_ref().unwrap();
        assert_eq!(llm.provider, Provider::OpenAi);
        assert_eq!(llm.model, "gpt-4o");
        reset_providers_verified();
    }

    #[test]
    fn test_fallback_stt_models_no_providers_returns_all() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let models = fallback_stt_models();
        assert!(models.iter().all(|m| m.provider == "Apple Intelligence"));
        assert!(models.iter().any(|m| m.id == "speechanalyzer-en_US"));
    }

    #[test]
    fn test_fallback_stt_models_groq_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let models = fallback_stt_models();
        assert!(models.iter().any(|m| m.provider == "Groq"));
        assert!(models.iter().any(|m| m.provider == "Apple Intelligence"));
        assert!(models.iter().any(|m| m.id == "whisper-large-v3-turbo"));
        reset_providers_verified();
    }

    #[test]
    fn test_fallback_llm_models_openai_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let models = fallback_llm_models();
        assert!(models.iter().any(|m| m.provider == "OpenAI"));
        assert!(models.iter().any(|m| m.provider == "Apple Intelligence"));
        assert!(models.iter().any(|m| m.id == "gpt-5.4-nano"));
        assert!(models.iter().any(|m| m.id == APPLE_FOUNDATION_MODEL_ID));
        reset_providers_verified();
    }

    #[test]
    fn test_openai_generation_models_are_excluded_from_llm_picker() {
        for id in [
            "sora-2",
            "sora-2-pro",
            "gpt-image-1",
            "gpt-image-1-mini",
            "gpt-audio",
            "gpt-audio-mini",
        ] {
            assert!(excluded_remote_llm_model(Provider::OpenAi, id));
        }

        assert!(!excluded_remote_llm_model(Provider::OpenAi, "gpt-5.4-nano"));
        assert!(!excluded_remote_llm_model(Provider::Groq, "sora-2"));
    }

    #[test]
    fn test_removed_apple_foundation_selection_falls_back_to_default() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let mut config = GlideConfig::default();
        config.dictation.llm = Some(ModelSelection {
            provider: Provider::AppleLocal,
            model: "apple-foundation-rewrite".to_string(),
        });

        apply_smart_defaults(&mut config);

        let llm = config.dictation.llm.as_ref().unwrap();
        assert_eq!(llm.provider, Provider::AppleLocal);
        assert_eq!(llm.model, APPLE_FOUNDATION_MODEL_ID);
    }

    #[test]
    fn test_only_default_apple_foundation_model_is_listed() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        let models = local_llm_models();
        assert!(models.iter().any(|m| m.id == APPLE_FOUNDATION_MODEL_ID));
        assert!(!models.iter().any(|m| m.id == "apple-foundation-rewrite"));
        assert!(!models.iter().any(|m| m.id == "apple-foundation-summary"));
    }
}

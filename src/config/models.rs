use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use super::GlideConfig;
use super::providers::{Provider, ProvidersConfig};

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
            system_prompt: "You are a dictation post-processor. You receive raw speech-to-text output and return clean text ready to be typed into an application.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning.\n- Fix spelling, grammar, and punctuation errors.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context or custom vocabulary, correct the spelling. Never insert names or terms from context that the speaker did not say.\n- Preserve the speaker's intent, tone, and meaning exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription. The context is only for correcting spelling of words already spoken.\n- Do not change the meaning of what was said.".to_string(),
            styles: vec![
                Style {
                    name: "Professional".to_string(),
                    apps: vec![],
                    prompt: "You are a dictation post-processor for professional communication. You receive raw speech-to-text output and return clean, formal text ready to be typed into a work application.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning.\n- Fix spelling, grammar, and punctuation errors.\n- Elevate the language to a professional, clear, and well-structured tone.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.\n- Preserve the speaker's intent and meaning exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription.\n- Do not change the meaning of what was said.".to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Messaging".to_string(),
                    apps: vec![],
                    prompt: "You are a dictation post-processor for casual messaging. You receive raw speech-to-text output and return clean, conversational text ready to be sent in a chat or text message.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning or add personality.\n- Fix obvious spelling and grammar errors, but keep the tone informal and natural.\n- Use casual punctuation\u{2014}lowercase is fine, fragments are OK.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.\n- Preserve the speaker's voice and conversational style exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription.\n- Do not change the meaning of what was said.".to_string(),
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
// Model caching & fetching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub logo: String,
}

static CACHED_STT_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
static CACHED_LLM_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
pub(crate) static PROVIDER_VERIFIED: OnceLock<Mutex<[bool; 2]>> = OnceLock::new();

pub fn provider_verified(provider: Provider) -> bool {
    let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
    let locked = cache.lock().unwrap();
    match provider {
        Provider::OpenAi => locked[0],
        Provider::Groq => locked[1],
    }
}

pub fn any_provider_verified() -> bool {
    provider_verified(Provider::OpenAi) || provider_verified(Provider::Groq)
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
    } else {
        None
    }
}

pub fn apply_smart_defaults(config: &mut GlideConfig) {
    if !any_provider_verified() {
        return;
    }

    if !provider_verified(config.dictation.stt.provider) {
        if let Some(smart) = smart_stt_default() {
            config.dictation.stt = smart;
        }
    }

    if let Some(ref llm) = config.dictation.llm {
        if !provider_verified(llm.provider) {
            config.dictation.llm = smart_llm_default();
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
    let all = vec![
        ModelInfo {
            id: "whisper-1".into(),
            provider: "OpenAI".into(),
            logo: "assets/icons/openai.png".into(),
        },
        ModelInfo {
            id: "whisper-large-v3".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
        ModelInfo {
            id: "whisper-large-v3-turbo".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
    ];
    filter_models_by_verified_providers(all)
}

fn fallback_llm_models() -> Vec<ModelInfo> {
    let all = vec![
        ModelInfo {
            id: "gpt-5.4-nano".into(),
            provider: "OpenAI".into(),
            logo: "assets/icons/openai.png".into(),
        },
        ModelInfo {
            id: "gpt-4o-mini".into(),
            provider: "OpenAI".into(),
            logo: "assets/icons/openai.png".into(),
        },
        ModelInfo {
            id: "gpt-4o".into(),
            provider: "OpenAI".into(),
            logo: "assets/icons/openai.png".into(),
        },
        ModelInfo {
            id: "gpt-4-turbo".into(),
            provider: "OpenAI".into(),
            logo: "assets/icons/openai.png".into(),
        },
        ModelInfo {
            id: "meta-llama/llama-4-scout-17b-16e-instruct".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
        ModelInfo {
            id: "llama-3.3-70b-versatile".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
        ModelInfo {
            id: "llama-3.1-8b-instant".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
        ModelInfo {
            id: "mixtral-8x7b-32768".into(),
            provider: "Groq".into(),
            logo: "assets/icons/groq.png".into(),
        },
    ];
    filter_models_by_verified_providers(all)
}

fn filter_models_by_verified_providers(models: Vec<ModelInfo>) -> Vec<ModelInfo> {
    if !any_provider_verified() {
        return models;
    }
    models
        .into_iter()
        .filter(|m| {
            Provider::from_model_info_provider(&m.provider)
                .map(provider_verified)
                .unwrap_or(false)
        })
        .collect()
}

pub fn cached_stt_models() -> Vec<ModelInfo> {
    let cache = CACHED_STT_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_stt_models()
    } else {
        locked.clone()
    }
}

pub fn cached_llm_models() -> Vec<ModelInfo> {
    let cache = CACHED_LLM_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_llm_models()
    } else {
        locked.clone()
    }
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
            let idx = match provider {
                Provider::OpenAi => 0,
                Provider::Groq => 1,
            };

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
                        id: entry.id,
                        provider: label.clone(),
                        logo: logo.clone(),
                    };

                    if is_stt {
                        stt.push(info);
                    } else {
                        let excluded = id_lower.contains("embedding")
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
                        if !excluded {
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
        }
    }

    fn reset_providers_verified() {
        let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
        let mut locked = cache.lock().unwrap();
        *locked = [false; 2];
    }

    #[test]
    fn test_provider_variants() {
        assert_eq!(Provider::ALL.len(), 2);
        assert_eq!(Provider::OpenAi.label(), "OpenAI");
        assert_eq!(Provider::Groq.label(), "Groq");
        assert!(!Provider::OpenAi.default_base_url().is_empty());
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
        assert!(!any_provider_verified());
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
        assert!(smart_stt_default().is_none());
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
        assert!(smart_llm_default().is_none());
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
        assert_eq!(config.dictation.stt.provider, Provider::OpenAi);
        assert_eq!(config.dictation.stt.model, "whisper-1");
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
        assert_eq!(models.len(), 3);
        assert!(models.iter().any(|m| m.provider == "OpenAI"));
        assert!(models.iter().any(|m| m.provider == "Groq"));
    }

    #[test]
    fn test_fallback_stt_models_groq_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::Groq, true);
        let models = fallback_stt_models();
        assert!(models.iter().all(|m| m.provider == "Groq"));
        assert!(models.iter().any(|m| m.id == "whisper-large-v3-turbo"));
        reset_providers_verified();
    }

    #[test]
    fn test_fallback_llm_models_openai_only() {
        let _g = PROVIDER_LOCK.lock().unwrap();
        reset_providers_verified();
        set_provider_verified(Provider::OpenAi, true);
        let models = fallback_llm_models();
        assert!(models.iter().all(|m| m.provider == "OpenAI"));
        assert!(models.iter().any(|m| m.id == "gpt-5.4-nano"));
        reset_providers_verified();
    }
}

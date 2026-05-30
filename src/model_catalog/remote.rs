use std::{sync::Mutex, time::Duration};

use crate::config::{Provider, ProvidersConfig};

use super::{
    catalog::{CACHED_LLM_MODELS, CACHED_STT_MODELS, model_info, model_info_with_display},
    types::ModelInfo,
    verification::set_remote_provider_verified,
};

pub(super) fn excluded_remote_llm_model(provider: Provider, id_lower: &str) -> bool {
    let excluded_by_family = id_lower.contains("embedding")
        || id_lower.contains("embed")
        || id_lower.contains("rerank")
        || id_lower.contains("tts")
        || id_lower.contains("dall-e")
        || id_lower.contains("flux")
        || id_lower.contains("stable-diffusion")
        || id_lower.contains("sdxl")
        || id_lower.contains("image")
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
    active: Option<bool>,
}

#[derive(serde::Deserialize)]
pub(super) struct ElevenLabsModelsResponseEntry {
    pub(super) model_id: String,
    #[serde(default)]
    pub(super) name: Option<String>,
}

pub(super) fn append_elevenlabs_scribe_models(
    stt: &mut Vec<ModelInfo>,
    entries: Vec<ElevenLabsModelsResponseEntry>,
) {
    let mut saw_scribe_v2 = false;
    let mut saw_scribe_v1 = false;

    for entry in entries {
        if !matches!(entry.model_id.as_str(), "scribe_v2" | "scribe_v1") {
            continue;
        }

        saw_scribe_v2 |= entry.model_id == "scribe_v2";
        saw_scribe_v1 |= entry.model_id == "scribe_v1";
        let display_name = entry.name.unwrap_or_else(|| {
            elevenlabs_scribe_display_name(&entry.model_id)
                .unwrap_or("ElevenLabs Scribe")
                .to_string()
        });
        stt.push(model_info_with_display(
            Provider::ElevenLabs,
            entry.model_id,
            display_name,
            false,
        ));
    }

    if !saw_scribe_v2 {
        stt.push(model_info_with_display(
            Provider::ElevenLabs,
            "scribe_v2",
            "Scribe v2",
            false,
        ));
    }
    if !saw_scribe_v1 {
        stt.push(model_info_with_display(
            Provider::ElevenLabs,
            "scribe_v1",
            "Scribe v1",
            false,
        ));
    }
}

fn elevenlabs_scribe_display_name(model_id: &str) -> Option<&'static str> {
    match model_id {
        "scribe_v2" => Some("Scribe v2"),
        "scribe_v1" => Some("Scribe v1"),
        _ => None,
    }
}

pub fn fetch_all_models(providers: &ProvidersConfig) {
    let remote_credentials = providers
        .remote_credentials()
        .map(|(provider, credentials)| (provider, credentials.clone()))
        .collect::<Vec<_>>();

    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::blocking::Client::new());
        let mut stt = Vec::new();
        let mut llm = Vec::new();

        for (provider, creds) in remote_credentials
            .iter()
            .filter(|(provider, _)| *provider != Provider::ElevenLabs)
        {
            let provider = *provider;

            if creds.api_key.trim().is_empty() || creds.base_url.trim().is_empty() {
                set_remote_provider_verified(provider, false);
                continue;
            }

            let url = format!("{}/models", creds.base_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .bearer_auth(&creds.api_key)
                .send()
                .and_then(|r| r.json::<ModelsResponse>());

            if let Ok(resp) = resp {
                set_remote_provider_verified(provider, true);
                let logo = provider.logo().to_string();
                let label = provider.label().to_string();
                let mut saw_fireworks_whisper_v3 = false;
                let mut saw_fireworks_whisper_turbo = false;
                let mut saw_fireworks_gpt_oss_20b = false;
                let mut saw_fireworks_gpt_oss_120b = false;
                for entry in resp.data {
                    if entry.active == Some(false) {
                        continue;
                    }

                    let id_lower = entry.id.to_lowercase();

                    let is_stt =
                        id_lower.contains("whisper") || id_lower.contains("distil-whisper");
                    if provider == Provider::Fireworks {
                        saw_fireworks_whisper_v3 |= entry.id == "whisper-v3";
                        saw_fireworks_whisper_turbo |= entry.id == "whisper-v3-turbo";
                        saw_fireworks_gpt_oss_20b |=
                            entry.id.ends_with("/gpt-oss-20b") || entry.id == "gpt-oss-20b";
                        saw_fireworks_gpt_oss_120b |=
                            entry.id.ends_with("/gpt-oss-120b") || entry.id == "gpt-oss-120b";
                    }

                    let info = ModelInfo {
                        id: entry.id.clone(),
                        display_name: entry.id,
                        provider: label.clone(),
                        logo: logo.clone(),
                        installed: false,
                    };

                    if is_stt {
                        if provider != Provider::Cerebras {
                            stt.push(info);
                        }
                    } else if !excluded_remote_llm_model(provider, &id_lower) {
                        llm.push(info);
                    }
                }
                if provider == Provider::Fireworks {
                    if !saw_fireworks_whisper_turbo {
                        stt.push(model_info(Provider::Fireworks, "whisper-v3-turbo", false));
                    }
                    if !saw_fireworks_whisper_v3 {
                        stt.push(model_info(Provider::Fireworks, "whisper-v3", false));
                    }
                    if !saw_fireworks_gpt_oss_20b {
                        llm.push(model_info(
                            Provider::Fireworks,
                            "accounts/fireworks/models/gpt-oss-20b",
                            false,
                        ));
                    }
                    if !saw_fireworks_gpt_oss_120b {
                        llm.push(model_info(
                            Provider::Fireworks,
                            "accounts/fireworks/models/gpt-oss-120b",
                            false,
                        ));
                    }
                }
            } else {
                set_remote_provider_verified(provider, false);
            }
        }

        if let Some((_, elevenlabs)) = remote_credentials
            .iter()
            .find(|(provider, _)| *provider == Provider::ElevenLabs)
        {
            let api_key = elevenlabs.api_key.trim();
            if api_key.is_empty() || elevenlabs.base_url.trim().is_empty() {
                set_remote_provider_verified(Provider::ElevenLabs, false);
            } else {
                let base_url = elevenlabs.base_url.trim_end_matches('/');
                let models_url = format!("{base_url}/models");
                let models_response = client
                    .get(&models_url)
                    .header("xi-api-key", api_key)
                    .header(reqwest::header::ACCEPT, "application/json")
                    .send()
                    .and_then(|r| r.error_for_status());

                match models_response {
                    Ok(response) => {
                        set_remote_provider_verified(Provider::ElevenLabs, true);
                        let discovered = response
                            .json::<Vec<ElevenLabsModelsResponseEntry>>()
                            .unwrap_or_else(|error| {
                                eprintln!(
                                    "[glide] ElevenLabs: failed to parse model list from {models_url}: {error:#}"
                                );
                                Vec::new()
                            });
                        append_elevenlabs_scribe_models(&mut stt, discovered);
                    }
                    Err(models_error) => {
                        let user_url = format!("{base_url}/user");
                        let user_verified = client
                            .get(&user_url)
                            .header("xi-api-key", api_key)
                            .header(reqwest::header::ACCEPT, "application/json")
                            .send()
                            .and_then(|r| r.error_for_status())
                            .is_ok();

                        set_remote_provider_verified(Provider::ElevenLabs, user_verified);
                        if user_verified {
                            append_elevenlabs_scribe_models(&mut stt, Vec::new());
                        } else {
                            eprintln!(
                                "[glide] ElevenLabs: failed to verify API key via {models_url}: {models_error:#}"
                            );
                        }
                    }
                }
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

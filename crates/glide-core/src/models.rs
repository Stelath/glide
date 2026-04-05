use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct FetchModelsConfig {
    pub api_key: String,
    pub base_url: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ModelsResult {
    pub stt: Vec<String>,
    pub llm: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelsResponseEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelsResponseEntry {
    id: String,
    #[serde(default)]
    active: Option<bool>,
}

pub async fn fetch_models(config: &FetchModelsConfig) -> Result<ModelsResult> {
    let url = format!("{}/models", config.base_url.trim_end_matches('/'));
    let response = Client::new()
        .get(&url)
        .bearer_auth(&config.api_key)
        .send()
        .await
        .context("failed to fetch models")?
        .error_for_status()
        .context("models API returned an error status")?;

    let parsed: ModelsResponse = response
        .json()
        .await
        .context("failed to parse models response")?;

    Ok(classify_models(parsed.data))
}

fn classify_models(entries: Vec<ModelsResponseEntry>) -> ModelsResult {
    let mut stt = Vec::new();
    let mut llm = Vec::new();

    for entry in entries {
        if entry.active == Some(false) {
            continue;
        }

        let id_lower = entry.id.to_lowercase();
        if id_lower.contains("whisper") || id_lower.contains("distil-whisper") {
            stt.push(entry.id);
            continue;
        }

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
            llm.push(entry.id);
        }
    }

    stt.sort();
    llm.sort();

    ModelsResult { stt, llm }
}

#[cfg(test)]
mod tests {
    use super::{ModelsResponseEntry, ModelsResult, classify_models};

    #[test]
    fn classifies_and_filters_models() {
        let result = classify_models(vec![
            ModelsResponseEntry {
                id: "gpt-4o-mini".to_string(),
                active: Some(true),
            },
            ModelsResponseEntry {
                id: "whisper-1".to_string(),
                active: Some(true),
            },
            ModelsResponseEntry {
                id: "text-embedding-3-small".to_string(),
                active: Some(true),
            },
            ModelsResponseEntry {
                id: "llama-3.3-70b-versatile".to_string(),
                active: None,
            },
            ModelsResponseEntry {
                id: "gpt-realtime".to_string(),
                active: Some(true),
            },
            ModelsResponseEntry {
                id: "inactive-whisper".to_string(),
                active: Some(false),
            },
        ]);

        assert_eq!(
            result,
            ModelsResult {
                stt: vec!["whisper-1".to_string()],
                llm: vec![
                    "gpt-4o-mini".to_string(),
                    "llama-3.3-70b-versatile".to_string()
                ],
            }
        );
    }
}

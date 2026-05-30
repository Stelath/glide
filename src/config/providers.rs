use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    Groq,
    Cerebras,
    Fireworks,
    ElevenLabs,
    AppleLocal,
    Parakeet,
}

impl Provider {
    pub const ALL: [Self; 7] = [
        Self::OpenAi,
        Self::Groq,
        Self::Cerebras,
        Self::Fireworks,
        Self::ElevenLabs,
        Self::AppleLocal,
        Self::Parakeet,
    ];
    pub const REMOTE: [Self; 5] = [
        Self::OpenAi,
        Self::Groq,
        Self::Cerebras,
        Self::Fireworks,
        Self::ElevenLabs,
    ];
    pub const SETTINGS_REMOTE: [Self; 5] = [
        Self::OpenAi,
        Self::Groq,
        Self::Fireworks,
        Self::ElevenLabs,
        Self::Cerebras,
    ];

    pub fn key_id(self) -> Option<&'static str> {
        match self {
            Self::OpenAi => Some("openai"),
            Self::Groq => Some("groq"),
            Self::Cerebras => Some("cerebras"),
            Self::Fireworks => Some("fireworks"),
            Self::ElevenLabs => Some("elevenlabs"),
            Self::AppleLocal | Self::Parakeet => None,
        }
    }

    pub fn remote_index(self) -> Option<usize> {
        Self::REMOTE.iter().position(|provider| *provider == self)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Groq => "Groq",
            Self::Cerebras => "Cerebras",
            Self::Fireworks => "Fireworks",
            Self::ElevenLabs => "ElevenLabs",
            Self::AppleLocal => "Apple Intelligence",
            Self::Parakeet => "Parakeet",
        }
    }

    pub fn logo(self) -> &'static str {
        match self {
            Self::OpenAi => "assets/icons/openai.png",
            Self::Groq => "assets/icons/groq.png",
            Self::Cerebras => "assets/icons/cerebras.png",
            Self::Fireworks => "assets/icons/fireworks.png",
            Self::ElevenLabs => "assets/icons/elevenlabs.png",
            Self::AppleLocal => "assets/icons/apple-intelligence.png",
            Self::Parakeet => "assets/icons/nvidia.png",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
            Self::Cerebras => "https://api.cerebras.ai/v1",
            Self::Fireworks => "https://api.fireworks.ai/inference/v1",
            Self::ElevenLabs => "https://api.elevenlabs.io/v1",
            Self::AppleLocal | Self::Parakeet => "",
        }
    }

    pub fn is_local(self) -> bool {
        matches!(self, Self::AppleLocal | Self::Parakeet)
    }

    pub fn stt_endpoint(self, base: &str) -> String {
        self.stt_endpoint_for_model(base, "")
    }

    pub fn stt_endpoint_for_model(self, base: &str, model: &str) -> String {
        if self == Self::ElevenLabs {
            return format!("{}/speech-to-text", base.trim_end_matches('/'));
        }

        if self == Self::Fireworks && fireworks_uses_default_inference_base(base) {
            let base = if model.to_lowercase().contains("turbo") {
                "https://audio-turbo.api.fireworks.ai/v1"
            } else {
                "https://audio-prod.api.fireworks.ai/v1"
            };
            return format!("{base}/audio/transcriptions");
        }

        format!("{}/audio/transcriptions", base.trim_end_matches('/'))
    }

    pub fn llm_endpoint(self, base: &str) -> String {
        format!("{}/chat/completions", base.trim_end_matches('/'))
    }

    pub fn from_model_info_provider(s: &str) -> Option<Self> {
        match s {
            "OpenAI" => Some(Self::OpenAi),
            "Groq" => Some(Self::Groq),
            "Cerebras" => Some(Self::Cerebras),
            "Fireworks" => Some(Self::Fireworks),
            "ElevenLabs" | "Eleven Labs" => Some(Self::ElevenLabs),
            "Apple Local" | "Apple Intelligence" => Some(Self::AppleLocal),
            "Parakeet" => Some(Self::Parakeet),
            _ => None,
        }
    }

    pub fn from_key_id(s: &str) -> Option<Self> {
        match s {
            "openai" => Some(Self::OpenAi),
            "groq" => Some(Self::Groq),
            "cerebras" => Some(Self::Cerebras),
            "fireworks" => Some(Self::Fireworks),
            "elevenlabs" => Some(Self::ElevenLabs),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProvidersConfig {
    pub openai: ProviderCredentials,
    pub groq: ProviderCredentials,
    pub cerebras: ProviderCredentials,
    pub fireworks: ProviderCredentials,
    pub elevenlabs: ProviderCredentials,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            openai: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::OpenAi.default_base_url().to_string(),
            },
            groq: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::Groq.default_base_url().to_string(),
            },
            cerebras: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::Cerebras.default_base_url().to_string(),
            },
            fireworks: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::Fireworks.default_base_url().to_string(),
            },
            elevenlabs: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::ElevenLabs.default_base_url().to_string(),
            },
        }
    }
}

impl ProvidersConfig {
    pub fn credentials_for(&self, provider: Provider) -> &ProviderCredentials {
        match provider {
            Provider::OpenAi => &self.openai,
            Provider::Groq => &self.groq,
            Provider::Cerebras => &self.cerebras,
            Provider::Fireworks => &self.fireworks,
            Provider::ElevenLabs => &self.elevenlabs,
            Provider::AppleLocal | Provider::Parakeet => {
                panic!("local providers do not use API credentials")
            }
        }
    }

    pub fn credentials_for_mut(&mut self, provider: Provider) -> &mut ProviderCredentials {
        match provider {
            Provider::OpenAi => &mut self.openai,
            Provider::Groq => &mut self.groq,
            Provider::Cerebras => &mut self.cerebras,
            Provider::Fireworks => &mut self.fireworks,
            Provider::ElevenLabs => &mut self.elevenlabs,
            Provider::AppleLocal | Provider::Parakeet => {
                panic!("local providers do not use API credentials")
            }
        }
    }

    pub fn remote_credentials(&self) -> impl Iterator<Item = (Provider, &ProviderCredentials)> {
        Provider::REMOTE
            .into_iter()
            .map(|provider| (provider, self.credentials_for(provider)))
    }
}

fn fireworks_uses_default_inference_base(base: &str) -> bool {
    let trimmed = base.trim().trim_end_matches('/');
    trimmed.is_empty()
        || trimmed == Provider::Fireworks.default_base_url()
        || trimmed == "https://api.fireworks.ai/inference"
        || trimmed.contains("api.fireworks.ai/inference")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ProviderCredentials {
    #[serde(skip)]
    pub api_key: String,
    pub base_url: String,
}

impl ProviderCredentials {
    pub fn resolve_api_key(&self, label: &str) -> Result<String> {
        if !self.api_key.trim().is_empty() {
            return Ok(self.api_key.trim().to_string());
        }
        anyhow::bail!("missing {label} API key; set it in Glide settings")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_metadata_is_stable() {
        let cases = [
            (Provider::OpenAi, "OpenAI", "https://api.openai.com/v1"),
            (Provider::Groq, "Groq", "https://api.groq.com/openai/v1"),
            (Provider::Cerebras, "Cerebras", "https://api.cerebras.ai/v1"),
            (
                Provider::Fireworks,
                "Fireworks",
                "https://api.fireworks.ai/inference/v1",
            ),
            (
                Provider::ElevenLabs,
                "ElevenLabs",
                "https://api.elevenlabs.io/v1",
            ),
            (Provider::AppleLocal, "Apple Intelligence", ""),
            (Provider::Parakeet, "Parakeet", ""),
        ];

        assert_eq!(Provider::ALL.len(), cases.len());
        for (provider, label, base_url) in cases {
            assert_eq!(provider.label(), label);
            assert_eq!(provider.default_base_url(), base_url);
        }
        assert_eq!(
            Provider::REMOTE
                .into_iter()
                .filter_map(Provider::key_id)
                .collect::<Vec<_>>(),
            ["openai", "groq", "cerebras", "fireworks", "elevenlabs"]
        );
    }

    #[test]
    fn fireworks_stt_turbo_uses_audio_turbo_endpoint() {
        assert_eq!(
            Provider::Fireworks
                .stt_endpoint_for_model(Provider::Fireworks.default_base_url(), "whisper-v3-turbo"),
            "https://audio-turbo.api.fireworks.ai/v1/audio/transcriptions"
        );
    }

    #[test]
    fn resolves_direct_api_key_and_rejects_missing_key() {
        let creds = ProviderCredentials {
            api_key: " direct-key ".to_string(),
            ..Default::default()
        };
        assert_eq!(creds.resolve_api_key("test").unwrap(), "direct-key");

        let missing = ProviderCredentials::default()
            .resolve_api_key("test")
            .unwrap_err()
            .to_string();
        assert!(missing.contains("missing test API key"));
    }
}

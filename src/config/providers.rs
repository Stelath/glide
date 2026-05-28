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
    #[allow(dead_code)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

fn fireworks_uses_default_inference_base(base: &str) -> bool {
    let trimmed = base.trim().trim_end_matches('/');
    trimmed.is_empty()
        || trimmed == Provider::Fireworks.default_base_url()
        || trimmed == "https://api.fireworks.ai/inference"
        || trimmed.contains("api.fireworks.ai/inference")
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

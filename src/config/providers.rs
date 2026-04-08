use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    Groq,
}

impl Provider {
    #[allow(dead_code)]
    pub const ALL: [Self; 2] = [Self::OpenAi, Self::Groq];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Groq => "Groq",
        }
    }

    pub fn logo(self) -> &'static str {
        match self {
            Self::OpenAi => "assets/icons/openai.png",
            Self::Groq => "assets/icons/groq.png",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
        }
    }

    pub fn stt_endpoint(self, base: &str) -> String {
        format!("{}/audio/transcriptions", base.trim_end_matches('/'))
    }

    pub fn llm_endpoint(self, base: &str) -> String {
        format!("{}/chat/completions", base.trim_end_matches('/'))
    }

    pub fn from_model_info_provider(s: &str) -> Option<Self> {
        match s {
            "OpenAI" => Some(Self::OpenAi),
            "Groq" => Some(Self::Groq),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub openai: ProviderCredentials,
    pub groq: ProviderCredentials,
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
        }
    }
}

impl ProvidersConfig {
    pub fn credentials_for(&self, provider: Provider) -> &ProviderCredentials {
        match provider {
            Provider::OpenAi => &self.openai,
            Provider::Groq => &self.groq,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCredentials {
    #[serde(skip)]
    pub api_key: String,
    pub base_url: String,
}

impl Default for ProviderCredentials {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
        }
    }
}

impl ProviderCredentials {
    pub fn resolve_api_key(&self, label: &str) -> Result<String> {
        if !self.api_key.trim().is_empty() {
            return Ok(self.api_key.trim().to_string());
        }
        anyhow::bail!("missing {label} API key; set it in Glide settings")
    }
}

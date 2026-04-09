use anyhow::Result;

use crate::config::{Provider, ProvidersConfig};

mod openai;

#[derive(Debug, Clone, Default)]
pub struct CleanupContext {
    pub target_app: Option<String>,
    pub mode_hint: Option<String>,
}

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn clean(&self, raw_text: &str, context: &CleanupContext) -> Result<String>;
    fn name(&self) -> &'static str;
}

pub fn build_provider(
    provider: Provider,
    model: &str,
    system_prompt: &str,
    providers: &ProvidersConfig,
) -> Result<Box<dyn LlmProvider>> {
    match provider {
        // Both OpenAI and Groq use the OpenAI-compatible API format
        Provider::OpenAi | Provider::Groq => Ok(Box::new(openai::OpenAiLlmProvider::new(
            provider,
            model,
            system_prompt,
            providers,
        )?)),
    }
}

use anyhow::Result;

use crate::config::{GlideConfig, LlmProviderKind};

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

pub fn build_provider(config: &GlideConfig) -> Result<Option<Box<dyn LlmProvider>>> {
    match config.llm.provider {
        LlmProviderKind::None => Ok(None),
        // Both OpenAI and Groq use the OpenAI-compatible API format
        LlmProviderKind::OpenAi | LlmProviderKind::Groq => {
            Ok(Some(Box::new(openai::OpenAiLlmProvider::new(config.clone())?)))
        }
    }
}

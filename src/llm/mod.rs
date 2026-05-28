use anyhow::Result;

use crate::{
    benchmark::ProfileCollector,
    config::{Provider, ProvidersConfig},
};

mod apple;
mod openai;
mod util;

pub(crate) use util::{
    build_cleanup_system_prompt, build_cleanup_user_prompt, prepare_cleanup_transcript,
    strip_think_tags,
};

#[derive(Debug, Clone)]
pub struct CleanupContext {
    pub target_app: Option<String>,
    pub mode_hint: Option<String>,
    pub apply_edit_preprocessing: bool,
}

impl Default for CleanupContext {
    fn default() -> Self {
        Self {
            target_app: None,
            mode_hint: None,
            apply_edit_preprocessing: true,
        }
    }
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
    build_profiled_provider(
        provider,
        model,
        system_prompt,
        providers,
        ProfileCollector::disabled(),
    )
}

pub(crate) fn build_profiled_provider(
    provider: Provider,
    model: &str,
    system_prompt: &str,
    providers: &ProvidersConfig,
    profile: ProfileCollector,
) -> Result<Box<dyn LlmProvider>> {
    let system_prompt = build_cleanup_system_prompt(system_prompt);
    match provider {
        // OpenAI, Groq, Cerebras, and Fireworks use the OpenAI-compatible API format.
        Provider::OpenAi | Provider::Groq | Provider::Cerebras | Provider::Fireworks => {
            Ok(Box::new(openai::OpenAiLlmProvider::new(
                provider,
                model,
                &system_prompt,
                providers,
                profile,
            )?))
        }
        Provider::AppleLocal => Ok(Box::new(apple::AppleFoundationLlmProvider::new(
            model,
            &system_prompt,
            profile,
        ))),
        Provider::ElevenLabs => {
            anyhow::bail!("ElevenLabs does not provide an LLM cleanup model")
        }
        Provider::Parakeet => anyhow::bail!("Parakeet does not provide an LLM cleanup model"),
    }
}

//! Text cleanup: the second pipeline stage. Takes the raw transcript from
//! [`crate::engines::stt`] and polishes it (grammar, punctuation, style prompt,
//! removing filler). Same provider-trait-plus-factory shape as STT, but its
//! input and output are both text.

use anyhow::Result;

use crate::{
    config::{Provider, ProvidersConfig},
    profile::ProfileCollector,
};

mod apple;
mod openai;
mod util;

pub(crate) use util::{build_cleanup_system_prompt, build_cleanup_user_prompt, strip_think_tags};

#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    async fn clean(&self, raw_text: &str) -> Result<String>;
    fn name(&self) -> &'static str;
}

pub(crate) fn build_profiled_provider(
    provider: Provider,
    model: &str,
    prompt_template: &str,
    style_prompt: Option<&str>,
    providers: &ProvidersConfig,
    profile: ProfileCollector,
) -> Result<Box<dyn LlmProvider>> {
    let system_prompt = build_cleanup_system_prompt(prompt_template, style_prompt);
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

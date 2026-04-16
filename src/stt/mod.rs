use anyhow::Result;

use crate::{
    audio::AudioFormat,
    config::{Provider, ProvidersConfig},
};

mod openai;

#[async_trait::async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio: &[u8], format: AudioFormat) -> Result<String>;
    fn name(&self) -> &'static str;
}

pub fn build_provider(
    provider: Provider,
    model: &str,
    providers: &ProvidersConfig,
    vocabulary_prompt: Option<String>,
) -> Result<Box<dyn SttProvider>> {
    match provider {
        // Both OpenAI and Groq use the OpenAI-compatible API format
        Provider::OpenAi | Provider::Groq => Ok(Box::new(openai::OpenAiSttProvider::new(
            provider,
            model,
            providers,
            vocabulary_prompt,
        )?)),
    }
}

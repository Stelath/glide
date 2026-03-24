use anyhow::Result;

use crate::{audio::AudioFormat, config::{GlideConfig, Provider}};

mod openai;

#[async_trait::async_trait]
pub trait SttProvider: Send + Sync {
    async fn transcribe(&self, audio: &[u8], format: AudioFormat) -> Result<String>;
    fn name(&self) -> &'static str;
}

pub fn build_provider(config: &GlideConfig) -> Result<Box<dyn SttProvider>> {
    match config.stt.provider {
        // Both OpenAI and Groq use the OpenAI-compatible API format
        Provider::OpenAi | Provider::Groq => {
            Ok(Box::new(openai::OpenAiSttProvider::new(config.clone())?))
        }
    }
}

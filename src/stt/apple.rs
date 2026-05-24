use anyhow::Result;

use crate::{apple_helper, audio::AudioFormat};

pub struct AppleSpeechProvider {
    model_id: String,
    vocabulary: Vec<String>,
}

impl AppleSpeechProvider {
    pub fn new(model_id: &str, vocabulary_prompt: Option<String>) -> Result<Self> {
        let model_id = crate::local_models::resolve_apple_speech_model_id(model_id)
            .ok_or_else(|| anyhow::anyhow!("Apple Speech model is not installed: {model_id}"))?;
        Ok(Self {
            model_id,
            vocabulary: vocabulary_prompt
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|term| !term.is_empty())
                .take(100)
                .map(ToOwned::to_owned)
                .collect(),
        })
    }
}

#[async_trait::async_trait]
impl super::SttProvider for AppleSpeechProvider {
    async fn transcribe(&self, audio: &[u8], format: AudioFormat) -> Result<String> {
        match format {
            AudioFormat::Wav => {}
        }

        let audio = audio.to_vec();
        let model_id = self.model_id.clone();
        let vocabulary = self.vocabulary.clone();
        tokio::task::spawn_blocking(move || apple_helper::transcribe(&audio, model_id, vocabulary))
            .await?
    }

    fn name(&self) -> &'static str {
        "Apple Speech"
    }
}

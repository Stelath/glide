use anyhow::Result;

use crate::apple_helper;

use super::CleanupContext;

pub struct AppleFoundationLlmProvider {
    model_id: String,
    system_prompt: String,
}

impl AppleFoundationLlmProvider {
    pub fn new(model_id: &str, system_prompt: &str) -> Self {
        Self {
            model_id: model_id.to_string(),
            system_prompt: system_prompt.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl super::LlmProvider for AppleFoundationLlmProvider {
    async fn clean(&self, raw_text: &str, context: &CleanupContext) -> Result<String> {
        let raw_text = raw_text.to_string();
        let model_id = self.model_id.clone();
        let system_prompt = self.system_prompt.clone();
        let context = context.clone();
        tokio::task::spawn_blocking(move || {
            apple_helper::cleanup(&model_id, &raw_text, &system_prompt, &context)
        })
        .await?
    }

    fn name(&self) -> &'static str {
        "Apple Foundation Models"
    }
}

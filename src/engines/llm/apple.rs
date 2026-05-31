use anyhow::Result;

use crate::{engines::apple_helper, profile::ProfileCollector};

pub struct AppleFoundationLlmProvider {
    model_id: String,
    system_prompt: String,
    profile: ProfileCollector,
}

impl AppleFoundationLlmProvider {
    pub fn new(model_id: &str, system_prompt: &str, profile: ProfileCollector) -> Self {
        Self {
            model_id: model_id.to_string(),
            system_prompt: system_prompt.to_string(),
            profile,
        }
    }
}

#[async_trait::async_trait]
impl super::LlmProvider for AppleFoundationLlmProvider {
    async fn clean(&self, raw_text: &str) -> Result<String> {
        let raw_text = raw_text.trim().to_string();
        let model_id = self.model_id.clone();
        let system_prompt = self.system_prompt.clone();
        let profile = self.profile.clone();
        tokio::task::spawn_blocking(move || {
            apple_helper::cleanup_profiled(&model_id, &raw_text, &system_prompt, profile)
        })
        .await?
    }

    fn name(&self) -> &'static str {
        "Apple Foundation Models"
    }
}

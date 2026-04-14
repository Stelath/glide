//! API routing seam — the one place Phase 2 subscription routing will plug in.

use anyhow::{Result, bail};

use crate::config::{GlideConfig, Provider};
use crate::llm::{self, LlmProvider};
use crate::stt::{self, SttProvider};

#[derive(Debug, Clone)]
pub enum ApiRouting {
    UserProvidedKeys,
    #[allow(dead_code)]
    Subscription { bearer: String, base_url: String },
}

pub fn current_routing(_config: &GlideConfig) -> ApiRouting {
    ApiRouting::UserProvidedKeys
}

pub fn build_stt_provider(
    config: &GlideConfig,
    provider: Provider,
    model: &str,
) -> Result<Box<dyn SttProvider>> {
    match current_routing(config) {
        ApiRouting::UserProvidedKeys => stt::build_provider(provider, model, &config.providers),
        ApiRouting::Subscription { .. } => bail!("subscription routing not yet implemented"),
    }
}

pub fn build_llm_provider(
    config: &GlideConfig,
    provider: Provider,
    model: &str,
    system_prompt: &str,
) -> Result<Box<dyn LlmProvider>> {
    match current_routing(config) {
        ApiRouting::UserProvidedKeys => {
            llm::build_provider(provider, model, system_prompt, &config.providers)
        }
        ApiRouting::Subscription { .. } => bail!("subscription routing not yet implemented"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_1_always_uses_byo_keys() {
        let config = GlideConfig::default();
        match current_routing(&config) {
            ApiRouting::UserProvidedKeys => {}
            _ => panic!("Phase 1 should always return UserProvidedKeys"),
        }
    }
}

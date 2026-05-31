use anyhow::Result;

pub const GLIDE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use crate::audio::{AudioFormat, RecordedAudio};
pub use crate::config::{
    GlideConfig, ModelSelection, PasteConfig, Provider, ProvidersConfig, ReplacementRule,
};
pub use crate::engines::llm::LlmProvider;
pub use crate::engines::stt::SttProvider;
pub use crate::profile::{ProfileCollector, SpanRecord};

pub fn build_profiled_stt_provider(
    provider: Provider,
    model: &str,
    providers: &ProvidersConfig,
    vocabulary_prompt: Option<String>,
    profile: ProfileCollector,
) -> Result<Box<dyn SttProvider>> {
    crate::engines::stt::build_profiled_provider(
        provider,
        model,
        providers,
        vocabulary_prompt,
        profile,
    )
}

pub fn build_llm_provider(
    provider: Provider,
    model: &str,
    system_prompt: &str,
    providers: &ProvidersConfig,
) -> Result<Box<dyn LlmProvider>> {
    crate::engines::llm::build_profiled_provider(
        provider,
        model,
        system_prompt,
        None,
        providers,
        ProfileCollector::disabled(),
    )
}

pub fn build_profiled_llm_provider(
    provider: Provider,
    model: &str,
    system_prompt: &str,
    providers: &ProvidersConfig,
    profile: ProfileCollector,
) -> Result<Box<dyn LlmProvider>> {
    crate::engines::llm::build_profiled_provider(
        provider,
        model,
        system_prompt,
        None,
        providers,
        profile,
    )
}

pub fn strip_think_tags(text: &str) -> String {
    crate::engines::llm::strip_think_tags(text)
}

pub fn paste_text(text: &str, config: &PasteConfig) -> Result<()> {
    crate::platform::paste::paste_text(text, config)
}

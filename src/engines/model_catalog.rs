mod catalog;
mod defaults;
mod remote;
mod types;
mod verification;

pub use catalog::{cached_llm_models, cached_stt_models};
pub use defaults::{apply_smart_defaults, smart_llm_default, smart_stt_default};
pub use remote::fetch_all_models;
pub use types::ModelInfo;
pub use verification::{any_provider_verified, provider_verified};

#[cfg(test)]
use crate::{
    config::{GlideConfig, ModelSelection, Provider},
    engines::model_assets::{self, ParakeetInstallState},
};
#[cfg(test)]
use catalog::{fallback_llm_models, fallback_stt_models};
#[cfg(test)]
use remote::{
    ElevenLabsModelsResponseEntry, append_elevenlabs_scribe_models, excluded_remote_llm_model,
};
#[cfg(test)]
use verification::{PROVIDER_VERIFIED, set_remote_provider_verified};

#[cfg(test)]
mod tests;

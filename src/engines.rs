//! Speech & text engines.
//!
//! These split into two kinds of thing, which is the part that's easy to
//! confuse:
//!
//! **Inference stages** — the two sequential steps the [`pipeline`] runs on a
//! recording. They look structurally similar on purpose (each is a provider
//! `trait` plus a `build_profiled_provider` factory that dispatches on the
//! configured [`Provider`]), but they do different jobs:
//!
//! - [`stt`] — speech-to-text: audio bytes → raw transcript text
//! - [`llm`] — cleanup: raw transcript → polished text (grammar, punctuation,
//!   style prompt, filler removal)
//!
//! A single provider can serve both roles through different APIs (e.g. OpenAI:
//! Whisper transcription vs. chat completion); others do only one (ElevenLabs
//! and Parakeet are STT-only; Cerebras is LLM-only).
//!
//! **Model infrastructure** — not pipeline stages, but support the engines:
//!
//! - [`local_models`] — lifecycle of *on-device* model files (download,
//!   install, validate, status) for Apple Speech and Parakeet. "Do we have the
//!   files, are they ready?"
//! - [`model_catalog`] — discovery of *remote* models + smart defaults
//! - [`apple_helper`] — the IPC subprocess used to reach Apple's on-device
//!   Speech and Foundation models
//!
//! [`pipeline`]: crate::pipeline
//! [`Provider`]: crate::config::Provider

pub mod apple_helper;
pub mod llm;
pub mod local_models;
pub mod model_catalog;
pub mod stt;

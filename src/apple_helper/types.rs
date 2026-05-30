use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleCapabilities {
    #[serde(default)]
    pub apple_speech_available: bool,
    #[serde(default)]
    pub apple_speech_reason: String,
    #[serde(default)]
    pub foundation_models_reason: String,
}

#[cfg_attr(test, allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleSpeechModel {
    pub id: String,
    pub display_name: String,
    pub locale_id: String,
    pub status: String,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub reserved: bool,
}

#[cfg_attr(test, allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleFoundationModel {
    pub id: String,
    pub display_name: String,
    pub model_name: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleSpeechInstallProgress {
    #[serde(default)]
    pub ok: bool,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub model_id: String,
    pub fraction_completed: Option<f64>,
    pub completed_unit_count: Option<i64>,
    pub total_unit_count: Option<i64>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HelperResponse {
    pub(super) ok: bool,
    pub(super) text: Option<String>,
    #[serde(default)]
    pub(super) timings: Vec<HelperTiming>,
    #[cfg_attr(test, allow(dead_code))]
    #[serde(default)]
    pub(super) speech_models: Vec<AppleSpeechModel>,
    #[cfg_attr(test, allow(dead_code))]
    #[serde(default)]
    pub(super) foundation_models: Vec<AppleFoundationModel>,
    #[serde(default)]
    pub(super) apple_speech_available: bool,
    #[serde(default)]
    pub(super) apple_speech_reason: String,
    #[serde(default)]
    pub(super) foundation_models_reason: String,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HelperTiming {
    pub(super) phase: String,
    pub(super) duration_ms: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TranscribeRequest {
    pub(super) audio_path: String,
    pub(super) model_id: String,
    pub(super) vocabulary: Vec<String>,
    pub(super) profile: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SpeechModelRequest<'a> {
    pub(super) model_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CleanupRequest<'a> {
    pub(super) model_id: &'a str,
    pub(super) raw_text: &'a str,
    pub(super) system_prompt: &'a str,
    pub(super) target_app: Option<&'a str>,
    pub(super) mode_hint: Option<&'a str>,
    pub(super) profile: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PersistentHelperRequest<'a> {
    pub(super) command: &'a str,
    pub(super) request: Value,
}

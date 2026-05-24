use std::{
    io::Write,
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::llm::CleanupContext;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppleCapabilities {
    #[serde(default)]
    pub apple_speech_available: bool,
    #[serde(default)]
    pub apple_speech_reason: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub foundation_models_available: bool,
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
struct HelperResponse {
    ok: bool,
    text: Option<String>,
    #[cfg_attr(test, allow(dead_code))]
    #[serde(default)]
    speech_models: Vec<AppleSpeechModel>,
    #[cfg_attr(test, allow(dead_code))]
    #[serde(default)]
    foundation_models: Vec<AppleFoundationModel>,
    #[serde(default)]
    apple_speech_available: bool,
    #[serde(default)]
    apple_speech_reason: String,
    #[serde(default)]
    foundation_models_available: bool,
    #[serde(default)]
    foundation_models_reason: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscribeRequest {
    audio_path: String,
    model_id: String,
    vocabulary: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SpeechModelRequest<'a> {
    model_id: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupRequest<'a> {
    model_id: &'a str,
    raw_text: &'a str,
    system_prompt: &'a str,
    target_app: Option<&'a str>,
    mode_hint: Option<&'a str>,
}

static CAPABILITIES: OnceLock<Mutex<Option<AppleCapabilities>>> = OnceLock::new();

pub fn cached_capabilities() -> AppleCapabilities {
    let cache = CAPABILITIES.get_or_init(|| Mutex::new(None));
    let mut locked = cache.lock().expect("Apple capabilities cache poisoned");
    if let Some(capabilities) = locked.clone() {
        return capabilities;
    }

    let capabilities = capabilities().unwrap_or_else(|error| AppleCapabilities {
        apple_speech_available: false,
        apple_speech_reason: error.to_string(),
        foundation_models_available: false,
        foundation_models_reason: error.to_string(),
    });
    *locked = Some(capabilities.clone());
    capabilities
}

pub fn invalidate_capabilities_cache() {
    if let Ok(mut locked) = CAPABILITIES.get_or_init(|| Mutex::new(None)).lock() {
        *locked = None;
    }
}

pub fn capabilities() -> Result<AppleCapabilities> {
    let response = run_helper("capabilities", None)?;
    Ok(AppleCapabilities {
        apple_speech_available: response.apple_speech_available,
        apple_speech_reason: response.apple_speech_reason,
        foundation_models_available: response.foundation_models_available,
        foundation_models_reason: response.foundation_models_reason,
    })
}

#[cfg_attr(test, allow(dead_code))]
pub fn speech_models() -> Result<Vec<AppleSpeechModel>> {
    let response = run_helper("speech-models", None)?;
    Ok(response.speech_models)
}

#[cfg_attr(test, allow(dead_code))]
pub fn foundation_models() -> Result<Vec<AppleFoundationModel>> {
    let response = run_helper("foundation-models", None)?;
    Ok(response.foundation_models)
}

pub fn release_speech_model(model_id: &str) -> Result<()> {
    let input = speech_model_request_json(model_id)?;
    run_helper("release-speech-model", Some(&input)).map(|_| ())
}

pub(crate) fn speech_model_request_json(model_id: &str) -> Result<Vec<u8>> {
    serde_json::to_vec(&SpeechModelRequest { model_id })
        .context("failed to encode Apple Speech model request")
}

pub fn transcribe(audio: &[u8], model_id: String, vocabulary: Vec<String>) -> Result<String> {
    let audio_path = write_temp_audio(audio)?;
    let request = TranscribeRequest {
        audio_path: audio_path.to_string_lossy().to_string(),
        model_id,
        vocabulary,
    };
    let input = serde_json::to_vec(&request).context("failed to encode Apple Speech request")?;
    let result = run_helper("transcribe", Some(&input)).and_then(|response| {
        response
            .text
            .map(|text| text.trim().to_string())
            .context("Apple Speech helper did not return text")
    });
    std::fs::remove_file(&audio_path).ok();
    result
}

pub fn cleanup(
    model_id: &str,
    raw_text: &str,
    system_prompt: &str,
    context: &CleanupContext,
) -> Result<String> {
    let request = CleanupRequest {
        model_id,
        raw_text,
        system_prompt,
        target_app: context.target_app.as_deref(),
        mode_hint: context.mode_hint.as_deref(),
    };
    let input =
        serde_json::to_vec(&request).context("failed to encode Apple Foundation Models request")?;
    run_helper("cleanup", Some(&input)).and_then(|response| {
        response
            .text
            .map(|text| text.trim().to_string())
            .context("Apple Foundation Models helper did not return text")
    })
}

fn run_helper(command: &str, input: Option<&[u8]>) -> Result<HelperResponse> {
    let helper = helper_path()?;
    let mut child = Command::new(&helper)
        .arg(command)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start Apple helper at {}", helper.display()))?;

    if let Some(input) = input {
        let stdin = child
            .stdin
            .as_mut()
            .context("failed to open Apple helper stdin")?;
        stdin
            .write_all(input)
            .context("failed to write Apple helper request")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to wait for Apple helper")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    decode_helper_response(command, &output.status, stdout.trim(), stderr.trim())
}

fn decode_helper_response(
    command: &str,
    status: &ExitStatus,
    stdout: &str,
    stderr: &str,
) -> Result<HelperResponse> {
    if stdout.is_empty() {
        anyhow::bail!("{}", helper_failure_message(command, status, stderr));
    }

    let response: HelperResponse = serde_json::from_str(stdout).with_context(|| {
        format!(
            "failed to parse Apple helper response; status: {}, stderr: {}",
            status, stderr
        )
    })?;

    if !status.success() || !response.ok {
        anyhow::bail!(
            "{}",
            response
                .error
                .unwrap_or_else(|| helper_failure_message(command, status, stderr))
        );
    }

    Ok(response)
}

pub(crate) fn helper_failure_message(command: &str, status: &ExitStatus, stderr: &str) -> String {
    if command == "foundation-models" && helper_was_fatal_signal(status) {
        return "Apple Foundation model availability check failed: BackgroundAssets validation failed."
            .to_string();
    }

    if command == "capabilities" && helper_was_fatal_signal(status) {
        return "Apple local provider capabilities unavailable: BackgroundAssets validation failed."
            .to_string();
    }

    let mut message = format!(
        "Apple helper failed while running {command}: {}",
        helper_exit_description(status)
    );
    if !stderr.trim().is_empty() {
        message.push_str("; ");
        message.push_str(stderr.trim());
    }
    if helper_was_fatal_signal(status) {
        message.push_str(
            ". Apple local model APIs can abort when BackgroundAssets cannot validate the running app bundle. Use the signed app bundle path; if this persists, the Apple API may require additional provisioning.",
        );
    }
    message
}

fn helper_exit_description(status: &ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            let name = signal_name(signal)
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            return format!("signal {signal}{name}");
        }
    }

    format!("status {status}")
}

fn helper_was_fatal_signal(status: &ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        return matches!(status.signal(), Some(5 | 6));
    }
    #[cfg(not(unix))]
    {
        let _ = status;
        false
    }
}

fn signal_name(signal: i32) -> Option<&'static str> {
    match signal {
        5 => Some("SIGTRAP"),
        6 => Some("SIGABRT"),
        9 => Some("SIGKILL"),
        15 => Some("SIGTERM"),
        _ => None,
    }
}

pub(crate) fn helper_path() -> Result<PathBuf> {
    let exe = std::env::current_exe().context("failed to locate current executable")?;
    if let Some(parent) = exe.parent() {
        if let Some(contents_dir) = parent.parent() {
            let nested = contents_dir
                .join("Helpers")
                .join("GlideAppleHelper.app")
                .join("Contents")
                .join("MacOS")
                .join("GlideAppleHelper");
            if nested.is_file() {
                return Ok(nested);
            }
        }

        let bundled = parent.join("GlideAppleHelper");
        if bundled.is_file() {
            return Ok(bundled);
        }
    }

    if let Some(path) = option_env!("GLIDE_APPLE_HELPER_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
    }

    anyhow::bail!("Apple helper is not available in this build")
}

fn write_temp_audio(audio: &[u8]) -> Result<PathBuf> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("glide-apple-speech-{suffix}.wav"));
    std::fs::write(&path, audio)
        .with_context(|| format!("failed to write temporary audio to {}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_helper_requests_as_camel_case() {
        let request = TranscribeRequest {
            audio_path: "/tmp/glide.wav".to_string(),
            model_id: "speechanalyzer-en_US".to_string(),
            vocabulary: vec!["Glide".to_string()],
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("audioPath"));
        assert!(!json.contains("audio_path"));

        let cleanup = CleanupRequest {
            model_id: "apple-foundation-default",
            raw_text: "hello",
            system_prompt: "clean",
            target_app: None,
            mode_hint: None,
        };
        let json = serde_json::to_string(&cleanup).unwrap();
        assert!(json.contains("modelId"));
        assert!(!json.contains("model_id"));
    }

    #[test]
    fn parses_speech_models_and_unavailable_reasons() {
        let populated = r#"{
            "ok": true,
            "speechModels": [{
                "id": "speechanalyzer-en_US",
                "displayName": "English (United States)",
                "localeId": "en_US",
                "status": "installed",
                "installed": true,
                "reserved": true
            }]
        }"#;
        let response: HelperResponse = serde_json::from_str(populated).unwrap();
        assert!(response.ok);
        assert_eq!(response.speech_models.len(), 1);
        assert_eq!(response.speech_models[0].id, "speechanalyzer-en_US");

        let denied = r#"{
            "ok": false,
            "appleSpeechAvailable": false,
            "appleSpeechReason": "speech recognition permission denied",
            "error": "speech recognition permission denied"
        }"#;
        let response: HelperResponse = serde_json::from_str(denied).unwrap();
        assert!(!response.ok);
        assert_eq!(
            response.apple_speech_reason,
            "speech recognition permission denied"
        );
    }

    #[test]
    fn parses_foundation_model_statuses() {
        let json = r#"{
            "ok": true,
            "foundationModels": [{
                "id": "apple-foundation-default",
                "displayName": "Apple Foundation Model",
                "modelName": "SystemLanguageModel.default",
                "available": true,
                "reason": "available"
            }]
        }"#;
        let response: HelperResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.foundation_models.len(), 1);
        assert!(response.foundation_models[0].available);
        assert_eq!(
            response.foundation_models[0].model_name,
            "SystemLanguageModel.default"
        );
    }

    #[cfg(unix)]
    #[test]
    fn helper_sigabrt_response_reports_crash_instead_of_json_parse_error() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        let status = ExitStatus::from_raw(6);
        let error = decode_helper_response("speech-models", &status, "", "")
            .unwrap_err()
            .to_string();

        assert!(error.contains("speech-models"));
        assert!(error.contains("SIGABRT"));
        assert!(error.contains("signed app"));
    }

    #[cfg(unix)]
    #[test]
    fn foundation_model_fatal_signal_reports_background_assets_failure() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        let status = ExitStatus::from_raw(5);
        let error = decode_helper_response(
            "foundation-models",
            &status,
            "",
            "BackgroundAssets /AssetPackManager.swift:206: Fatal error",
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("Apple Foundation model availability check failed"));
        assert!(error.contains("BackgroundAssets validation failed"));
        assert!(!error.contains("running found"));
    }
}

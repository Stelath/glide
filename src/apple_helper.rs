use std::{
    error::Error,
    fmt,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistentHelperRequest<'a> {
    command: &'a str,
    request: Value,
}

static CAPABILITIES: OnceLock<Mutex<Option<AppleCapabilities>>> = OnceLock::new();
static PERSISTENT_HELPER: OnceLock<Mutex<PersistentHelperClient>> = OnceLock::new();

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
    let started = Instant::now();
    let result = run_persistent_helper("transcribe", &input).and_then(|response| {
        response
            .text
            .map(|text| text.trim().to_string())
            .context("Apple Speech helper did not return text")
    });
    eprintln!(
        "[glide] Apple helper: transcribe request finished in {} ms",
        started.elapsed().as_millis()
    );
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
    let started = Instant::now();
    let result = run_persistent_helper("cleanup", &input).and_then(|response| {
        response
            .text
            .map(|text| text.trim().to_string())
            .context("Apple Foundation Models helper did not return text")
    });
    eprintln!(
        "[glide] Apple helper: cleanup request finished in {} ms",
        started.elapsed().as_millis()
    );
    result
}

fn run_persistent_helper(command: &str, input: &[u8]) -> Result<HelperResponse> {
    let helper = helper_path()?;
    let client = PERSISTENT_HELPER.get_or_init(|| Mutex::new(PersistentHelperClient::new(helper)));
    client
        .lock()
        .expect("Apple persistent helper client poisoned")
        .request(command, input)
}

#[derive(Debug)]
struct PersistentTransportError {
    message: String,
}

impl PersistentTransportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for PersistentTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl Error for PersistentTransportError {}

struct PersistentHelperClient {
    helper: PathBuf,
    server: Option<PersistentHelperServer>,
}

struct PersistentHelperServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl PersistentHelperClient {
    fn new(helper: PathBuf) -> Self {
        Self {
            helper,
            server: None,
        }
    }

    fn request(&mut self, command: &str, input: &[u8]) -> Result<HelperResponse> {
        let request = persistent_request_json(command, input)?;
        for attempt in 0..=1 {
            if self.server.is_none() {
                self.start_server()?;
            }

            let result = self
                .server
                .as_mut()
                .context("Apple persistent helper server was unavailable")?
                .send(command, &request);

            match result {
                Ok(response) => return Ok(response),
                Err(error) if attempt == 0 && is_transport_error(error.as_ref()) => {
                    eprintln!(
                        "[glide] Apple helper: persistent server failed, restarting: {error:#}"
                    );
                    self.stop_server();
                }
                Err(error) => return Err(error),
            }
        }

        unreachable!("persistent helper retry loop should always return")
    }

    fn start_server(&mut self) -> Result<()> {
        let mut child = Command::new(&self.helper)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to start persistent Apple helper at {}",
                    self.helper.display()
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .context("failed to open persistent Apple helper stdin")?;
        let stdout = child
            .stdout
            .take()
            .context("failed to open persistent Apple helper stdout")?;

        eprintln!(
            "[glide] Apple helper: started persistent server at {}",
            self.helper.display()
        );
        self.server = Some(PersistentHelperServer {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        });
        Ok(())
    }

    fn stop_server(&mut self) {
        if let Some(mut server) = self.server.take() {
            let _ = server.child.kill();
            let _ = server.child.wait();
        }
    }
}

impl PersistentHelperServer {
    fn send(&mut self, command: &str, request: &[u8]) -> Result<HelperResponse> {
        self.stdin
            .write_all(request)
            .map_err(|error| persistent_transport_error("write request", error))?;
        self.stdin
            .write_all(b"\n")
            .map_err(|error| persistent_transport_error("write request newline", error))?;
        self.stdin
            .flush()
            .map_err(|error| persistent_transport_error("flush request", error))?;

        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .map_err(|error| persistent_transport_error("read response", error))?;
        if bytes == 0 {
            return Err(
                PersistentTransportError::new("persistent Apple helper closed stdout").into(),
            );
        }

        decode_persistent_response(command, line.trim())
    }
}

fn persistent_request_json(command: &str, input: &[u8]) -> Result<Vec<u8>> {
    let request = serde_json::from_slice(input)
        .with_context(|| format!("failed to encode persistent Apple helper {command} request"))?;
    serde_json::to_vec(&PersistentHelperRequest { command, request })
        .with_context(|| format!("failed to encode persistent Apple helper {command} envelope"))
}

fn decode_persistent_response(command: &str, line: &str) -> Result<HelperResponse> {
    if line.is_empty() {
        anyhow::bail!("Apple helper returned an empty {command} response");
    }

    let response: HelperResponse = serde_json::from_str(line).with_context(|| {
        format!("failed to parse persistent Apple helper {command} response: {line}")
    })?;
    if !response.ok {
        anyhow::bail!(
            "{}",
            response
                .error
                .unwrap_or_else(|| format!("Apple helper returned an error for {command}"))
        );
    }
    Ok(response)
}

fn persistent_transport_error(action: &str, error: std::io::Error) -> anyhow::Error {
    PersistentTransportError::new(format!(
        "failed to {action} through persistent Apple helper: {error}"
    ))
    .into()
}

fn is_transport_error(error: &(dyn Error + 'static)) -> bool {
    error.is::<PersistentTransportError>()
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
        matches!(status.signal(), Some(5 | 6))
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
    fn encodes_persistent_helper_envelope() {
        let request = TranscribeRequest {
            audio_path: "/tmp/glide.wav".to_string(),
            model_id: "speechanalyzer-en_US".to_string(),
            vocabulary: vec!["Glide".to_string()],
        };
        let input = serde_json::to_vec(&request).unwrap();
        let encoded = persistent_request_json("transcribe", &input).unwrap();
        let envelope: serde_json::Value = serde_json::from_slice(&encoded).unwrap();

        assert_eq!(envelope["command"], "transcribe");
        assert_eq!(envelope["request"]["audioPath"], "/tmp/glide.wav");
        assert_eq!(envelope["request"]["modelId"], "speechanalyzer-en_US");
    }

    #[test]
    fn persistent_response_preserves_helper_error_message() {
        let error = decode_persistent_response(
            "cleanup",
            r#"{"ok":false,"error":"Apple Foundation Model unavailable"}"#,
        )
        .unwrap_err()
        .to_string();

        assert_eq!(error, "Apple Foundation Model unavailable");
    }

    #[cfg(unix)]
    #[test]
    fn persistent_helper_restarts_after_transport_failure() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let state = dir.path().join("first-run");
        let helper = dir.path().join("helper.sh");
        fs::write(
            &helper,
            format!(
                r#"#!/bin/sh
if [ "$1" != "serve" ]; then
  echo '{{"ok":false,"error":"unexpected command"}}'
  exit 0
fi
if [ ! -f "{state}" ]; then
  touch "{state}"
  exit 0
fi
while IFS= read -r line; do
  echo '{{"ok":true,"text":"warm"}}'
done
"#,
                state = state.display()
            ),
        )
        .unwrap();
        let mut permissions = fs::metadata(&helper).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&helper, permissions).unwrap();

        let cleanup = CleanupRequest {
            model_id: "apple-foundation-default",
            raw_text: "hello",
            system_prompt: "clean",
            target_app: None,
            mode_hint: None,
        };
        let input = serde_json::to_vec(&cleanup).unwrap();
        let mut client = PersistentHelperClient::new(helper);
        let response = client.request("cleanup", &input).unwrap();

        assert_eq!(response.text.as_deref(), Some("warm"));
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

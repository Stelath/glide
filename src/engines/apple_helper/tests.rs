use super::*;

#[test]
fn encodes_helper_requests_as_camel_case() {
    let request = TranscribeRequest {
        audio_path: "/tmp/glide.wav".to_string(),
        model_id: "speechanalyzer-en_US".to_string(),
        vocabulary: vec!["Glide".to_string()],
        profile: true,
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("audioPath"));
    assert!(!json.contains("audio_path"));

    let cleanup = CleanupRequest {
        model_id: "apple-foundation-default",
        raw_text: "hello",
        system_prompt: "clean",
        profile: true,
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
        profile: false,
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
        profile: false,
    };
    let input = serde_json::to_vec(&cleanup).unwrap();
    let mut client = PersistentHelperClient::new(helper);
    let response = client
        .request("cleanup", &input, &ProfileCollector::disabled())
        .unwrap();

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

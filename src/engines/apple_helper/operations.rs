use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::profile::ProfileCollector;

#[cfg(not(test))]
use super::types::{AppleFoundationModel, AppleSpeechModel};
use super::{
    process::{helper_path, run_helper, write_temp_audio},
    transport::PersistentHelperClient,
    types::{
        AppleCapabilities, CleanupRequest, HelperResponse, HelperTiming, SpeechModelRequest,
        TranscribeRequest,
    },
};

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
        foundation_models_reason: response.foundation_models_reason,
    })
}

#[cfg(not(test))]
pub fn speech_models() -> Result<Vec<AppleSpeechModel>> {
    let response = run_helper("speech-models", None)?;
    Ok(response.speech_models)
}

#[cfg(not(test))]
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

pub(crate) fn transcribe_profiled(
    audio: &[u8],
    model_id: String,
    vocabulary: Vec<String>,
    profile: ProfileCollector,
) -> Result<String> {
    let audio_path = write_temp_audio(audio)?;
    let request = TranscribeRequest {
        audio_path: audio_path.to_string_lossy().to_string(),
        model_id,
        vocabulary,
        profile: profile.is_enabled(),
    };
    let input = serde_json::to_vec(&request).context("failed to encode Apple Speech request")?;
    let started = Instant::now();
    let result = run_persistent_helper("transcribe", &input, &profile).and_then(|response| {
        record_helper_timings(&profile, "apple_stt_helper", &response.timings);
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

pub(crate) fn cleanup_profiled(
    model_id: &str,
    raw_text: &str,
    system_prompt: &str,
    profile: ProfileCollector,
) -> Result<String> {
    let request = CleanupRequest {
        model_id,
        raw_text,
        system_prompt,
        profile: profile.is_enabled(),
    };
    let input =
        serde_json::to_vec(&request).context("failed to encode Apple Foundation Models request")?;
    let started = Instant::now();
    let result = run_persistent_helper("cleanup", &input, &profile).and_then(|response| {
        record_helper_timings(&profile, "apple_foundation_helper", &response.timings);
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

pub(crate) fn prewarm_foundation(model_id: &str, system_prompt: &str) -> Result<()> {
    let request = CleanupRequest {
        model_id,
        raw_text: "",
        system_prompt,
        profile: false,
    };
    let input = serde_json::to_vec(&request)
        .context("failed to encode Apple Foundation prewarm request")?;
    let started = Instant::now();
    let result = run_persistent_helper("prewarm-foundation", &input, &ProfileCollector::disabled())
        .map(|_| ());
    eprintln!(
        "[glide] Apple helper: prewarm request finished in {} ms",
        started.elapsed().as_millis()
    );
    result
}

fn run_persistent_helper(
    command: &str,
    input: &[u8],
    profile: &ProfileCollector,
) -> Result<HelperResponse> {
    let helper = helper_path()?;
    let client = PERSISTENT_HELPER.get_or_init(|| Mutex::new(PersistentHelperClient::new(helper)));
    client
        .lock()
        .expect("Apple persistent helper client poisoned")
        .request(command, input, profile)
}

fn record_helper_timings(profile: &ProfileCollector, prefix: &str, timings: &[HelperTiming]) {
    for timing in timings {
        if timing.duration_ms.is_finite() && timing.duration_ms >= 0.0 {
            profile.record(
                format!("{prefix}_{}", timing.phase),
                Duration::from_secs_f64(timing.duration_ms / 1000.0),
            );
        }
    }
}

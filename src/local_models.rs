use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use bzip2::read::BzDecoder;

pub const APPLE_SPEECH_MODEL_ID: &str = "apple-speech-on-device";
pub const APPLE_SPEECH_MODEL_PREFIX: &str = "speechanalyzer-";
pub const APPLE_FOUNDATION_MODEL_ID: &str = "apple-foundation-default";

#[cfg_attr(test, allow(dead_code))]
const APPLE_FOUNDATION_MODEL_DEFINITIONS: [(&str, &str, &str); 1] = [(
    APPLE_FOUNDATION_MODEL_ID,
    "Apple Foundation Model",
    "SystemLanguageModel.default",
)];

const REQUIRED_PARAKEET_FILES: [&str; 4] = [
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "joiner.int8.onnx",
    "tokens.txt",
];

#[derive(Debug, Clone, Copy)]
pub struct ParakeetModelDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub language: &'static str,
    pub url: &'static str,
    pub archive_name: &'static str,
}

pub const PARAKEET_MODELS: [ParakeetModelDefinition; 2] = [
    ParakeetModelDefinition {
        id: "parakeet-tdt-0.6b-v2-int8",
        label: "Parakeet TDT 0.6B v2 int8",
        language: "English",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
        archive_name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8.tar.bz2",
    },
    ParakeetModelDefinition {
        id: "parakeet-tdt-0.6b-v3-int8",
        label: "Parakeet TDT 0.6B v3 int8",
        language: "25 European languages",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
        archive_name: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8.tar.bz2",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalModelInstallState {
    NotInstalled,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Cancelling {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Installed {
        size_bytes: u64,
    },
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ParakeetModelStatus {
    pub definition: ParakeetModelDefinition,
    pub state: LocalModelInstallState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppleSpeechInstallState {
    NotInstalled,
    Downloading { progress: Option<f64> },
    Cancelling,
    Installed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleSpeechModelDefinition {
    pub id: String,
    pub display_name: String,
    pub locale_id: String,
    pub asset_status: String,
    pub installed: bool,
    pub reserved: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppleSpeechModelStatus {
    pub definition: AppleSpeechModelDefinition,
    pub state: AppleSpeechInstallState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleFoundationModelStatus {
    pub id: String,
    pub display_name: String,
    pub model_name: String,
    pub available: bool,
    pub reason: String,
}

static DOWNLOADS: OnceLock<Mutex<HashMap<String, LocalModelInstallState>>> = OnceLock::new();
static DOWNLOAD_CANCELLATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static APPLE_SPEECH_MODELS: OnceLock<Mutex<Option<Vec<AppleSpeechModelDefinition>>>> =
    OnceLock::new();
static APPLE_SPEECH_MODELS_UNAVAILABLE_REASON: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static APPLE_FOUNDATION_MODELS: OnceLock<Mutex<Option<Vec<AppleFoundationModelStatus>>>> =
    OnceLock::new();
static APPLE_SPEECH_DOWNLOADS: OnceLock<Mutex<HashMap<String, AppleSpeechInstallState>>> =
    OnceLock::new();
static APPLE_SPEECH_DOWNLOAD_CANCELLATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static APPLE_SPEECH_DOWNLOAD_CHILDREN: OnceLock<Mutex<HashMap<String, Arc<Mutex<Option<Child>>>>>> =
    OnceLock::new();

#[cfg_attr(not(test), allow(dead_code))]
pub fn apple_speech_model_id(locale_id: &str) -> String {
    format!("{APPLE_SPEECH_MODEL_PREFIX}{locale_id}")
}

pub fn apple_speech_locale_id(model_id: &str) -> Option<&str> {
    model_id
        .strip_prefix(APPLE_SPEECH_MODEL_PREFIX)
        .filter(|locale| !locale.trim().is_empty())
}

pub fn is_legacy_apple_speech_model(model_id: &str) -> bool {
    model_id == APPLE_SPEECH_MODEL_ID
}

pub fn resolve_apple_speech_model_id(model_id: &str) -> Option<String> {
    if !is_legacy_apple_speech_model(model_id) {
        return Some(model_id.to_string()).filter(|id| apple_speech_locale_id(id).is_some());
    }

    first_installed_apple_speech_model().map(|model| model.definition.id)
}

pub fn resolve_apple_foundation_model_id(model_id: &str) -> Option<String> {
    apple_foundation_models_status()
        .into_iter()
        .find(|model| model.id == model_id && model.available)
        .map(|model| model.id)
}

pub fn first_available_apple_foundation_model() -> Option<AppleFoundationModelStatus> {
    apple_foundation_models_status()
        .into_iter()
        .find(|model| model.available)
}

pub fn apple_foundation_models_status() -> Vec<AppleFoundationModelStatus> {
    apple_foundation_model_statuses()
}

pub fn first_installed_apple_speech_model() -> Option<AppleSpeechModelStatus> {
    apple_speech_models_status()
        .into_iter()
        .find(|model| model.state == AppleSpeechInstallState::Installed)
}

pub fn apple_speech_models_status() -> Vec<AppleSpeechModelStatus> {
    apple_speech_model_definitions()
        .into_iter()
        .map(|definition| {
            let state = apple_speech_install_state_for_definition(&definition);
            AppleSpeechModelStatus { definition, state }
        })
        .collect()
}

pub fn apple_speech_models_unavailable_reason() -> Option<String> {
    APPLE_SPEECH_MODELS_UNAVAILABLE_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|reason| reason.clone())
}

pub fn refresh_apple_local_models() {
    invalidate_apple_speech_model_cache();
    invalidate_apple_foundation_model_cache();
    crate::apple_helper::invalidate_capabilities_cache();
}

pub fn apple_speech_install_state(id: &str) -> AppleSpeechInstallState {
    if let Some(state) = apple_speech_download_state(id) {
        return state;
    }

    apple_speech_model_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .map(|definition| apple_speech_install_state_for_definition(&definition))
        .unwrap_or(AppleSpeechInstallState::NotInstalled)
}

pub fn start_apple_speech_model_download(id: &str) -> Result<()> {
    let definition = apple_speech_model_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .with_context(|| format!("unknown Apple Speech model: {id}"))?;

    if matches!(
        apple_speech_download_state(&definition.id),
        Some(AppleSpeechInstallState::Downloading { .. } | AppleSpeechInstallState::Cancelling)
    ) {
        return Ok(());
    }

    clear_apple_speech_download_cancellation(&definition.id);
    set_apple_speech_download_state(
        &definition.id,
        AppleSpeechInstallState::Downloading { progress: None },
    );

    let model_id = definition.id.clone();
    thread::spawn(move || {
        if let Err(error) = download_and_install_apple_speech_model(&model_id) {
            if is_apple_speech_download_cancelled(&model_id) {
                let _ = crate::apple_helper::release_speech_model(&model_id);
                clear_apple_speech_download_state(&model_id);
            } else {
                set_apple_speech_download_state(
                    &model_id,
                    AppleSpeechInstallState::Failed(error.to_string()),
                );
            }
        } else {
            clear_apple_speech_download_state(&model_id);
        }
        clear_apple_speech_download_child(&model_id);
        clear_apple_speech_download_cancellation(&model_id);
        invalidate_apple_speech_model_cache();
    });

    Ok(())
}

pub fn cancel_apple_speech_model_download(id: &str) -> Result<()> {
    anyhow::ensure!(
        apple_speech_locale_id(id).is_some(),
        "unknown Apple Speech model: {id}"
    );

    match apple_speech_download_state(id) {
        Some(AppleSpeechInstallState::Downloading { .. }) => {
            mark_apple_speech_download_cancelled(id);
            set_apple_speech_download_state(id, AppleSpeechInstallState::Cancelling);
            kill_apple_speech_download_child(id);
        }
        Some(AppleSpeechInstallState::Cancelling) => {
            mark_apple_speech_download_cancelled(id);
            kill_apple_speech_download_child(id);
        }
        _ => {}
    }

    Ok(())
}

pub fn release_apple_speech_model(id: &str) -> Result<()> {
    if matches!(
        apple_speech_download_state(id),
        Some(AppleSpeechInstallState::Downloading { .. } | AppleSpeechInstallState::Cancelling)
    ) {
        cancel_apple_speech_model_download(id)?;
        return Ok(());
    }

    crate::apple_helper::release_speech_model(id)?;
    clear_apple_speech_download_state(id);
    invalidate_apple_speech_model_cache();
    Ok(())
}

pub fn apple_speech_has_active_downloads() -> bool {
    apple_speech_models_status().iter().any(|status| {
        matches!(
            status.state,
            AppleSpeechInstallState::Downloading { .. } | AppleSpeechInstallState::Cancelling
        )
    })
}

fn apple_speech_install_state_for_definition(
    definition: &AppleSpeechModelDefinition,
) -> AppleSpeechInstallState {
    if let Some(state) = apple_speech_download_state(&definition.id) {
        return state;
    }

    if definition.reserved {
        AppleSpeechInstallState::Installed
    } else {
        AppleSpeechInstallState::NotInstalled
    }
}

#[cfg(test)]
fn apple_speech_model_definitions() -> Vec<AppleSpeechModelDefinition> {
    let mut models = vec![
        AppleSpeechModelDefinition {
            id: apple_speech_model_id("en_US"),
            display_name: "English (United States)".to_string(),
            locale_id: "en_US".to_string(),
            asset_status: "installed".to_string(),
            installed: true,
            reserved: true,
        },
        AppleSpeechModelDefinition {
            id: apple_speech_model_id("fr_FR"),
            display_name: "French (France)".to_string(),
            locale_id: "fr_FR".to_string(),
            asset_status: "supported".to_string(),
            installed: false,
            reserved: false,
        },
    ];

    if let Ok(id) = std::env::var("GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID")
        && let Some(locale_id) = apple_speech_locale_id(&id)
        && !models.iter().any(|model| model.id == id)
    {
        let locale_id = locale_id.to_string();
        models.push(AppleSpeechModelDefinition {
            id,
            display_name: format!("Test locale {locale_id}"),
            locale_id,
            asset_status: "supported".to_string(),
            installed: false,
            reserved: false,
        });
    }

    models
}

#[cfg(not(test))]
fn apple_speech_model_definitions() -> Vec<AppleSpeechModelDefinition> {
    let cache = APPLE_SPEECH_MODELS.get_or_init(|| Mutex::new(None));
    if let Ok(mut cached) = cache.lock() {
        if let Some(models) = cached.clone() {
            return models;
        }

        let models = match crate::apple_helper::speech_models() {
            Ok(models) => models
                .into_iter()
                .map(|model| AppleSpeechModelDefinition {
                    id: model.id,
                    display_name: model.display_name,
                    locale_id: model.locale_id,
                    asset_status: model.status,
                    installed: model.installed,
                    reserved: model.reserved,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                set_apple_speech_models_unavailable_reason(Some(error.to_string()));
                *cached = None;
                return Vec::new();
            }
        };

        if models.is_empty() {
            set_apple_speech_models_unavailable_reason(Some(
                "Apple Speech returned no supported locales".to_string(),
            ));
            *cached = None;
        } else {
            set_apple_speech_models_unavailable_reason(None);
            *cached = Some(models.clone());
        }
        models
    } else {
        Vec::new()
    }
}

fn invalidate_apple_speech_model_cache() {
    if let Ok(mut cache) = APPLE_SPEECH_MODELS.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
    set_apple_speech_models_unavailable_reason(None);
}

fn set_apple_speech_models_unavailable_reason(reason: Option<String>) {
    if let Ok(mut locked) = APPLE_SPEECH_MODELS_UNAVAILABLE_REASON
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *locked = reason;
    }
}

#[cfg(test)]
fn apple_foundation_model_statuses() -> Vec<AppleFoundationModelStatus> {
    vec![AppleFoundationModelStatus {
        id: APPLE_FOUNDATION_MODEL_ID.to_string(),
        display_name: "Apple Foundation Model".to_string(),
        model_name: "SystemLanguageModel.default".to_string(),
        available: true,
        reason: "available".to_string(),
    }]
}

#[cfg(not(test))]
fn apple_foundation_model_statuses() -> Vec<AppleFoundationModelStatus> {
    let cache = APPLE_FOUNDATION_MODELS.get_or_init(|| Mutex::new(None));
    if let Ok(mut cached) = cache.lock() {
        if let Some(models) = cached.clone() {
            return models;
        }

        let models = match crate::apple_helper::foundation_models() {
            Ok(models) => models
                .into_iter()
                .map(|model| AppleFoundationModelStatus {
                    id: model.id,
                    display_name: model.display_name,
                    model_name: model.model_name,
                    available: model.available,
                    reason: model.reason,
                })
                .collect::<Vec<_>>(),
            Err(error) => unavailable_apple_foundation_models(error.to_string()),
        };
        *cached = Some(models.clone());
        models
    } else {
        unavailable_apple_foundation_models("Apple Foundation model cache unavailable".to_string())
    }
}

#[cfg(not(test))]
fn unavailable_apple_foundation_models(reason: String) -> Vec<AppleFoundationModelStatus> {
    APPLE_FOUNDATION_MODEL_DEFINITIONS
        .iter()
        .map(
            |(id, display_name, model_name)| AppleFoundationModelStatus {
                id: (*id).to_string(),
                display_name: (*display_name).to_string(),
                model_name: (*model_name).to_string(),
                available: false,
                reason: reason.clone(),
            },
        )
        .collect()
}

fn invalidate_apple_foundation_model_cache() {
    if let Ok(mut cache) = APPLE_FOUNDATION_MODELS
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *cache = None;
    }
}

fn download_and_install_apple_speech_model(id: &str) -> Result<()> {
    let helper = crate::apple_helper::helper_path()?;
    let input = crate::apple_helper::speech_model_request_json(id)?;
    let mut child = Command::new(&helper)
        .arg("install-speech-model")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to start Apple helper at {}", helper.display()))?;

    let mut stdin = child
        .stdin
        .take()
        .context("failed to open Apple helper stdin")?;
    stdin
        .write_all(&input)
        .context("failed to write Apple Speech install request")?;
    drop(stdin);

    let stdout = child
        .stdout
        .take()
        .context("failed to open Apple helper stdout")?;
    let mut stderr = child
        .stderr
        .take()
        .context("failed to open Apple helper stderr")?;
    let child_handle = Arc::new(Mutex::new(Some(child)));
    set_apple_speech_download_child(id, child_handle.clone());

    let mut saw_finished = false;
    let mut helper_error = None;
    for line in BufReader::new(stdout).lines() {
        let line = line.context("failed to read Apple Speech install progress")?;
        if line.trim().is_empty() {
            continue;
        }

        let progress =
            serde_json::from_str::<crate::apple_helper::AppleSpeechInstallProgress>(line.trim())
                .with_context(|| {
                    format!("failed to parse Apple Speech install progress: {line}")
                })?;

        if !progress.model_id.is_empty() && progress.model_id != id {
            continue;
        }

        if !progress.ok {
            helper_error = Some(
                progress
                    .error
                    .unwrap_or_else(|| "Apple Speech install failed".to_string()),
            );
            break;
        }

        if progress.event == "finished" {
            saw_finished = true;
            set_apple_speech_download_state(
                id,
                AppleSpeechInstallState::Downloading {
                    progress: Some(1.0),
                },
            );
        } else if progress.event == "progress" {
            let fraction = progress
                .fraction_completed
                .map(|value| value.clamp(0.0, 1.0))
                .or_else(|| {
                    let total = progress.total_unit_count?;
                    let completed = progress.completed_unit_count?;
                    (total > 0).then_some((completed as f64 / total as f64).clamp(0.0, 1.0))
                });
            set_apple_speech_download_state(
                id,
                AppleSpeechInstallState::Downloading { progress: fraction },
            );
        }
    }

    let mut child = child_handle
        .lock()
        .ok()
        .and_then(|mut locked| locked.take())
        .context("Apple Speech helper process handle was unavailable")?;
    let status = child.wait().context("failed to wait for Apple helper")?;
    let mut stderr_output = String::new();
    let _ = stderr.read_to_string(&mut stderr_output);
    if is_apple_speech_download_cancelled(id) {
        anyhow::bail!("download cancelled");
    }
    if !status.success() {
        anyhow::bail!(
            "{}",
            crate::apple_helper::helper_failure_message(
                "install-speech-model",
                &status,
                &stderr_output
            )
        );
    }
    if let Some(error) = helper_error {
        anyhow::bail!("{error}");
    }
    anyhow::ensure!(
        status.success() && saw_finished,
        "Apple Speech install did not complete"
    );

    Ok(())
}

fn apple_speech_download_state(id: &str) -> Option<AppleSpeechInstallState> {
    APPLE_SPEECH_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|downloads| downloads.get(id).cloned())
}

fn set_apple_speech_download_state(id: &str, state: AppleSpeechInstallState) {
    if let Ok(mut downloads) = APPLE_SPEECH_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        downloads.insert(id.to_string(), state);
    }
}

fn clear_apple_speech_download_state(id: &str) {
    if let Ok(mut downloads) = APPLE_SPEECH_DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        downloads.remove(id);
    }
}

fn mark_apple_speech_download_cancelled(id: &str) {
    if let Ok(mut cancellations) = APPLE_SPEECH_DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        cancellations.insert(id.to_string());
    }
}

fn clear_apple_speech_download_cancellation(id: &str) {
    if let Ok(mut cancellations) = APPLE_SPEECH_DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        cancellations.remove(id);
    }
}

fn is_apple_speech_download_cancelled(id: &str) -> bool {
    APPLE_SPEECH_DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map(|cancellations| cancellations.contains(id))
        .unwrap_or(false)
}

fn set_apple_speech_download_child(id: &str, child: Arc<Mutex<Option<Child>>>) {
    if let Ok(mut children) = APPLE_SPEECH_DOWNLOAD_CHILDREN
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        children.insert(id.to_string(), child);
    }
}

fn clear_apple_speech_download_child(id: &str) {
    if let Ok(mut children) = APPLE_SPEECH_DOWNLOAD_CHILDREN
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        children.remove(id);
    }
}

fn kill_apple_speech_download_child(id: &str) {
    let child = APPLE_SPEECH_DOWNLOAD_CHILDREN
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|children| children.get(id).cloned());

    if let Some(child) = child
        && let Ok(mut locked) = child.lock()
        && let Some(process) = locked.as_mut()
    {
        let _ = process.kill();
    }
}

pub fn parakeet_definition(id: &str) -> Option<&'static ParakeetModelDefinition> {
    PARAKEET_MODELS.iter().find(|model| model.id == id)
}

pub fn parakeet_models_status() -> Vec<ParakeetModelStatus> {
    PARAKEET_MODELS
        .iter()
        .map(|definition| ParakeetModelStatus {
            definition: *definition,
            state: parakeet_install_state(definition.id),
        })
        .collect()
}

pub fn parakeet_install_state(id: &str) -> LocalModelInstallState {
    if let Some(state) = download_state(id) {
        return state;
    }

    let Ok(dir) = parakeet_model_dir(id) else {
        return LocalModelInstallState::NotInstalled;
    };

    if validate_parakeet_model_dir(&dir).is_ok() {
        LocalModelInstallState::Installed {
            size_bytes: directory_size(&dir).unwrap_or(0),
        }
    } else {
        LocalModelInstallState::NotInstalled
    }
}

pub fn parakeet_model_dir(id: &str) -> Result<PathBuf> {
    let definition =
        parakeet_definition(id).with_context(|| format!("unknown Parakeet model: {id}"))?;
    Ok(local_models_dir()?.join(definition.id))
}

pub fn validate_parakeet_model_dir(dir: &Path) -> Result<()> {
    anyhow::ensure!(dir.is_dir(), "model directory does not exist");
    for file in REQUIRED_PARAKEET_FILES {
        let path = dir.join(file);
        anyhow::ensure!(path.is_file(), "missing required model file: {file}");
    }
    Ok(())
}

pub fn start_parakeet_download(id: &str) -> Result<()> {
    let definition =
        *parakeet_definition(id).with_context(|| format!("unknown Parakeet model: {id}"))?;

    if matches!(
        download_state(definition.id),
        Some(
            LocalModelInstallState::Downloading { .. } | LocalModelInstallState::Cancelling { .. }
        )
    ) {
        return Ok(());
    }

    clear_download_cancellation(definition.id);
    set_download_state(
        definition.id,
        LocalModelInstallState::Downloading {
            downloaded_bytes: 0,
            total_bytes: None,
        },
    );

    thread::spawn(move || {
        if let Err(error) = download_and_install_parakeet(definition) {
            if is_download_cancelled(definition.id) {
                clear_download_state(definition.id);
            } else {
                set_download_state(
                    definition.id,
                    LocalModelInstallState::Failed(error.to_string()),
                );
            }
        } else {
            clear_download_state(definition.id);
        }
        clear_download_cancellation(definition.id);
    });

    Ok(())
}

pub fn cancel_parakeet_download(id: &str) -> Result<()> {
    parakeet_definition(id).with_context(|| format!("unknown Parakeet model: {id}"))?;

    match download_state(id) {
        Some(LocalModelInstallState::Downloading {
            downloaded_bytes,
            total_bytes,
        }) => {
            mark_download_cancelled(id);
            set_download_state(
                id,
                LocalModelInstallState::Cancelling {
                    downloaded_bytes,
                    total_bytes,
                },
            );
        }
        Some(LocalModelInstallState::Cancelling { .. }) => {
            mark_download_cancelled(id);
        }
        _ => {}
    }

    Ok(())
}

pub fn delete_parakeet_model(id: &str) -> Result<()> {
    if matches!(
        download_state(id),
        Some(
            LocalModelInstallState::Downloading { .. } | LocalModelInstallState::Cancelling { .. }
        )
    ) {
        cancel_parakeet_download(id)?;
    } else {
        clear_download_state(id);
    }

    let dir = parakeet_model_dir(id)?;
    if dir.exists() {
        fs::remove_dir_all(&dir)
            .with_context(|| format!("failed to delete local model at {}", dir.display()))?;
    }
    Ok(())
}

fn download_and_install_parakeet(definition: ParakeetModelDefinition) -> Result<()> {
    let root = local_models_dir()?;
    fs::create_dir_all(&root).context("failed to create local model directory")?;

    let install_dir = root.join(definition.id);
    let temp_dir = root.join(format!(
        ".{}-download-{}",
        definition.id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    ));
    let archive_path = temp_dir.join(definition.archive_name);

    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).ok();
    }
    fs::create_dir_all(&temp_dir).context("failed to create temporary model directory")?;

    let result = (|| -> Result<()> {
        check_download_cancelled(definition.id)?;
        download_archive(definition, &archive_path)?;
        check_download_cancelled(definition.id)?;
        safe_extract_tar_bz2_checked(&archive_path, &temp_dir, || {
            check_download_cancelled(definition.id)
        })?;
        fs::remove_file(&archive_path).ok();
        check_download_cancelled(definition.id)?;
        validate_parakeet_model_dir(&temp_dir)?;

        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).with_context(|| {
                format!(
                    "failed to replace existing model at {}",
                    install_dir.display()
                )
            })?;
        }
        fs::rename(&temp_dir, &install_dir).with_context(|| {
            format!(
                "failed to move model from {} to {}",
                temp_dir.display(),
                install_dir.display()
            )
        })?;
        Ok(())
    })();

    if result.is_err() {
        fs::remove_dir_all(&temp_dir).ok();
    }

    result
}

fn download_archive(definition: ParakeetModelDefinition, archive_path: &Path) -> Result<()> {
    let mut response = reqwest::blocking::get(definition.url)
        .with_context(|| format!("failed to start model download from {}", definition.url))?
        .error_for_status()
        .context("model download returned an error status")?;
    let total = response.content_length();
    let mut file = File::create(archive_path).with_context(|| {
        format!(
            "failed to create model archive at {}",
            archive_path.display()
        )
    })?;

    let mut downloaded = 0u64;
    let mut buffer = [0u8; 1024 * 256];
    loop {
        check_download_cancelled(definition.id)?;
        let read = response
            .read(&mut buffer)
            .context("failed while reading model download")?;
        if read == 0 {
            break;
        }
        check_download_cancelled(definition.id)?;
        file.write_all(&buffer[..read])
            .context("failed while writing model archive")?;
        downloaded += read as u64;
        check_download_cancelled(definition.id)?;
        set_download_state(
            definition.id,
            LocalModelInstallState::Downloading {
                downloaded_bytes: downloaded,
                total_bytes: total,
            },
        );
    }

    Ok(())
}

#[cfg(test)]
fn safe_extract_tar_bz2(archive_path: &Path, destination: &Path) -> Result<()> {
    safe_extract_tar_bz2_checked(archive_path, destination, || Ok(()))
}

fn safe_extract_tar_bz2_checked(
    archive_path: &Path,
    destination: &Path,
    mut check_cancelled: impl FnMut() -> Result<()>,
) -> Result<()> {
    let archive_file = File::open(archive_path)
        .with_context(|| format!("failed to open model archive {}", archive_path.display()))?;
    let decoder = BzDecoder::new(archive_file);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries().context("failed to read model archive")? {
        check_cancelled()?;
        let mut entry = entry.context("failed to read model archive entry")?;
        let entry_path = entry.path().context("failed to read model archive path")?;
        let safe_path = strip_archive_root(&entry_path)?;

        if safe_path.as_os_str().is_empty() {
            continue;
        }

        let out_path = destination.join(safe_path);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&out_path)
                .with_context(|| format!("failed to create {}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            entry
                .unpack(&out_path)
                .with_context(|| format!("failed to unpack {}", out_path.display()))?;
        }
        check_cancelled()?;
    }

    Ok(())
}

fn strip_archive_root(path: &Path) -> Result<PathBuf> {
    let mut components = path.components();
    let _root = components.next();
    let mut stripped = PathBuf::new();

    for component in components {
        match component {
            Component::Normal(part) => stripped.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("archive contains an unsafe path: {}", path.display());
            }
        }
    }

    Ok(stripped)
}

fn directory_size(path: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

fn download_state(id: &str) -> Option<LocalModelInstallState> {
    DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|downloads| downloads.get(id).cloned())
}

fn set_download_state(id: &str, state: LocalModelInstallState) {
    if let Ok(mut downloads) = DOWNLOADS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        downloads.insert(id.to_string(), state);
    }
}

fn clear_download_state(id: &str) {
    if let Ok(mut downloads) = DOWNLOADS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        downloads.remove(id);
    }
}

fn mark_download_cancelled(id: &str) {
    if let Ok(mut cancellations) = DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        cancellations.insert(id.to_string());
    }
}

fn clear_download_cancellation(id: &str) {
    if let Ok(mut cancellations) = DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        cancellations.remove(id);
    }
}

fn is_download_cancelled(id: &str) -> bool {
    DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .map(|cancellations| cancellations.contains(id))
        .unwrap_or(false)
}

fn check_download_cancelled(id: &str) -> Result<()> {
    anyhow::ensure!(!is_download_cancelled(id), "download cancelled");
    Ok(())
}

fn local_models_dir() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("GLIDE_LOCAL_MODELS_DIR") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Glide")
        .join("Models"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bzip2::{Compression, write::BzEncoder};
    use std::io::Cursor;
    use tar::Builder;
    use tempfile::tempdir;

    #[test]
    fn validates_required_parakeet_files() {
        let dir = tempdir().unwrap();
        for file in REQUIRED_PARAKEET_FILES {
            fs::write(dir.path().join(file), "x").unwrap();
        }

        validate_parakeet_model_dir(dir.path()).unwrap();
        fs::remove_file(dir.path().join("tokens.txt")).unwrap();
        assert!(validate_parakeet_model_dir(dir.path()).is_err());
    }

    #[test]
    fn strips_archive_root_and_rejects_unsafe_paths() {
        assert_eq!(
            strip_archive_root(Path::new("root/encoder.int8.onnx")).unwrap(),
            PathBuf::from("encoder.int8.onnx")
        );
        assert!(strip_archive_root(Path::new("root/../bad")).is_err());
    }

    #[test]
    fn extracts_archive_without_top_level_directory() {
        let source = make_test_archive(&[
            ("model/encoder.int8.onnx", "encoder"),
            ("model/decoder.int8.onnx", "decoder"),
            ("model/joiner.int8.onnx", "joiner"),
            ("model/tokens.txt", "tokens"),
        ]);
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("model.tar.bz2");
        fs::write(&archive_path, source).unwrap();
        let dest = dir.path().join("dest");
        fs::create_dir(&dest).unwrap();

        safe_extract_tar_bz2(&archive_path, &dest).unwrap();
        validate_parakeet_model_dir(&dest).unwrap();
        assert!(dest.join("encoder.int8.onnx").is_file());
        assert!(!dest.join("model").exists());
    }

    #[test]
    fn cancel_marks_active_download_as_cancelling() {
        let id = PARAKEET_MODELS[0].id;
        clear_download_state(id);
        clear_download_cancellation(id);

        set_download_state(
            id,
            LocalModelInstallState::Downloading {
                downloaded_bytes: 1024,
                total_bytes: Some(2048),
            },
        );

        cancel_parakeet_download(id).unwrap();

        assert_eq!(
            download_state(id),
            Some(LocalModelInstallState::Cancelling {
                downloaded_bytes: 1024,
                total_bytes: Some(2048),
            })
        );
        assert!(is_download_cancelled(id));

        clear_download_state(id);
        clear_download_cancellation(id);
    }

    #[test]
    fn apple_speech_model_ids_round_trip_locale() {
        let id = apple_speech_model_id("en_US");
        assert_eq!(id, "speechanalyzer-en_US");
        assert_eq!(apple_speech_locale_id(&id), Some("en_US"));
        assert!(apple_speech_locale_id(APPLE_SPEECH_MODEL_ID).is_none());
        assert!(is_legacy_apple_speech_model(APPLE_SPEECH_MODEL_ID));
    }

    #[test]
    fn legacy_apple_speech_model_resolves_to_first_installed_locale() {
        assert_eq!(
            resolve_apple_speech_model_id(APPLE_SPEECH_MODEL_ID),
            Some("speechanalyzer-en_US".to_string())
        );
    }

    #[test]
    fn apple_speech_status_uses_reservation_as_installed_state() {
        let statuses = apple_speech_models_status();
        let installed = statuses
            .iter()
            .find(|status| status.definition.id == "speechanalyzer-en_US")
            .unwrap();
        let available = statuses
            .iter()
            .find(|status| status.definition.id == "speechanalyzer-fr_FR")
            .unwrap();

        assert_eq!(installed.state, AppleSpeechInstallState::Installed);
        assert_eq!(available.state, AppleSpeechInstallState::NotInstalled);
    }

    #[test]
    fn apple_speech_unavailable_reason_is_cleared_by_refresh() {
        set_apple_speech_models_unavailable_reason(Some("permission denied".to_string()));
        assert_eq!(
            apple_speech_models_unavailable_reason(),
            Some("permission denied".to_string())
        );

        refresh_apple_local_models();

        assert_eq!(apple_speech_models_unavailable_reason(), None);
    }

    #[test]
    fn apple_foundation_models_report_default_model_only() {
        let statuses = apple_foundation_models_status();
        let model = statuses
            .iter()
            .find(|status| status.id == APPLE_FOUNDATION_MODEL_ID)
            .unwrap();

        assert_eq!(statuses.len(), 1);
        assert!(model.available);
        assert_eq!(
            resolve_apple_foundation_model_id(APPLE_FOUNDATION_MODEL_ID),
            Some(APPLE_FOUNDATION_MODEL_ID.to_string())
        );
        assert_eq!(
            resolve_apple_foundation_model_id("apple-foundation-rewrite"),
            None
        );
    }

    #[test]
    fn cancel_marks_active_apple_speech_download_as_cancelling() {
        let id = "speechanalyzer-fr_FR";
        clear_apple_speech_download_state(id);
        clear_apple_speech_download_cancellation(id);

        set_apple_speech_download_state(
            id,
            AppleSpeechInstallState::Downloading {
                progress: Some(0.5),
            },
        );

        cancel_apple_speech_model_download(id).unwrap();

        assert_eq!(
            apple_speech_download_state(id),
            Some(AppleSpeechInstallState::Cancelling)
        );
        assert!(is_apple_speech_download_cancelled(id));

        clear_apple_speech_download_state(id);
        clear_apple_speech_download_cancellation(id);
    }

    #[test]
    #[ignore = "requires macOS 26 Apple Speech assets; set GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID=speechanalyzer-<locale>"]
    fn attempts_real_apple_speech_download_then_cancel() {
        let id = std::env::var("GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID")
            .expect("set GLIDE_TEST_APPLE_SPEECH_CANCEL_MODEL_ID=speechanalyzer-<locale>");
        assert!(
            apple_speech_locale_id(&id).is_some(),
            "test model id must use speechanalyzer-<locale>"
        );

        clear_apple_speech_download_state(&id);
        clear_apple_speech_download_cancellation(&id);
        clear_apple_speech_download_child(&id);

        start_apple_speech_model_download(&id).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(750));
        cancel_apple_speech_model_download(&id).unwrap();

        let state = apple_speech_download_state(&id);
        assert!(
            matches!(state, Some(AppleSpeechInstallState::Cancelling)) || state.is_none(),
            "expected cancellation to be requested or the OS-managed install to finish quickly; got {state:?}"
        );

        for _ in 0..20 {
            if !apple_speech_has_active_downloads() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        assert!(
            !matches!(
                apple_speech_download_state(&id),
                Some(AppleSpeechInstallState::Downloading { .. })
            ),
            "Apple Speech download remained active after best-effort cancellation"
        );

        clear_apple_speech_download_state(&id);
        clear_apple_speech_download_cancellation(&id);
        clear_apple_speech_download_child(&id);
    }

    fn make_test_archive(files: &[(&str, &str)]) -> Vec<u8> {
        let mut tar_data = Vec::new();
        {
            let cursor = Cursor::new(&mut tar_data);
            let mut builder = Builder::new(cursor);
            for (path, contents) in files {
                let mut header = tar::Header::new_gnu();
                header.set_size(contents.len() as u64);
                header.set_cksum();
                builder
                    .append_data(&mut header, *path, contents.as_bytes())
                    .unwrap();
            }
            builder.finish().unwrap();
        }

        let mut encoder = BzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&tar_data).unwrap();
        encoder.finish().unwrap()
    }
}

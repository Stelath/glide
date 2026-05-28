use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use bzip2::read::BzDecoder;

pub(super) const REQUIRED_PARAKEET_FILES: [&str; 4] = [
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

static DOWNLOADS: OnceLock<Mutex<HashMap<String, LocalModelInstallState>>> = OnceLock::new();
static DOWNLOAD_CANCELLATIONS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
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
pub(super) fn safe_extract_tar_bz2(archive_path: &Path, destination: &Path) -> Result<()> {
    safe_extract_tar_bz2_checked(archive_path, destination, || Ok(()))
}

pub(super) fn safe_extract_tar_bz2_checked(
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

pub(super) fn strip_archive_root(path: &Path) -> Result<PathBuf> {
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

pub(super) fn download_state(id: &str) -> Option<LocalModelInstallState> {
    DOWNLOADS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|downloads| downloads.get(id).cloned())
}

pub(super) fn set_download_state(id: &str, state: LocalModelInstallState) {
    if let Ok(mut downloads) = DOWNLOADS.get_or_init(|| Mutex::new(HashMap::new())).lock() {
        downloads.insert(id.to_string(), state);
    }
}

#[cfg(test)]
pub(crate) fn set_parakeet_install_state_for_test(id: &str, state: LocalModelInstallState) {
    set_download_state(id, state);
}

pub(super) fn clear_download_state(id: &str) {
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

pub(super) fn clear_download_cancellation(id: &str) {
    if let Ok(mut cancellations) = DOWNLOAD_CANCELLATIONS
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
    {
        cancellations.remove(id);
    }
}

pub(super) fn is_download_cancelled(id: &str) -> bool {
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
    if let Ok(path) = std::env::var("GLIDE_LOCAL_MODELS_DIR")
        && !path.trim().is_empty()
    {
        return Ok(PathBuf::from(path));
    }

    let home = std::env::var_os("HOME").context("HOME is not set")?;
    Ok(PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("Glide")
        .join("Models"))
}

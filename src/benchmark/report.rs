use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};

use crate::config::ProvidersConfig;

use super::{
    Provider, REPORT_SCHEMA_VERSION,
    types::{
        BenchmarkReport, BenchmarkRun, EnvironmentMetadata, PhaseSummary, PromptEvalReport,
        ProviderModelMetadata, ScenarioMetadata, TextSummary,
    },
};

pub(super) fn build_report(
    mode: impl Into<String>,
    scenario: ScenarioMetadata,
    runs: Vec<BenchmarkRun>,
) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: REPORT_SCHEMA_VERSION,
        mode: mode.into(),
        generated_at_unix_ms: unix_millis(),
        environment: environment_metadata(),
        summary: summarize_runs(&runs),
        scenario,
        runs,
    }
}

pub fn summarize_runs(runs: &[BenchmarkRun]) -> Vec<PhaseSummary> {
    let mut durations: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    let mut phases = BTreeSet::new();

    for run in runs.iter().filter(|run| !run.warmup) {
        for span in &run.phases {
            phases.insert(span.phase.clone());
            durations
                .entry(span.phase.clone())
                .or_default()
                .push(span.duration_ms);
        }
        if let Some(error_phase) = &run.error_phase {
            phases.insert(error_phase.clone());
            *errors.entry(error_phase.clone()).or_default() += 1;
        }
    }

    phases
        .into_iter()
        .map(|phase| {
            phase_summary(
                phase.clone(),
                durations.get(&phase).map(Vec::as_slice).unwrap_or(&[]),
                *errors.get(&phase).unwrap_or(&0),
            )
        })
        .collect()
}

pub fn phase_summary(phase: String, samples: &[f64], errors: usize) -> PhaseSummary {
    let mut values = samples
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    values.sort_by(|a, b| a.total_cmp(b));

    let min_ms = values.first().copied().unwrap_or(0.0);
    let max_ms = values.last().copied().unwrap_or(0.0);
    PhaseSummary {
        phase,
        samples: values.len(),
        errors,
        min_ms,
        median_ms: percentile_sorted(&values, 0.50),
        p95_ms: percentile_sorted(&values, 0.95),
        max_ms,
    }
}

fn percentile_sorted(values: &[f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let rank = (percentile.clamp(0.0, 1.0) * values.len() as f64).ceil() as usize;
    values[rank.saturating_sub(1).min(values.len() - 1)]
}

pub(super) fn write_report(report: &BenchmarkReport, output: Option<&Path>) -> Result<PathBuf> {
    let path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_output_path(&report.mode));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory {}", parent.display()))?;
    }
    let json = serde_json::to_string_pretty(report).context("failed to encode benchmark report")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

pub(super) fn write_prompt_eval_report(
    report: &PromptEvalReport,
    output: Option<&Path>,
) -> Result<PathBuf> {
    let path = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| default_output_path("prompt-eval"));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create report directory {}", parent.display()))?;
    }
    let json =
        serde_json::to_string_pretty(report).context("failed to encode prompt eval report")?;
    fs::write(&path, json).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

fn default_output_path(mode: &str) -> PathBuf {
    PathBuf::from("target")
        .join("glide-bench")
        .join(format!("{}-{mode}.json", unix_millis()))
}

pub(super) fn provider_metadata(
    role: &str,
    provider: Provider,
    model: &str,
    providers: &ProvidersConfig,
) -> ProviderModelMetadata {
    ProviderModelMetadata {
        role: role.to_string(),
        provider: provider.label().to_string(),
        model: model.to_string(),
        base_url_host: provider_base_url_host(providers, provider, role, model),
    }
}

pub(super) fn provider_base_url_host(
    providers: &ProvidersConfig,
    provider: Provider,
    role: &str,
    model: &str,
) -> Option<String> {
    match provider {
        Provider::OpenAi
        | Provider::Groq
        | Provider::Cerebras
        | Provider::Fireworks
        | Provider::ElevenLabs => {
            let creds = providers.credentials_for(provider);
            if role == "stt" {
                return redacted_base_url_host(
                    &provider.stt_endpoint_for_model(&creds.base_url, model),
                );
            }
            redacted_base_url_host(&creds.base_url)
        }
        Provider::AppleLocal | Provider::Parakeet => None,
    }
}

pub fn redacted_base_url_host(base_url: &str) -> Option<String> {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let without_path = without_scheme.split('/').next().unwrap_or(without_scheme);
    let without_userinfo = without_path
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(without_path);
    let host = without_userinfo.trim();
    (!host.is_empty()).then(|| host.to_string())
}

pub(super) fn summarize_text(text: &str) -> TextSummary {
    TextSummary {
        char_count: text.chars().count(),
        byte_count: text.len(),
        hash: stable_hash_hex(text),
    }
}

fn stable_hash_hex<T: Hash>(value: T) -> String {
    let mut hasher = StableHasher::default();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Default)]
struct StableHasher(u64);

impl Hasher for StableHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut hash = if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        };
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }

    fn finish(&self) -> u64 {
        if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        }
    }
}

pub(super) fn environment_metadata() -> EnvironmentMetadata {
    EnvironmentMetadata {
        glide_version: env!("CARGO_PKG_VERSION").to_string(),
        git_sha: command_stdout("git", &["rev-parse", "--short", "HEAD"]),
        os: os_version(),
    }
}

fn os_version() -> String {
    if cfg!(target_os = "macos")
        && let Some(version) = command_stdout("sw_vers", &["-productVersion"])
    {
        return format!("macOS {version}");
    }
    std::env::consts::OS.to_string()
}

fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .filter(|stdout| !stdout.is_empty())
}

pub(super) fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

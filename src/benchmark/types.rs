use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Provider;

#[derive(Debug, Clone, PartialEq)]
pub enum BenchCommand {
    Stt(SttBenchOptions),
    Llm(LlmBenchOptions),
    Flow(FlowBenchOptions),
    PromptEval(PromptEvalOptions),
    Compare(CompareOptions),
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SttBenchOptions {
    pub audio: PathBuf,
    pub provider: Provider,
    pub model: String,
    pub runs: usize,
    pub warmups: usize,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LlmBenchOptions {
    pub text: String,
    pub provider: Provider,
    pub model: String,
    pub runs: usize,
    pub warmups: usize,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowBenchOptions {
    pub audio: PathBuf,
    pub target_app: Option<String>,
    pub style: Option<String>,
    pub runs: usize,
    pub warmups: usize,
    pub paste: bool,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptEvalOptions {
    pub suite: PathBuf,
    pub candidates: Vec<PromptEvalCandidate>,
    pub runs: usize,
    pub timeout_secs: u64,
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromptEvalCandidate {
    pub provider: Provider,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareOptions {
    pub baseline: PathBuf,
    pub candidate: PathBuf,
    pub fail_threshold_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub schema_version: u8,
    pub mode: String,
    pub generated_at_unix_ms: u128,
    pub environment: EnvironmentMetadata,
    pub scenario: ScenarioMetadata,
    pub runs: Vec<BenchmarkRun>,
    pub summary: Vec<PhaseSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentMetadata {
    pub glide_version: String,
    pub git_sha: Option<String>,
    pub os: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioMetadata {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub run_count: usize,
    pub warmup_count: usize,
    pub audio: Option<AudioMetadata>,
    pub text: Option<TextSummary>,
    pub target_app: Option<String>,
    pub style: Option<String>,
    pub paste_enabled: bool,
    pub base_url_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioMetadata {
    pub path: String,
    pub byte_count: usize,
    pub sample_count: usize,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextSummary {
    pub char_count: usize,
    pub byte_count: usize,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRun {
    pub index: usize,
    pub warmup: bool,
    pub ok: bool,
    pub error: Option<String>,
    pub error_phase: Option<String>,
    pub phases: Vec<SpanRecord>,
    pub output: Option<TextSummary>,
    pub selections: Vec<ProviderModelMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SpanRecord {
    pub phase: String,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelMetadata {
    pub role: String,
    pub provider: String,
    pub model: String,
    pub base_url_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvalReport {
    pub schema_version: u8,
    pub generated_at_unix_ms: u128,
    pub environment: EnvironmentMetadata,
    pub suite_path: String,
    pub run_count: usize,
    pub timeout_seconds: u64,
    pub candidates: Vec<PromptEvalCandidateReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvalCandidateReport {
    pub provider: String,
    pub model: String,
    pub base_url_host: Option<String>,
    pub summary: PromptEvalSummary,
    pub results: Vec<PromptEvalResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptEvalSummary {
    pub total: usize,
    pub passed: usize,
    pub pass_rate: f64,
    pub tags: Vec<PromptEvalTagSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PromptEvalTagSummary {
    pub tag: String,
    pub total: usize,
    pub passed: usize,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEvalResult {
    pub case_id: String,
    pub run_index: usize,
    pub provider: String,
    pub model: String,
    pub style: String,
    pub tags: Vec<String>,
    pub input: String,
    pub expected: String,
    pub accepted_outputs: Vec<String>,
    pub raw_output: String,
    pub normalized_output: String,
    pub passed: bool,
    pub reason: String,
    pub latency_ms: f64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(super) struct PromptEvalCase {
    pub(super) id: String,
    #[serde(default = "default_prompt_eval_style")]
    pub(super) style: String,
    pub(super) input: String,
    pub(super) expected: String,
    #[serde(default)]
    pub(super) accepted_outputs: Vec<String>,
    #[serde(default)]
    pub(super) forbidden_substrings: Vec<String>,
    #[serde(default)]
    pub(super) tags: Vec<String>,
}

fn default_prompt_eval_style() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PhaseSummary {
    pub phase: String,
    pub samples: usize,
    pub errors: usize,
    pub min_ms: f64,
    pub median_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareResult {
    pub rows: Vec<CompareRow>,
    pub failures: Vec<CompareFailure>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareRow {
    pub phase: String,
    pub median_delta_percent: Option<f64>,
    pub p95_delta_percent: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompareFailure {
    pub phase: String,
    pub metric: String,
    pub delta_percent: f64,
}

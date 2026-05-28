use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    hash::{Hash, Hasher},
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{AudioFormat, RecordedAudio},
    config::{GlideConfig, ModelSelection, ProvidersConfig, ReplacementRule},
    llm::{self, CleanupContext},
    paste, stt,
};

pub use crate::config::Provider;

const DEFAULT_RUNS: usize = 3;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_PROMPT_EVAL_RUNS: usize = 1;
const DEFAULT_PROMPT_EVAL_TIMEOUT_SECS: u64 = 60;
const REPORT_SCHEMA_VERSION: u8 = 1;

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
    pub edit_prepass: bool,
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
    pub edit_prepass: bool,
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
struct PromptEvalCase {
    id: String,
    #[serde(default = "default_prompt_eval_style")]
    style: String,
    input: String,
    expected: String,
    #[serde(default)]
    accepted_outputs: Vec<String>,
    #[serde(default)]
    forbidden_substrings: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
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

#[derive(Clone, Default)]
pub(crate) struct ProfileCollector {
    inner: Option<Arc<ProfileState>>,
}

#[derive(Default)]
struct ProfileState {
    spans: Mutex<Vec<SpanRecord>>,
    markers: Mutex<BTreeMap<String, Instant>>,
}

impl ProfileCollector {
    pub(crate) fn enabled() -> Self {
        Self {
            inner: Some(Arc::new(ProfileState::default())),
        }
    }

    pub(crate) fn disabled() -> Self {
        Self { inner: None }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    pub(crate) fn record(&self, phase: impl Into<String>, duration: Duration) {
        if let Some(inner) = &self.inner
            && let Ok(mut spans) = inner.spans.lock()
        {
            spans.push(SpanRecord {
                phase: phase.into(),
                duration_ms: duration.as_secs_f64() * 1000.0,
            });
        }
    }

    pub(crate) fn measure<T>(&self, phase: &str, f: impl FnOnce() -> T) -> T {
        let started = Instant::now();
        let result = f();
        self.record(phase, started.elapsed());
        result
    }

    pub(crate) fn measure_result<T>(
        &self,
        phase: &str,
        f: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let started = Instant::now();
        let result = f();
        self.record(phase, started.elapsed());
        result
    }

    pub(crate) fn mark(&self, marker: impl Into<String>) {
        if let Some(inner) = &self.inner
            && let Ok(mut markers) = inner.markers.lock()
        {
            markers.insert(marker.into(), Instant::now());
        }
    }

    pub(crate) fn record_since_marker(&self, marker: &str, phase: impl Into<String>) {
        let Some(started) = self.inner.as_ref().and_then(|inner| {
            inner
                .markers
                .lock()
                .ok()
                .and_then(|markers| markers.get(marker).copied())
        }) else {
            return;
        };
        self.record(phase, started.elapsed());
    }

    pub(crate) fn spans(&self) -> Vec<SpanRecord> {
        self.inner
            .as_ref()
            .and_then(|inner| inner.spans.lock().ok().map(|spans| spans.clone()))
            .unwrap_or_default()
    }
}

pub fn run_cli() -> Result<()> {
    match parse_cli_args(std::env::args())? {
        BenchCommand::Stt(options) => {
            let (report, path) = run_stt_benchmark(&options)?;
            print_report_summary(&report);
            println!("report: {}", path.display());
        }
        BenchCommand::Llm(options) => {
            let (report, path) = run_llm_benchmark(&options)?;
            print_report_summary(&report);
            println!("report: {}", path.display());
        }
        BenchCommand::Flow(options) => {
            let (report, path) = run_flow_benchmark(&options)?;
            print_report_summary(&report);
            println!("report: {}", path.display());
        }
        BenchCommand::PromptEval(options) => {
            let (report, path) = run_prompt_eval(&options)?;
            print_prompt_eval_summary(&report);
            println!("report: {}", path.display());
        }
        BenchCommand::Compare(options) => {
            let result = compare_report_files(&options)?;
            print_compare_result(&result);
            if !result.failures.is_empty() {
                anyhow::bail!(
                    "{} benchmark regression(s) exceeded {:.2}%",
                    result.failures.len(),
                    options.fail_threshold_percent
                );
            }
        }
        BenchCommand::Help => print_usage(),
    }
    Ok(())
}

pub fn parse_cli_args<I, S>(args: I) -> Result<BenchCommand>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into).collect::<VecDeque<_>>();
    let _program = args.pop_front();
    let Some(command) = args.pop_front() else {
        return Ok(BenchCommand::Help);
    };

    if command == "-h" || command == "--help" || command == "help" {
        return Ok(BenchCommand::Help);
    }

    match command.as_str() {
        "stt" => parse_stt_args(args).map(BenchCommand::Stt),
        "llm" => parse_llm_args(args).map(BenchCommand::Llm),
        "flow" => parse_flow_args(args).map(BenchCommand::Flow),
        "prompt-eval" => parse_prompt_eval_args(args).map(BenchCommand::PromptEval),
        "compare" => parse_compare_args(args).map(BenchCommand::Compare),
        other => anyhow::bail!("unknown glide-bench command '{other}'\n\n{}", usage()),
    }
}

fn parse_stt_args(mut args: VecDeque<String>) -> Result<SttBenchOptions> {
    let mut audio = None;
    let mut provider = None;
    let mut model = None;
    let mut runs = DEFAULT_RUNS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut output = None;

    while let Some(flag) = args.pop_front() {
        match flag.as_str() {
            "--audio" => audio = Some(PathBuf::from(take_value(&mut args, "--audio")?)),
            "--provider" => provider = Some(parse_provider(&take_value(&mut args, "--provider")?)?),
            "--model" => model = Some(take_value(&mut args, "--model")?),
            "--runs" => runs = parse_usize(&take_value(&mut args, "--runs")?, "--runs")?,
            "--warmups" => {
                warmups = parse_usize(&take_value(&mut args, "--warmups")?, "--warmups")?
            }
            "--output" => output = Some(PathBuf::from(take_value(&mut args, "--output")?)),
            "-h" | "--help" => anyhow::bail!("{}", usage()),
            other => anyhow::bail!("unknown stt option '{other}'\n\n{}", usage()),
        }
    }

    let provider = provider.context("missing required --provider")?;
    if provider == Provider::Cerebras {
        anyhow::bail!("Cerebras does not provide a speech-to-text model");
    }
    Ok(SttBenchOptions {
        audio: audio.context("missing required --audio")?,
        provider,
        model: model.context("missing required --model")?,
        runs,
        warmups,
        output,
    })
}

fn parse_llm_args(mut args: VecDeque<String>) -> Result<LlmBenchOptions> {
    let mut text = None;
    let mut provider = None;
    let mut model = None;
    let mut runs = DEFAULT_RUNS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut output = None;

    while let Some(flag) = args.pop_front() {
        match flag.as_str() {
            "--text" => text = Some(take_value(&mut args, "--text")?),
            "--text-file" => {
                let path = take_value(&mut args, "--text-file")?;
                text = Some(format!("@{path}"));
            }
            "--provider" => provider = Some(parse_provider(&take_value(&mut args, "--provider")?)?),
            "--model" => model = Some(take_value(&mut args, "--model")?),
            "--runs" => runs = parse_usize(&take_value(&mut args, "--runs")?, "--runs")?,
            "--warmups" => {
                warmups = parse_usize(&take_value(&mut args, "--warmups")?, "--warmups")?
            }
            "--output" => output = Some(PathBuf::from(take_value(&mut args, "--output")?)),
            "-h" | "--help" => anyhow::bail!("{}", usage()),
            other => anyhow::bail!("unknown llm option '{other}'\n\n{}", usage()),
        }
    }

    let provider = provider.context("missing required --provider")?;
    if matches!(provider, Provider::Parakeet | Provider::ElevenLabs) {
        anyhow::bail!("{} does not provide an LLM cleanup model", provider.label());
    }
    Ok(LlmBenchOptions {
        text: text.context("missing required --text")?,
        provider,
        model: model.context("missing required --model")?,
        runs,
        warmups,
        output,
    })
}

fn parse_flow_args(mut args: VecDeque<String>) -> Result<FlowBenchOptions> {
    let mut audio = None;
    let mut target_app = None;
    let mut style = None;
    let mut runs = DEFAULT_RUNS;
    let mut warmups = DEFAULT_WARMUPS;
    let mut paste = false;
    let mut output = None;

    while let Some(flag) = args.pop_front() {
        match flag.as_str() {
            "--audio" => audio = Some(PathBuf::from(take_value(&mut args, "--audio")?)),
            "--target-app" => target_app = Some(take_value(&mut args, "--target-app")?),
            "--style" => {
                let value = take_value(&mut args, "--style")?;
                style = (value != "default").then_some(value);
            }
            "--runs" => runs = parse_usize(&take_value(&mut args, "--runs")?, "--runs")?,
            "--warmups" => {
                warmups = parse_usize(&take_value(&mut args, "--warmups")?, "--warmups")?
            }
            "--paste" => paste = true,
            "--no-paste" => paste = false,
            "--output" => output = Some(PathBuf::from(take_value(&mut args, "--output")?)),
            "-h" | "--help" => anyhow::bail!("{}", usage()),
            other => anyhow::bail!("unknown flow option '{other}'\n\n{}", usage()),
        }
    }

    Ok(FlowBenchOptions {
        audio: audio.context("missing required --audio")?,
        target_app,
        style,
        runs,
        warmups,
        paste,
        output,
    })
}

fn parse_prompt_eval_args(mut args: VecDeque<String>) -> Result<PromptEvalOptions> {
    let mut suite = None;
    let mut candidates = Vec::new();
    let mut runs = DEFAULT_PROMPT_EVAL_RUNS;
    let mut timeout_secs = DEFAULT_PROMPT_EVAL_TIMEOUT_SECS;
    let mut edit_prepass = true;
    let mut output = None;

    while let Some(flag) = args.pop_front() {
        match flag.as_str() {
            "--suite" => suite = Some(PathBuf::from(take_value(&mut args, "--suite")?)),
            "--candidate" => candidates.push(parse_prompt_eval_candidate(&take_value(
                &mut args,
                "--candidate",
            )?)?),
            "--runs" => runs = parse_usize(&take_value(&mut args, "--runs")?, "--runs")?,
            "--timeout-secs" => {
                timeout_secs =
                    parse_u64(&take_value(&mut args, "--timeout-secs")?, "--timeout-secs")?
            }
            "--no-edit-prepass" => edit_prepass = false,
            "--output" => output = Some(PathBuf::from(take_value(&mut args, "--output")?)),
            "-h" | "--help" => anyhow::bail!("{}", usage()),
            other => anyhow::bail!("unknown prompt-eval option '{other}'\n\n{}", usage()),
        }
    }

    anyhow::ensure!(
        !candidates.is_empty(),
        "missing required --candidate; pass one or more provider:model values"
    );

    Ok(PromptEvalOptions {
        suite: suite.context("missing required --suite")?,
        candidates,
        runs,
        timeout_secs,
        edit_prepass,
        output,
    })
}

fn parse_compare_args(mut args: VecDeque<String>) -> Result<CompareOptions> {
    let mut baseline = None;
    let mut candidate = None;
    let mut fail_threshold_percent = 20.0;

    while let Some(flag) = args.pop_front() {
        match flag.as_str() {
            "--baseline" => baseline = Some(PathBuf::from(take_value(&mut args, "--baseline")?)),
            "--candidate" => candidate = Some(PathBuf::from(take_value(&mut args, "--candidate")?)),
            "--fail-threshold" => {
                fail_threshold_percent = parse_f64(
                    &take_value(&mut args, "--fail-threshold")?,
                    "--fail-threshold",
                )?
            }
            "-h" | "--help" => anyhow::bail!("{}", usage()),
            other => anyhow::bail!("unknown compare option '{other}'\n\n{}", usage()),
        }
    }

    Ok(CompareOptions {
        baseline: baseline.context("missing required --baseline")?,
        candidate: candidate.context("missing required --candidate")?,
        fail_threshold_percent,
    })
}

fn take_value(args: &mut VecDeque<String>, flag: &str) -> Result<String> {
    let value = args
        .pop_front()
        .with_context(|| format!("{flag} requires a value"))?;
    if value.starts_with("--") {
        anyhow::bail!("{flag} requires a value, got option '{value}'");
    }
    Ok(value)
}

fn parse_provider(raw: &str) -> Result<Provider> {
    match raw {
        "openai" | "open_ai" => Ok(Provider::OpenAi),
        "groq" => Ok(Provider::Groq),
        "cerebras" => Ok(Provider::Cerebras),
        "fireworks" => Ok(Provider::Fireworks),
        "elevenlabs" | "eleven_labs" | "eleven-labs" => Ok(Provider::ElevenLabs),
        "apple" | "apple_local" | "apple-local" => Ok(Provider::AppleLocal),
        "parakeet" => Ok(Provider::Parakeet),
        other => anyhow::bail!("unknown provider '{other}'"),
    }
}

fn parse_prompt_eval_candidate(raw: &str) -> Result<PromptEvalCandidate> {
    let (provider, model) = raw
        .split_once(':')
        .with_context(|| format!("candidate '{raw}' must use provider:model format"))?;
    let provider = parse_provider(provider)?;
    anyhow::ensure!(
        !matches!(provider, Provider::Parakeet | Provider::ElevenLabs),
        "{} does not provide an LLM cleanup model",
        provider.label()
    );
    anyhow::ensure!(
        !model.trim().is_empty(),
        "candidate model must not be empty"
    );
    Ok(PromptEvalCandidate {
        provider,
        model: model.trim().to_string(),
    })
}

fn parse_usize(raw: &str, flag: &str) -> Result<usize> {
    let value = raw
        .parse::<usize>()
        .with_context(|| format!("{flag} must be a positive integer"))?;
    anyhow::ensure!(value > 0, "{flag} must be greater than zero");
    Ok(value)
}

fn parse_u64(raw: &str, flag: &str) -> Result<u64> {
    let value = raw
        .parse::<u64>()
        .with_context(|| format!("{flag} must be a positive integer"))?;
    anyhow::ensure!(value > 0, "{flag} must be greater than zero");
    Ok(value)
}

fn parse_f64(raw: &str, flag: &str) -> Result<f64> {
    let value = raw
        .parse::<f64>()
        .with_context(|| format!("{flag} must be a number"))?;
    anyhow::ensure!(value >= 0.0, "{flag} must be non-negative");
    Ok(value)
}

fn run_stt_benchmark(options: &SttBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
    let audio = load_wav_audio(&options.audio)?;
    let config = GlideConfig::load_or_create().context("failed to load Glide config")?;
    let runtime = tokio::runtime::Runtime::new().context("failed to start benchmark runtime")?;
    let scenario = ScenarioMetadata {
        provider: Some(options.provider.label().to_string()),
        model: Some(options.model.clone()),
        run_count: options.runs,
        warmup_count: options.warmups,
        audio: Some(audio.metadata.clone()),
        text: None,
        target_app: None,
        style: None,
        paste_enabled: false,
        base_url_host: provider_base_url_host(
            &config.providers,
            options.provider,
            "stt",
            &options.model,
        ),
    };

    let mut runs = Vec::new();
    for index in 0..(options.warmups + options.runs) {
        let warmup = index < options.warmups;
        runs.push(run_stt_once(
            index, warmup, options, &config, &audio, &runtime,
        ));
    }

    let report = build_report("stt", scenario, runs);
    let path = write_report(&report, options.output.as_deref())?;
    Ok((report, path))
}

fn run_llm_benchmark(options: &LlmBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
    let text = read_text_argument(&options.text)?;
    let text_summary = summarize_text(&text);
    let config = GlideConfig::load_or_create().context("failed to load Glide config")?;
    let runtime = tokio::runtime::Runtime::new().context("failed to start benchmark runtime")?;
    let scenario = ScenarioMetadata {
        provider: Some(options.provider.label().to_string()),
        model: Some(options.model.clone()),
        run_count: options.runs,
        warmup_count: options.warmups,
        audio: None,
        text: Some(text_summary),
        target_app: None,
        style: None,
        paste_enabled: false,
        base_url_host: provider_base_url_host(
            &config.providers,
            options.provider,
            "llm",
            &options.model,
        ),
    };

    let mut runs = Vec::new();
    for index in 0..(options.warmups + options.runs) {
        let warmup = index < options.warmups;
        runs.push(run_llm_once(
            index, warmup, options, &config, &text, &runtime,
        ));
    }

    let report = build_report("llm", scenario, runs);
    let path = write_report(&report, options.output.as_deref())?;
    Ok((report, path))
}

fn run_flow_benchmark(options: &FlowBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
    let audio_metadata = load_wav_audio(&options.audio)?.metadata;
    let runtime = tokio::runtime::Runtime::new().context("failed to start benchmark runtime")?;
    let scenario = ScenarioMetadata {
        provider: None,
        model: None,
        run_count: options.runs,
        warmup_count: options.warmups,
        audio: Some(audio_metadata),
        text: None,
        target_app: options.target_app.clone(),
        style: options
            .style
            .clone()
            .or_else(|| Some("default".to_string())),
        paste_enabled: options.paste,
        base_url_host: None,
    };

    let mut runs = Vec::new();
    for index in 0..(options.warmups + options.runs) {
        let warmup = index < options.warmups;
        runs.push(run_flow_once(index, warmup, options, &runtime));
    }

    let report = build_report("flow", scenario, runs);
    let path = write_report(&report, options.output.as_deref())?;
    Ok((report, path))
}

fn run_prompt_eval(options: &PromptEvalOptions) -> Result<(PromptEvalReport, PathBuf)> {
    let cases = read_prompt_eval_suite(&options.suite)?;
    anyhow::ensure!(
        !cases.is_empty(),
        "prompt eval suite {} did not contain any cases",
        options.suite.display()
    );

    let config = GlideConfig::load_or_create().context("failed to load Glide config")?;
    let runtime = tokio::runtime::Runtime::new().context("failed to start prompt eval runtime")?;
    let mut candidate_reports = Vec::new();

    eprintln!(
        "prompt-eval: loaded {} case(s), {} candidate(s), {} run(s), {}s timeout per case, edit prepass {}",
        cases.len(),
        options.candidates.len(),
        options.runs,
        options.timeout_secs,
        if options.edit_prepass { "on" } else { "off" }
    );

    for (candidate_index, candidate) in options.candidates.iter().enumerate() {
        let mut providers: BTreeMap<String, Box<dyn llm::LlmProvider>> = BTreeMap::new();
        let mut results = Vec::new();
        eprintln!(
            "prompt-eval: candidate {}/{} {}:{}",
            candidate_index + 1,
            options.candidates.len(),
            candidate.provider.label(),
            candidate.model
        );

        for run_index in 0..options.runs {
            for (case_index, case) in cases.iter().enumerate() {
                let style_prompt = prompt_eval_style_prompt(&config, &case.style)?;
                let provider_key = case.style.to_lowercase();
                if !providers.contains_key(&provider_key) {
                    let provider = llm::build_provider(
                        candidate.provider,
                        &candidate.model,
                        style_prompt,
                        &config.providers,
                    )
                    .with_context(|| {
                        format!(
                            "failed to build prompt eval provider {}:{} for style {}",
                            candidate.provider.label(),
                            candidate.model,
                            case.style
                        )
                    })?;
                    providers.insert(provider_key.clone(), provider);
                }

                let provider = providers
                    .get(&provider_key)
                    .context("prompt eval provider cache was unexpectedly empty")?;
                eprintln!(
                    "prompt-eval:   run {}/{} case {}/{} {} [{}]",
                    run_index + 1,
                    options.runs,
                    case_index + 1,
                    cases.len(),
                    case.id,
                    case.style
                );
                let started = Instant::now();
                let output = runtime.block_on(async {
                    tokio::time::timeout(
                        Duration::from_secs(options.timeout_secs),
                        provider.clean(
                            &case.input,
                            &CleanupContext {
                                target_app: None,
                                mode_hint: Some("general dictation".to_string()),
                                apply_edit_preprocessing: options.edit_prepass,
                            },
                        ),
                    )
                    .await
                    .unwrap_or_else(|_| {
                        Err(anyhow::anyhow!(
                            "prompt eval case timed out after {}s",
                            options.timeout_secs
                        ))
                    })
                });
                let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
                let result = prompt_eval_result(case, run_index, candidate, output, latency_ms);
                eprintln!(
                    "prompt-eval:     {} ({:.0} ms): {}",
                    if result.passed { "pass" } else { "fail" },
                    result.latency_ms,
                    result.reason
                );
                results.push(result);
            }
        }

        let summary = summarize_prompt_eval_results(&results);
        candidate_reports.push(PromptEvalCandidateReport {
            provider: candidate.provider.label().to_string(),
            model: candidate.model.clone(),
            base_url_host: provider_base_url_host(
                &config.providers,
                candidate.provider,
                "llm",
                &candidate.model,
            ),
            summary,
            results,
        });
    }

    let report = PromptEvalReport {
        schema_version: REPORT_SCHEMA_VERSION,
        generated_at_unix_ms: unix_millis(),
        environment: environment_metadata(),
        suite_path: options.suite.display().to_string(),
        run_count: options.runs,
        timeout_seconds: options.timeout_secs,
        edit_prepass: options.edit_prepass,
        candidates: candidate_reports,
    };
    let path = write_prompt_eval_report(&report, options.output.as_deref())?;
    Ok((report, path))
}

fn run_stt_once(
    index: usize,
    warmup: bool,
    options: &SttBenchOptions,
    config: &GlideConfig,
    audio: &BenchmarkAudio,
    runtime: &tokio::runtime::Runtime,
) -> BenchmarkRun {
    let collector = ProfileCollector::enabled();
    collector.mark("stt_start");
    let mut error_phase = None;
    let result = (|| -> Result<TextSummary> {
        let vocab_prompt = if config.dictionary.vocabulary.is_empty() {
            None
        } else {
            Some(config.dictionary.vocabulary.join(", "))
        };
        let provider = collector.measure_result("stt_provider_build", || {
            stt::build_profiled_provider(
                options.provider,
                &options.model,
                &config.providers,
                vocab_prompt,
                collector.clone(),
            )
        });
        let provider = match provider {
            Ok(provider) => provider,
            Err(error) => {
                error_phase = Some("stt_provider_build".to_string());
                return Err(error);
            }
        };

        let started = Instant::now();
        let transcript =
            runtime.block_on(provider.transcribe(&audio.recorded.bytes, audio.recorded.format));
        collector.record("stt_call_total", started.elapsed());
        let transcript = match transcript {
            Ok(transcript) => transcript,
            Err(error) => {
                error_phase = Some("stt_call_total".to_string());
                return Err(error);
            }
        };
        Ok(summarize_text(&transcript))
    })();

    build_run(
        index,
        warmup,
        result,
        error_phase,
        collector.spans(),
        vec![provider_metadata(
            "stt",
            options.provider,
            &options.model,
            &config.providers,
        )],
    )
}

fn run_llm_once(
    index: usize,
    warmup: bool,
    options: &LlmBenchOptions,
    config: &GlideConfig,
    text: &str,
    runtime: &tokio::runtime::Runtime,
) -> BenchmarkRun {
    let collector = ProfileCollector::enabled();
    collector.mark("llm_start");
    let mut error_phase = None;
    let result = (|| -> Result<TextSummary> {
        let provider = collector.measure_result("llm_provider_build", || {
            llm::build_profiled_provider(
                options.provider,
                &options.model,
                &config.dictation.system_prompt,
                &config.providers,
                collector.clone(),
            )
        });
        let provider = match provider {
            Ok(provider) => provider,
            Err(error) => {
                error_phase = Some("llm_provider_build".to_string());
                return Err(error);
            }
        };

        let started = Instant::now();
        let cleaned = runtime.block_on(provider.clean(text, &CleanupContext::default()));
        collector.record("llm_call_total", started.elapsed());
        let cleaned = match cleaned {
            Ok(cleaned) => cleaned,
            Err(error) => {
                error_phase = Some("llm_call_total".to_string());
                return Err(error);
            }
        };
        Ok(summarize_text(&cleaned))
    })();

    build_run(
        index,
        warmup,
        result,
        error_phase,
        collector.spans(),
        vec![provider_metadata(
            "llm",
            options.provider,
            &options.model,
            &config.providers,
        )],
    )
}

fn run_flow_once(
    index: usize,
    warmup: bool,
    options: &FlowBenchOptions,
    runtime: &tokio::runtime::Runtime,
) -> BenchmarkRun {
    let collector = ProfileCollector::enabled();
    collector.mark("flow_release");
    let mut selections = Vec::new();
    let mut error_phase = None;

    let result = (|| -> Result<TextSummary> {
        let audio = match collector
            .measure_result("flow_audio_fixture_load", || load_wav_audio(&options.audio))
        {
            Ok(audio) => audio,
            Err(error) => {
                error_phase = Some("flow_audio_fixture_load".to_string());
                return Err(error);
            }
        };

        let config = match collector.measure_result("flow_config_keychain_load", || {
            GlideConfig::load_or_create().context("failed to load Glide config")
        }) {
            Ok(config) => config,
            Err(error) => {
                error_phase = Some("flow_config_keychain_load".to_string());
                return Err(error);
            }
        };

        let resolved = match collector.measure_result("flow_style_model_resolution", || {
            resolve_flow_models(&config, options)
        }) {
            Ok(resolved) => resolved,
            Err(error) => {
                error_phase = Some("flow_style_model_resolution".to_string());
                return Err(error);
            }
        };

        selections.push(provider_metadata(
            "stt",
            resolved.stt.provider,
            &resolved.stt.model,
            &config.providers,
        ));
        if let Some(llm) = &resolved.llm {
            selections.push(provider_metadata(
                "llm",
                llm.provider,
                &llm.model,
                &config.providers,
            ));
        }

        let vocab_prompt = if config.dictionary.vocabulary.is_empty() {
            None
        } else {
            Some(config.dictionary.vocabulary.join(", "))
        };
        let stt_provider = match collector.measure_result("flow_stt_provider_build", || {
            stt::build_profiled_provider(
                resolved.stt.provider,
                &resolved.stt.model,
                &config.providers,
                vocab_prompt,
                collector.clone(),
            )
        }) {
            Ok(provider) => provider,
            Err(error) => {
                error_phase = Some("flow_stt_provider_build".to_string());
                return Err(error);
            }
        };

        let started = Instant::now();
        let raw_text =
            runtime.block_on(stt_provider.transcribe(&audio.recorded.bytes, audio.recorded.format));
        collector.record("flow_stt_call", started.elapsed());
        let raw_text = match raw_text {
            Ok(raw_text) => raw_text,
            Err(error) => {
                error_phase = Some("flow_stt_call".to_string());
                return Err(error);
            }
        };
        collector.mark("flow_stt_result");

        let raw_text = collector.measure("flow_postprocess_replacements", || {
            apply_replacements(&raw_text, &config.dictionary.replacements)
        });

        let cleaned_text = if let Some(llm_selection) = resolved.llm {
            let llm_provider = match collector.measure_result("flow_llm_provider_build", || {
                llm::build_profiled_provider(
                    llm_selection.provider,
                    &llm_selection.model,
                    &resolved.system_prompt,
                    &config.providers,
                    collector.clone(),
                )
            }) {
                Ok(provider) => provider,
                Err(error) => {
                    error_phase = Some("flow_llm_provider_build".to_string());
                    return Err(error);
                }
            };

            let started = Instant::now();
            let cleaned = runtime.block_on(llm_provider.clean(
                &raw_text,
                &CleanupContext {
                    target_app: options.target_app.clone(),
                    mode_hint: Some("general dictation".to_string()),
                    ..CleanupContext::default()
                },
            ));
            collector.record("flow_llm_call", started.elapsed());
            match cleaned {
                Ok(cleaned) => cleaned,
                Err(error) => {
                    error_phase = Some("flow_llm_call".to_string());
                    return Err(error);
                }
            }
        } else {
            raw_text
        };

        let cleaned_text = collector.measure("flow_postprocess_strip_think_tags", || {
            llm::strip_think_tags(&cleaned_text)
        });

        if options.paste
            && let Err(error) = collector.measure_result("flow_paste", || {
                paste::paste_text(&cleaned_text, &config.paste)
            })
        {
            error_phase = Some("flow_paste".to_string());
            return Err(error);
        }

        Ok(summarize_text(&cleaned_text))
    })();

    build_run(
        index,
        warmup,
        result,
        error_phase,
        collector.spans(),
        selections,
    )
}

struct FlowModels {
    stt: ModelSelection,
    llm: Option<ModelSelection>,
    system_prompt: String,
}

fn resolve_flow_models(config: &GlideConfig, options: &FlowBenchOptions) -> Result<FlowModels> {
    let explicit_style = options.style.as_ref().map(|style_name| {
        config
            .dictation
            .styles
            .iter()
            .find(|style| style.name.eq_ignore_ascii_case(style_name))
            .with_context(|| format!("style '{style_name}' was not found"))
    });

    let style = match explicit_style {
        Some(style) => Some(style?),
        None => options.target_app.as_ref().and_then(|target| {
            config.dictation.styles.iter().find(|style| {
                style
                    .apps
                    .iter()
                    .any(|app| app.eq_ignore_ascii_case(target))
            })
        }),
    };

    Ok(FlowModels {
        stt: style
            .and_then(|style| style.stt.clone())
            .unwrap_or_else(|| config.dictation.stt.clone()),
        llm: style
            .and_then(|style| style.llm.clone())
            .or_else(|| config.dictation.llm.clone()),
        system_prompt: style
            .map(|style| style.prompt.clone())
            .unwrap_or_else(|| config.dictation.system_prompt.clone()),
    })
}

fn build_run(
    index: usize,
    warmup: bool,
    result: Result<TextSummary>,
    error_phase: Option<String>,
    phases: Vec<SpanRecord>,
    selections: Vec<ProviderModelMetadata>,
) -> BenchmarkRun {
    match result {
        Ok(output) => BenchmarkRun {
            index,
            warmup,
            ok: true,
            error: None,
            error_phase: None,
            phases,
            output: Some(output),
            selections,
        },
        Err(error) => BenchmarkRun {
            index,
            warmup,
            ok: false,
            error: Some(error.to_string()),
            error_phase: error_phase.or_else(|| Some("run".to_string())),
            phases,
            output: None,
            selections,
        },
    }
}

struct BenchmarkAudio {
    recorded: RecordedAudio,
    metadata: AudioMetadata,
}

fn load_wav_audio(path: &Path) -> Result<BenchmarkAudio> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read audio file {}", path.display()))?;
    let cursor = Cursor::new(&bytes);
    let reader = hound::WavReader::new(cursor)
        .with_context(|| format!("failed to parse WAV file {}", path.display()))?;
    let spec = reader.spec();
    let sample_count = reader.duration() as usize;
    let frame_count = sample_count / usize::from(spec.channels.max(1));
    let duration_seconds = if spec.sample_rate == 0 {
        0.0
    } else {
        frame_count as f64 / f64::from(spec.sample_rate)
    };

    let recorded = RecordedAudio {
        bytes,
        format: AudioFormat::Wav,
        sample_count: frame_count,
    };

    Ok(BenchmarkAudio {
        metadata: AudioMetadata {
            path: path.display().to_string(),
            byte_count: recorded.bytes.len(),
            sample_count: frame_count,
            duration_seconds,
        },
        recorded,
    })
}

fn read_text_argument(value: &str) -> Result<String> {
    if let Some(path) = value.strip_prefix('@') {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read text file {path}"));
    }

    let path = Path::new(value);
    if path.is_file() {
        return fs::read_to_string(path)
            .with_context(|| format!("failed to read text file {}", path.display()));
    }

    Ok(value.to_string())
}

fn read_prompt_eval_suite(path: &Path) -> Result<Vec<PromptEvalCase>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read prompt eval suite {}", path.display()))?;
    let mut cases = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let case: PromptEvalCase = serde_json::from_str(trimmed).with_context(|| {
            format!(
                "failed to parse prompt eval suite {} line {}",
                path.display(),
                index + 1
            )
        })?;
        validate_prompt_eval_case(&case, index + 1)?;
        cases.push(case);
    }
    Ok(cases)
}

fn validate_prompt_eval_case(case: &PromptEvalCase, line: usize) -> Result<()> {
    anyhow::ensure!(
        !case.id.trim().is_empty(),
        "prompt eval case on line {line} has an empty id"
    );
    anyhow::ensure!(
        !case.style.trim().is_empty(),
        "prompt eval case {} has an empty style",
        case.id
    );
    anyhow::ensure!(
        !case.input.trim().is_empty(),
        "prompt eval case {} has an empty input",
        case.id
    );
    Ok(())
}

fn prompt_eval_style_prompt<'a>(config: &'a GlideConfig, style: &str) -> Result<&'a str> {
    if style.eq_ignore_ascii_case("default") {
        return Ok(&config.dictation.system_prompt);
    }

    config
        .dictation
        .styles
        .iter()
        .find(|candidate| candidate.name.eq_ignore_ascii_case(style))
        .map(|style| style.prompt.as_str())
        .with_context(|| format!("prompt eval style '{style}' was not found"))
}

fn prompt_eval_result(
    case: &PromptEvalCase,
    run_index: usize,
    candidate: &PromptEvalCandidate,
    output: Result<String>,
    latency_ms: f64,
) -> PromptEvalResult {
    match output {
        Ok(raw_output) => {
            let stripped_output = llm::strip_think_tags(&raw_output);
            let normalized_output = normalize_prompt_eval_text(&stripped_output);
            let (passed, reason) =
                score_prompt_eval_output(case, &normalized_output, &stripped_output);
            PromptEvalResult {
                case_id: case.id.clone(),
                run_index,
                provider: candidate.provider.label().to_string(),
                model: candidate.model.clone(),
                style: case.style.clone(),
                tags: case.tags.clone(),
                input: case.input.clone(),
                expected: case.expected.clone(),
                accepted_outputs: case.accepted_outputs.clone(),
                raw_output,
                normalized_output,
                passed,
                reason,
                latency_ms,
                error: None,
            }
        }
        Err(error) => PromptEvalResult {
            case_id: case.id.clone(),
            run_index,
            provider: candidate.provider.label().to_string(),
            model: candidate.model.clone(),
            style: case.style.clone(),
            tags: case.tags.clone(),
            input: case.input.clone(),
            expected: case.expected.clone(),
            accepted_outputs: case.accepted_outputs.clone(),
            raw_output: String::new(),
            normalized_output: String::new(),
            passed: false,
            reason: format!("provider error: {error}"),
            latency_ms,
            error: Some(error.to_string()),
        },
    }
}

fn score_prompt_eval_output(
    case: &PromptEvalCase,
    normalized_output: &str,
    raw_output: &str,
) -> (bool, String) {
    let lowercase_raw = raw_output.to_lowercase();
    for forbidden in &case.forbidden_substrings {
        let forbidden = forbidden.trim();
        if !forbidden.is_empty() && lowercase_raw.contains(&forbidden.to_lowercase()) {
            return (false, format!("forbidden substring present: {forbidden}"));
        }
    }

    let accepted = accepted_prompt_eval_outputs(case);
    if accepted
        .iter()
        .any(|accepted| normalized_output == accepted.as_str())
    {
        (true, "ok".to_string())
    } else {
        (
            false,
            format!(
                "expected one of [{}], got '{normalized_output}'",
                accepted
                    .iter()
                    .map(|value| format!("'{value}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    }
}

fn normalize_prompt_eval_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn accepted_prompt_eval_outputs(case: &PromptEvalCase) -> Vec<String> {
    std::iter::once(case.expected.as_str())
        .chain(case.accepted_outputs.iter().map(String::as_str))
        .map(normalize_prompt_eval_text)
        .collect()
}

fn summarize_prompt_eval_results(results: &[PromptEvalResult]) -> PromptEvalSummary {
    let total = results.len();
    let passed = results.iter().filter(|result| result.passed).count();
    let mut tags: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for result in results {
        for tag in &result.tags {
            let entry = tags.entry(tag.clone()).or_default();
            entry.0 += 1;
            if result.passed {
                entry.1 += 1;
            }
        }
    }

    PromptEvalSummary {
        total,
        passed,
        pass_rate: pass_rate(passed, total),
        tags: tags
            .into_iter()
            .map(|(tag, (total, passed))| PromptEvalTagSummary {
                tag,
                total,
                passed,
                pass_rate: pass_rate(passed, total),
            })
            .collect(),
    }
}

fn pass_rate(passed: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        passed as f64 / total as f64
    }
}

fn apply_replacements(text: &str, replacements: &[ReplacementRule]) -> String {
    let mut result = text.to_string();
    for rule in replacements {
        if rule.find.is_empty() {
            continue;
        }
        if rule.case_sensitive {
            result = result.replace(&rule.find, &rule.replace);
        } else {
            let mut output = String::with_capacity(result.len());
            let lower_find = rule.find.to_lowercase();
            let mut search_start = 0;
            let lower_result = result.to_lowercase();
            while let Some(pos) = lower_result[search_start..].find(&lower_find) {
                let abs_pos = search_start + pos;
                output.push_str(&result[search_start..abs_pos]);
                output.push_str(&rule.replace);
                search_start = abs_pos + rule.find.len();
            }
            output.push_str(&result[search_start..]);
            result = output;
        }
    }
    result
}

fn build_report(
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

fn write_report(report: &BenchmarkReport, output: Option<&Path>) -> Result<PathBuf> {
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

fn write_prompt_eval_report(report: &PromptEvalReport, output: Option<&Path>) -> Result<PathBuf> {
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

pub fn compare_report_files(options: &CompareOptions) -> Result<CompareResult> {
    let baseline = read_report(&options.baseline)?;
    let candidate = read_report(&options.candidate)?;
    Ok(compare_reports(
        &baseline,
        &candidate,
        options.fail_threshold_percent,
    ))
}

fn read_report(path: &Path) -> Result<BenchmarkReport> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read benchmark report {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
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

pub fn compare_reports(
    baseline: &BenchmarkReport,
    candidate: &BenchmarkReport,
    fail_threshold_percent: f64,
) -> CompareResult {
    let baseline_by_phase = baseline
        .summary
        .iter()
        .map(|summary| (summary.phase.as_str(), summary))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_phase = candidate
        .summary
        .iter()
        .map(|summary| (summary.phase.as_str(), summary))
        .collect::<BTreeMap<_, _>>();

    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for phase in baseline_by_phase
        .keys()
        .chain(candidate_by_phase.keys())
        .copied()
        .collect::<BTreeSet<_>>()
    {
        let baseline = baseline_by_phase.get(phase);
        let candidate = candidate_by_phase.get(phase);
        let median_delta_percent = baseline
            .zip(candidate)
            .and_then(|(base, cand)| percent_delta(base.median_ms, cand.median_ms));
        let p95_delta_percent = baseline
            .zip(candidate)
            .and_then(|(base, cand)| percent_delta(base.p95_ms, cand.p95_ms));

        if let Some(delta) = median_delta_percent
            && delta > fail_threshold_percent
        {
            failures.push(CompareFailure {
                phase: phase.to_string(),
                metric: "median".to_string(),
                delta_percent: delta,
            });
        }
        if let Some(delta) = p95_delta_percent
            && delta > fail_threshold_percent
        {
            failures.push(CompareFailure {
                phase: phase.to_string(),
                metric: "p95".to_string(),
                delta_percent: delta,
            });
        }

        rows.push(CompareRow {
            phase: phase.to_string(),
            median_delta_percent,
            p95_delta_percent,
        });
    }

    CompareResult { rows, failures }
}

fn percent_delta(baseline: f64, candidate: f64) -> Option<f64> {
    (baseline > 0.0).then(|| ((candidate - baseline) / baseline) * 100.0)
}

fn print_report_summary(report: &BenchmarkReport) {
    println!(
        "{} benchmark: {} measured run(s), {} warmup(s)",
        report.mode, report.scenario.run_count, report.scenario.warmup_count
    );
    println!(
        "{:<38} {:>7} {:>10} {:>10} {:>10} {:>10} {:>7}",
        "phase", "samples", "min", "median", "p95", "max", "errors"
    );
    for summary in &report.summary {
        println!(
            "{:<38} {:>7} {:>9.2} {:>9.2} {:>9.2} {:>9.2} {:>7}",
            summary.phase,
            summary.samples,
            summary.min_ms,
            summary.median_ms,
            summary.p95_ms,
            summary.max_ms,
            summary.errors
        );
    }
}

fn print_compare_result(result: &CompareResult) {
    println!("{:<38} {:>14} {:>14}", "phase", "median delta", "p95 delta");
    for row in &result.rows {
        println!(
            "{:<38} {:>13} {:>13}",
            row.phase,
            format_delta(row.median_delta_percent),
            format_delta(row.p95_delta_percent)
        );
    }
}

fn print_prompt_eval_summary(report: &PromptEvalReport) {
    println!(
        "prompt eval: {} candidate(s), {} run(s), suite {}",
        report.candidates.len(),
        report.run_count,
        report.suite_path
    );
    println!(
        "{:<14} {:<38} {:>8} {:>8} {:>10}",
        "provider", "model", "passed", "total", "pass rate"
    );
    for candidate in &report.candidates {
        println!(
            "{:<14} {:<38} {:>8} {:>8} {:>9.2}%",
            candidate.provider,
            candidate.model,
            candidate.summary.passed,
            candidate.summary.total,
            candidate.summary.pass_rate * 100.0
        );
    }
}

fn format_delta(delta: Option<f64>) -> String {
    delta
        .map(|delta| format!("{delta:.2}%"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_usage() {
    println!("{}", usage());
}

fn usage() -> &'static str {
    "Usage:
  glide-bench stt --audio <wav> --provider <openai|groq|fireworks|elevenlabs|apple_local|parakeet> --model <id> [--runs N] [--warmups N] [--output path]
  glide-bench llm --text <text|@file|file> --provider <openai|groq|cerebras|fireworks|apple_local> --model <id> [--runs N] [--warmups N] [--output path]
  glide-bench flow --audio <wav> [--target-app name] [--style name|default] [--runs N] [--warmups N] [--paste|--no-paste] [--output path]
  glide-bench prompt-eval --suite <jsonl> --candidate <provider:model> [--candidate <provider:model> ...] [--runs N] [--timeout-secs N] [--no-edit-prepass] [--output path]
  glide-bench compare --baseline <json> --candidate <json> [--fail-threshold percent]"
}

fn provider_metadata(
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

fn provider_base_url_host(
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

fn summarize_text(text: &str) -> TextSummary {
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

fn environment_metadata() -> EnvironmentMetadata {
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

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_prompt_eval_candidate() {
        let candidate = parse_prompt_eval_candidate("openai:gpt-5.4-nano").unwrap();
        assert_eq!(candidate.provider, Provider::OpenAi);
        assert_eq!(candidate.model, "gpt-5.4-nano");
    }

    #[test]
    fn rejects_non_llm_prompt_eval_candidate() {
        let error = parse_prompt_eval_candidate("parakeet:parakeet-tdt-0.6b-v3-int8")
            .unwrap_err()
            .to_string();
        assert!(error.contains("Parakeet does not provide an LLM cleanup model"));
    }

    #[test]
    fn reads_prompt_eval_jsonl_suite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("suite.jsonl");
        fs::write(
            &path,
            r#"{"id":"question","style":"default","input":"Can you help","expected":"Can you help?","accepted_outputs":["Can you help."],"forbidden_substrings":["Sure"],"tags":["question"]}

{"id":"scratch","input":"Hello scratch that goodbye","expected":"Goodbye.","tags":["correction"]}"#,
        )
        .unwrap();

        let cases = read_prompt_eval_suite(&path).unwrap();

        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].id, "question");
        assert_eq!(cases[0].accepted_outputs, vec!["Can you help."]);
        assert_eq!(cases[0].forbidden_substrings, vec!["Sure"]);
        assert_eq!(cases[1].style, "default");
    }

    #[test]
    fn scores_prompt_eval_with_normalized_exact_match() {
        let case = PromptEvalCase {
            id: "case".to_string(),
            style: "default".to_string(),
            input: "Can you help".to_string(),
            expected: "Can you help?".to_string(),
            accepted_outputs: Vec::new(),
            forbidden_substrings: Vec::new(),
            tags: vec!["question".to_string()],
        };
        let normalized = normalize_prompt_eval_text("  Can you\nhelp?  ");

        let (passed, reason) = score_prompt_eval_output(&case, &normalized, "  Can you\nhelp?  ");

        assert!(passed);
        assert_eq!(reason, "ok");
    }

    #[test]
    fn scores_prompt_eval_with_accepted_output_variant() {
        let case = PromptEvalCase {
            id: "case".to_string(),
            style: "default".to_string(),
            input: "Can you help".to_string(),
            expected: "Can you help?".to_string(),
            accepted_outputs: vec!["Can you help.".to_string()],
            forbidden_substrings: Vec::new(),
            tags: vec!["question".to_string()],
        };
        let normalized = normalize_prompt_eval_text("Can you help.");

        let (passed, reason) = score_prompt_eval_output(&case, &normalized, "Can you help.");

        assert!(passed);
        assert_eq!(reason, "ok");
    }

    #[test]
    fn scores_prompt_eval_forbidden_substring_failure() {
        let case = PromptEvalCase {
            id: "case".to_string(),
            style: "default".to_string(),
            input: "Can you help".to_string(),
            expected: "Can you help?".to_string(),
            accepted_outputs: vec!["Sure, can you help?".to_string()],
            forbidden_substrings: vec!["Sure".to_string()],
            tags: vec!["question".to_string()],
        };
        let normalized = normalize_prompt_eval_text("Sure, I can help.");

        let (passed, reason) = score_prompt_eval_output(&case, &normalized, "Sure, I can help.");

        assert!(!passed);
        assert_eq!(reason, "forbidden substring present: Sure");
    }

    #[test]
    fn serializes_prompt_eval_provider_error_details() {
        let case = PromptEvalCase {
            id: "case".to_string(),
            style: "default".to_string(),
            input: "Can you help".to_string(),
            expected: "Can you help?".to_string(),
            accepted_outputs: Vec::new(),
            forbidden_substrings: Vec::new(),
            tags: vec!["question".to_string()],
        };
        let candidate = PromptEvalCandidate {
            provider: Provider::OpenAi,
            model: "gpt-5.4-nano".to_string(),
        };

        let result = prompt_eval_result(
            &case,
            0,
            &candidate,
            Err(anyhow::anyhow!(
                "OpenAI chat completions API returned HTTP 400 Bad Request: bad model"
            )),
            12.0,
        );
        let serialized = serde_json::to_string(&result).unwrap();

        assert!(!result.passed);
        assert!(result.reason.contains("HTTP 400 Bad Request"));
        assert!(serialized.contains("bad model"));
    }

    #[test]
    fn summarizes_prompt_eval_results_by_tag() {
        let results = vec![
            PromptEvalResult {
                case_id: "one".to_string(),
                run_index: 0,
                provider: "OpenAI".to_string(),
                model: "model".to_string(),
                style: "default".to_string(),
                tags: vec!["question".to_string(), "no_answer".to_string()],
                input: String::new(),
                expected: String::new(),
                accepted_outputs: Vec::new(),
                raw_output: String::new(),
                normalized_output: String::new(),
                passed: true,
                reason: "ok".to_string(),
                latency_ms: 1.0,
                error: None,
            },
            PromptEvalResult {
                case_id: "two".to_string(),
                run_index: 0,
                provider: "OpenAI".to_string(),
                model: "model".to_string(),
                style: "default".to_string(),
                tags: vec!["question".to_string()],
                input: String::new(),
                expected: String::new(),
                accepted_outputs: Vec::new(),
                raw_output: String::new(),
                normalized_output: String::new(),
                passed: false,
                reason: "mismatch".to_string(),
                latency_ms: 1.0,
                error: None,
            },
        ];

        let summary = summarize_prompt_eval_results(&results);

        assert_eq!(summary.total, 2);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.pass_rate, 0.5);
        assert_eq!(
            summary
                .tags
                .iter()
                .find(|tag| tag.tag == "question")
                .unwrap(),
            &PromptEvalTagSummary {
                tag: "question".to_string(),
                total: 2,
                passed: 1,
                pass_rate: 0.5,
            }
        );
    }

    #[test]
    fn disabled_profile_collector_records_nothing() {
        let collector = ProfileCollector::disabled();
        collector.record("phase", Duration::from_millis(10));
        collector.mark("start");
        collector.record_since_marker("start", "since_start");
        assert!(collector.spans().is_empty());
    }

    #[test]
    fn profile_collector_records_marker_intervals() {
        let collector = ProfileCollector::enabled();
        collector.mark("release");
        std::thread::sleep(Duration::from_millis(1));
        collector.record_since_marker("release", "release_to_send");

        let spans = collector.spans();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].phase, "release_to_send");
        assert!(spans[0].duration_ms > 0.0);
    }

    #[test]
    fn loads_recorded_benchmark_audio_fixtures() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let short = load_wav_audio(&root.join("fixtures/benchmark/dictation-short.wav")).unwrap();
        let long = load_wav_audio(&root.join("fixtures/benchmark/dictation-long.wav")).unwrap();

        assert!(short.metadata.duration_seconds > 6.0);
        assert!(short.metadata.duration_seconds < 8.0);
        assert!(long.metadata.duration_seconds > 17.0);
        assert!(long.metadata.duration_seconds < 19.0);
        assert!(matches!(short.recorded.format, AudioFormat::Wav));
        assert!(matches!(long.recorded.format, AudioFormat::Wav));
    }

    #[test]
    fn phase_summary_uses_nearest_rank_percentiles() {
        let summary = phase_summary("phase".to_string(), &[10.0, 50.0, 20.0, 40.0, 30.0], 2);
        assert_eq!(summary.samples, 5);
        assert_eq!(summary.errors, 2);
        assert_eq!(summary.min_ms, 10.0);
        assert_eq!(summary.median_ms, 30.0);
        assert_eq!(summary.p95_ms, 50.0);
        assert_eq!(summary.max_ms, 50.0);
    }

    #[test]
    fn compare_reports_flags_regressions() {
        let baseline = BenchmarkReport {
            schema_version: 1,
            mode: "stt".to_string(),
            generated_at_unix_ms: 1,
            environment: environment_metadata(),
            scenario: ScenarioMetadata {
                provider: None,
                model: None,
                run_count: 1,
                warmup_count: 0,
                audio: None,
                text: None,
                target_app: None,
                style: None,
                paste_enabled: false,
                base_url_host: None,
            },
            runs: Vec::new(),
            summary: vec![PhaseSummary {
                phase: "phase".to_string(),
                samples: 1,
                errors: 0,
                min_ms: 100.0,
                median_ms: 100.0,
                p95_ms: 100.0,
                max_ms: 100.0,
            }],
        };
        let mut candidate = baseline.clone();
        candidate.summary[0].median_ms = 130.0;
        candidate.summary[0].p95_ms = 105.0;

        let result = compare_reports(&baseline, &candidate, 20.0);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].metric, "median");
    }
}

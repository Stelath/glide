use std::{collections::VecDeque, path::PathBuf};

use anyhow::{Context, Result};

use super::{
    Provider,
    compare::compare_report_files,
    prompt_eval::run_prompt_eval,
    runner::{run_flow_benchmark, run_llm_benchmark, run_stt_benchmark},
    types::{
        BenchCommand, BenchmarkReport, CompareOptions, CompareResult, FlowBenchOptions,
        LlmBenchOptions, PromptEvalCandidate, PromptEvalOptions, PromptEvalReport, SttBenchOptions,
    },
};

const DEFAULT_RUNS: usize = 3;
const DEFAULT_WARMUPS: usize = 1;
const DEFAULT_PROMPT_EVAL_RUNS: usize = 1;
const DEFAULT_PROMPT_EVAL_TIMEOUT_SECS: u64 = 60;

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

pub(super) fn parse_prompt_eval_candidate(raw: &str) -> Result<PromptEvalCandidate> {
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
  glide-bench prompt-eval --suite <jsonl> --candidate <provider:model> [--candidate <provider:model> ...] [--runs N] [--timeout-secs N] [--output path]
  glide-bench compare --baseline <json> --candidate <json> [--fail-threshold percent]"
}

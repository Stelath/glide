use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};

use crate::{
    config::GlideConfig,
    llm::{self, CleanupContext},
};

use super::{
    REPORT_SCHEMA_VERSION,
    report::{environment_metadata, provider_base_url_host, unix_millis, write_prompt_eval_report},
    types::{
        PromptEvalCandidate, PromptEvalCandidateReport, PromptEvalCase, PromptEvalOptions,
        PromptEvalReport, PromptEvalResult, PromptEvalSummary, PromptEvalTagSummary,
    },
};

pub(super) fn run_prompt_eval(options: &PromptEvalOptions) -> Result<(PromptEvalReport, PathBuf)> {
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
        "prompt-eval: loaded {} case(s), {} candidate(s), {} run(s), {}s timeout per case",
        cases.len(),
        options.candidates.len(),
        options.runs,
        options.timeout_secs
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
        candidates: candidate_reports,
    };
    let path = write_prompt_eval_report(&report, options.output.as_deref())?;
    Ok((report, path))
}
pub(super) fn read_prompt_eval_suite(path: &Path) -> Result<Vec<PromptEvalCase>> {
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

pub(super) fn prompt_eval_result(
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

pub(super) fn score_prompt_eval_output(
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

pub(super) fn normalize_prompt_eval_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn accepted_prompt_eval_outputs(case: &PromptEvalCase) -> Vec<String> {
    std::iter::once(case.expected.as_str())
        .chain(case.accepted_outputs.iter().map(String::as_str))
        .map(normalize_prompt_eval_text)
        .collect()
}

pub(super) fn summarize_prompt_eval_results(results: &[PromptEvalResult]) -> PromptEvalSummary {
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

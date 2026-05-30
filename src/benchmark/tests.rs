use super::*;
use std::{fs, path::PathBuf, time::Duration};

fn prompt_case(
    expected: &str,
    accepted_outputs: &[&str],
    forbidden_substrings: &[&str],
) -> PromptEvalCase {
    PromptEvalCase {
        id: "case".to_string(),
        style: "default".to_string(),
        input: "Can you help".to_string(),
        expected: expected.to_string(),
        accepted_outputs: accepted_outputs
            .iter()
            .map(|output| output.to_string())
            .collect(),
        forbidden_substrings: forbidden_substrings
            .iter()
            .map(|substring| substring.to_string())
            .collect(),
        tags: vec!["question".to_string()],
    }
}

fn report_with_summary(summary: Vec<PhaseSummary>) -> BenchmarkReport {
    BenchmarkReport {
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
        summary,
    }
}

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
fn scores_prompt_eval_outputs() {
    let cases = [
        (
            prompt_case("Can you help?", &[], &[]),
            "  Can you\nhelp?  ",
            true,
            "ok",
        ),
        (
            prompt_case("Can you help?", &["Can you help."], &[]),
            "Can you help.",
            true,
            "ok",
        ),
        (
            prompt_case("Can you help?", &["Sure, can you help?"], &["Sure"]),
            "Sure, I can help.",
            false,
            "forbidden substring present: Sure",
        ),
    ];

    for (case, raw_output, expected_passed, expected_reason) in cases {
        let normalized = normalize_prompt_eval_text(raw_output);
        let (passed, reason) = score_prompt_eval_output(&case, &normalized, raw_output);
        assert_eq!(passed, expected_passed, "{raw_output}");
        assert_eq!(reason, expected_reason, "{raw_output}");
    }
}

#[test]
fn serializes_prompt_eval_provider_error_details() {
    let case = prompt_case("Can you help?", &[], &[]);
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
fn profile_collector_records_only_when_enabled() {
    let disabled = ProfileCollector::disabled();
    disabled.record("phase", Duration::from_millis(10));
    disabled.mark("start");
    disabled.record_since_marker("start", "since_start");
    assert!(disabled.spans().is_empty());

    let enabled = ProfileCollector::enabled();
    enabled.mark("release");
    std::thread::sleep(Duration::from_millis(1));
    enabled.record_since_marker("release", "release_to_send");

    let spans = enabled.spans();
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
    let baseline = report_with_summary(vec![PhaseSummary {
        phase: "phase".to_string(),
        samples: 1,
        errors: 0,
        min_ms: 100.0,
        median_ms: 100.0,
        p95_ms: 100.0,
        max_ms: 100.0,
    }]);
    let mut candidate = baseline.clone();
    candidate.summary[0].median_ms = 130.0;
    candidate.summary[0].p95_ms = 105.0;

    let result = compare_reports(&baseline, &candidate, 20.0);
    assert_eq!(result.failures.len(), 1);
    assert_eq!(result.failures[0].metric, "median");
}

#[test]
fn base_url_redaction_keeps_only_host() {
    assert_eq!(
        redacted_base_url_host("https://secret@example.test:8443/v1/models").as_deref(),
        Some("example.test:8443")
    );
    assert_eq!(redacted_base_url_host("  ").as_deref(), None);
}

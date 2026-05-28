use glide::benchmark::{
    BenchCommand, BenchmarkReport, CompareOptions, EnvironmentMetadata, PhaseSummary, Provider,
    ScenarioMetadata, compare_reports, parse_cli_args, phase_summary, redacted_base_url_host,
};

fn empty_report(summary: Vec<PhaseSummary>) -> BenchmarkReport {
    BenchmarkReport {
        schema_version: 1,
        mode: "stt".to_string(),
        generated_at_unix_ms: 1,
        environment: EnvironmentMetadata {
            glide_version: "test".to_string(),
            git_sha: None,
            os: "test".to_string(),
        },
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
fn parses_stt_cli_command() {
    let command = parse_cli_args([
        "glide-bench",
        "stt",
        "--audio",
        "sample.wav",
        "--provider",
        "groq",
        "--model",
        "whisper-large-v3-turbo",
        "--runs",
        "5",
        "--warmups",
        "2",
    ])
    .unwrap();

    match command {
        BenchCommand::Stt(options) => {
            assert_eq!(options.audio.to_string_lossy(), "sample.wav");
            assert_eq!(options.provider, Provider::Groq);
            assert_eq!(options.model, "whisper-large-v3-turbo");
            assert_eq!(options.runs, 5);
            assert_eq!(options.warmups, 2);
        }
        other => panic!("expected stt command, got {other:?}"),
    }
}

#[test]
fn parses_new_remote_providers() {
    let stt = parse_cli_args([
        "glide-bench",
        "stt",
        "--audio",
        "sample.wav",
        "--provider",
        "elevenlabs",
        "--model",
        "scribe_v2",
    ])
    .unwrap();
    match stt {
        BenchCommand::Stt(options) => assert_eq!(options.provider, Provider::ElevenLabs),
        other => panic!("expected stt command, got {other:?}"),
    }

    let llm = parse_cli_args([
        "glide-bench",
        "llm",
        "--text",
        "hello",
        "--provider",
        "fireworks",
        "--model",
        "accounts/fireworks/models/gpt-oss-20b",
    ])
    .unwrap();
    match llm {
        BenchCommand::Llm(options) => assert_eq!(options.provider, Provider::Fireworks),
        other => panic!("expected llm command, got {other:?}"),
    }
}

#[test]
fn parses_flow_as_no_paste_by_default() {
    let command = parse_cli_args(["glide-bench", "flow", "--audio", "sample.wav"]).unwrap();

    match command {
        BenchCommand::Flow(options) => assert!(!options.paste),
        other => panic!("expected flow command, got {other:?}"),
    }
}

#[test]
fn parses_prompt_eval_cli_command() {
    let command = parse_cli_args([
        "glide-bench",
        "prompt-eval",
        "--suite",
        "fixtures/prompt_eval/core.jsonl",
        "--candidate",
        "openai:gpt-5.4-nano",
        "--candidate",
        "groq:meta-llama/llama-4-scout-17b-16e-instruct",
        "--runs",
        "2",
        "--timeout-secs",
        "15",
        "--no-edit-prepass",
        "--output",
        "report.json",
    ])
    .unwrap();

    match command {
        BenchCommand::PromptEval(options) => {
            assert_eq!(
                options.suite.to_string_lossy(),
                "fixtures/prompt_eval/core.jsonl"
            );
            assert_eq!(
                options
                    .candidates
                    .iter()
                    .map(|candidate| candidate.model.as_str())
                    .collect::<Vec<_>>(),
                vec!["gpt-5.4-nano", "meta-llama/llama-4-scout-17b-16e-instruct",]
            );
            assert_eq!(options.runs, 2);
            assert_eq!(options.timeout_secs, 15);
            assert!(!options.edit_prepass);
            assert_eq!(options.output.unwrap().to_string_lossy(), "report.json");
        }
        other => panic!("expected prompt-eval command, got {other:?}"),
    }
}

#[test]
fn parses_compare_cli_command() {
    let command = parse_cli_args([
        "glide-bench",
        "compare",
        "--baseline",
        "base.json",
        "--candidate",
        "candidate.json",
        "--fail-threshold",
        "12.5",
    ])
    .unwrap();

    assert_eq!(
        command,
        BenchCommand::Compare(CompareOptions {
            baseline: "base.json".into(),
            candidate: "candidate.json".into(),
            fail_threshold_percent: 12.5,
        })
    );
}

#[test]
fn summarizes_percentiles_and_errors() {
    let summary = phase_summary("phase".to_string(), &[30.0, 10.0, 20.0, 40.0], 1);

    assert_eq!(summary.samples, 4);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.min_ms, 10.0);
    assert_eq!(summary.median_ms, 20.0);
    assert_eq!(summary.p95_ms, 40.0);
    assert_eq!(summary.max_ms, 40.0);
}

#[test]
fn compare_reports_fails_when_threshold_is_exceeded() {
    let baseline = empty_report(vec![PhaseSummary {
        phase: "remote_stt_provider_total".to_string(),
        samples: 3,
        errors: 0,
        min_ms: 90.0,
        median_ms: 100.0,
        p95_ms: 120.0,
        max_ms: 130.0,
    }]);
    let candidate = empty_report(vec![PhaseSummary {
        phase: "remote_stt_provider_total".to_string(),
        samples: 3,
        errors: 0,
        min_ms: 120.0,
        median_ms: 130.0,
        p95_ms: 150.0,
        max_ms: 160.0,
    }]);

    let result = compare_reports(&baseline, &candidate, 20.0);

    assert!(result.failures.iter().any(|failure| {
        failure.phase == "remote_stt_provider_total" && failure.metric == "median"
    }));
    assert!(result.failures.iter().any(|failure| {
        failure.phase == "remote_stt_provider_total" && failure.metric == "p95"
    }));
}

#[test]
fn base_url_redaction_keeps_only_host() {
    assert_eq!(
        redacted_base_url_host("https://secret@example.test:8443/v1/models").as_deref(),
        Some("example.test:8443")
    );
    assert_eq!(redacted_base_url_host("  ").as_deref(), None);
}

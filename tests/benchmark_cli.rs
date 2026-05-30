use glide::benchmark::{BenchCommand, CompareOptions, Provider, parse_cli_args};

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

use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};
use glide_tools::ProfileCollector;

use glide::benchmark_support::{
    self as glide_core, AudioFormat, GlideConfig, ModelSelection, RecordedAudio, ReplacementRule,
};

use super::{
    SpanRecord,
    report::{
        build_report, provider_base_url_host, provider_metadata, summarize_text, write_report,
    },
    types::{
        AudioMetadata, BenchmarkReport, BenchmarkRun, FlowBenchOptions, LlmBenchOptions,
        ProviderModelMetadata, ScenarioMetadata, SttBenchOptions, TextSummary,
    },
};

pub(super) fn run_stt_benchmark(options: &SttBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
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

pub(super) fn run_llm_benchmark(options: &LlmBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
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

pub(super) fn run_flow_benchmark(options: &FlowBenchOptions) -> Result<(BenchmarkReport, PathBuf)> {
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
            glide_core::build_profiled_stt_provider(
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
            glide_core::build_profiled_llm_provider(
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
        let cleaned = runtime.block_on(provider.clean(text));
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
            glide_core::build_profiled_stt_provider(
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
                glide_core::build_profiled_llm_provider(
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
            let cleaned = runtime.block_on(llm_provider.clean(&raw_text));
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
            glide_core::strip_think_tags(&cleaned_text)
        });

        if options.paste
            && let Err(error) = collector.measure_result("flow_paste", || {
                glide_core::paste_text(&cleaned_text, &config.paste)
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

pub(super) struct BenchmarkAudio {
    pub(super) recorded: RecordedAudio,
    pub(super) metadata: AudioMetadata,
}

pub(super) fn load_wav_audio(path: &Path) -> Result<BenchmarkAudio> {
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

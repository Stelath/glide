use std::time::Instant;

use anyhow::{Context, Result};

use crate::{
    audio::RecordedAudio,
    benchmark::ProfileCollector,
    config::ReplacementRule,
    llm::{self, CleanupContext},
    paste,
    state::{RuntimeStatus, SharedState},
    stt,
    trace::{TraceSession, attrs},
};

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

fn elapsed_ms(started: Instant) -> u128 {
    started.elapsed().as_millis()
}

pub async fn process_recording(
    shared: SharedState,
    audio: RecordedAudio,
    target_app: Option<String>,
    trace: TraceSession,
    release_started: Option<Instant>,
) -> Result<()> {
    let pipeline_started = Instant::now();
    let result =
        process_recording_inner(shared, audio, target_app, trace.clone(), release_started).await;
    trace.record("pipeline_total", pipeline_started.elapsed());
    result
}

async fn process_recording_inner(
    shared: SharedState,
    audio: RecordedAudio,
    target_app: Option<String>,
    trace: TraceSession,
    release_started: Option<Instant>,
) -> Result<()> {
    let pipeline_started = Instant::now();
    trace.instant_with_attrs(
        "pipeline_start",
        attrs([
            ("sample_count", audio.sample_count.to_string()),
            ("byte_count", audio.bytes.len().to_string()),
            (
                "target_app",
                target_app.clone().unwrap_or_else(|| "unknown".to_string()),
            ),
        ]),
    );

    if let Some(release_started) = release_started {
        trace.record_since("release_to_pipeline_start", release_started);
    }

    let config = trace.measure("pipeline_config_snapshot", || shared.config());
    trace.measure("pipeline_status_processing", || {
        shared.set_status(
            RuntimeStatus::Processing,
            format!("Transcribing {} samples", audio.sample_count),
        )
    });

    // Match style by target app
    let matched_style = trace.measure("pipeline_style_resolution", || {
        target_app.as_ref().and_then(|target| {
            config
                .dictation
                .styles
                .iter()
                .find(|s| s.apps.iter().any(|a| a.eq_ignore_ascii_case(target)))
        })
    });

    // Resolve effective STT settings
    let stt_sel = matched_style
        .and_then(|s| s.stt.as_ref())
        .unwrap_or(&config.dictation.stt);

    trace.instant_with_attrs(
        "pipeline_stt_selected",
        attrs([
            ("provider", format!("{:?}", stt_sel.provider)),
            ("model", stt_sel.model.clone()),
        ]),
    );

    eprintln!(
        "[glide] STT: transcribing {} samples via {:?} / {}...",
        audio.sample_count, stt_sel.provider, stt_sel.model
    );
    let vocab_prompt = trace.measure("pipeline_vocab_prompt", || {
        if config.dictionary.vocabulary.is_empty() {
            None
        } else {
            Some(config.dictionary.vocabulary.join(", "))
        }
    });

    let stt_profile = profile_for_trace(&trace);
    let stt_build_started = Instant::now();
    let stt_provider = if trace.is_enabled() {
        stt::build_profiled_provider(
            stt_sel.provider,
            &stt_sel.model,
            &config.providers,
            vocab_prompt,
            stt_profile.clone(),
        )
    } else {
        stt::build_provider(
            stt_sel.provider,
            &stt_sel.model,
            &config.providers,
            vocab_prompt,
        )
    }
    .context("failed to build STT provider");
    trace.record("pipeline_stt_provider_build", stt_build_started.elapsed());
    let stt_provider = stt_provider?;
    let stt_name = stt_provider.name();
    eprintln!(
        "[glide] STT: provider ready in {} ms",
        elapsed_ms(stt_build_started)
    );

    if let Some(release_started) = release_started {
        trace.record_since("release_to_stt_call_start", release_started);
    }
    let stt_started = Instant::now();
    let raw_text = stt_provider
        .transcribe(&audio.bytes, audio.format)
        .await
        .with_context(|| format!("{stt_name} transcription failed"));
    trace.record("pipeline_stt_call", stt_started.elapsed());
    trace.record_profile_spans("provider_stt", &stt_profile.spans());
    let raw_text = raw_text?;
    let stt_result_at = Instant::now();
    trace.instant_with_attrs(
        "pipeline_stt_result",
        attrs([("char_count", raw_text.chars().count().to_string())]),
    );

    anyhow::ensure!(
        !raw_text.trim().is_empty(),
        "transcription returned no text"
    );

    let raw_text = trace.measure("pipeline_replacements", || {
        apply_replacements(&raw_text, &config.dictionary.replacements)
    });
    eprintln!(
        "[glide] STT: got transcript ({} chars) in {} ms",
        raw_text.len(),
        elapsed_ms(stt_started)
    );

    // Resolve effective LLM settings
    let llm_sel = matched_style
        .and_then(|s| s.llm.as_ref())
        .or(config.dictation.llm.as_ref());
    let system_prompt = matched_style
        .map(|s| s.prompt.as_str())
        .unwrap_or(&config.dictation.system_prompt);

    let cleaned_text = if let Some(llm) = llm_sel {
        trace.instant_with_attrs(
            "pipeline_llm_selected",
            attrs([
                ("provider", format!("{:?}", llm.provider)),
                ("model", llm.model.clone()),
            ]),
        );
        eprintln!(
            "[glide] LLM: cleaning up via {:?} / {}...",
            llm.provider, llm.model
        );
        let llm_profile = profile_for_trace(&trace);
        let llm_build_started = Instant::now();
        let llm_provider = if trace.is_enabled() {
            llm::build_profiled_provider(
                llm.provider,
                &llm.model,
                system_prompt,
                &config.providers,
                llm_profile.clone(),
            )
        } else {
            llm::build_provider(llm.provider, &llm.model, system_prompt, &config.providers)
        }
        .context("failed to build LLM provider");
        trace.record("pipeline_llm_provider_build", llm_build_started.elapsed());
        let llm_provider = llm_provider?;
        let llm_name = llm_provider.name();
        eprintln!(
            "[glide] LLM: provider ready in {} ms",
            elapsed_ms(llm_build_started)
        );
        trace.record_since("stt_result_to_llm_call_start", stt_result_at);
        if let Some(release_started) = release_started {
            trace.record_since("release_to_llm_call_start", release_started);
        }
        let llm_started = Instant::now();
        let cleaned = llm_provider
            .clean(
                &raw_text,
                &CleanupContext {
                    target_app,
                    mode_hint: Some("general dictation".to_string()),
                },
            )
            .await
            .with_context(|| format!("{llm_name} cleanup failed"));
        trace.record("pipeline_llm_call", llm_started.elapsed());
        trace.record_profile_spans("provider_llm", &llm_profile.spans());
        cleaned.inspect(|text| {
            trace.instant_with_attrs(
                "pipeline_llm_result",
                attrs([("char_count", text.chars().count().to_string())]),
            );
            eprintln!(
                "[glide] LLM: cleanup returned {} chars in {} ms",
                text.len(),
                elapsed_ms(llm_started)
            );
        })?
    } else {
        trace.instant("pipeline_llm_disabled");
        trace.record_since("stt_result_to_paste_candidate", stt_result_at);
        eprintln!("[glide] LLM: disabled, using raw transcript");
        raw_text.clone()
    };

    // Strip <think>...</think> tags some models emit (e.g. DeepSeek reasoning)
    let cleaned_text = trace.measure("pipeline_strip_think_tags", || {
        llm::strip_think_tags(&cleaned_text)
    });

    eprintln!("[glide] Pasting {} chars", cleaned_text.len());
    if let Some(release_started) = release_started {
        trace.record_since("release_to_paste_start", release_started);
    }
    let paste_started = Instant::now();
    paste::paste_text(&cleaned_text, &config.paste).context("failed to paste transcript")?;
    trace.record("pipeline_paste", paste_started.elapsed());
    eprintln!(
        "[glide] Paste: request returned in {} ms",
        elapsed_ms(paste_started)
    );
    trace.measure("pipeline_set_last_transcript", || {
        shared.set_last_transcript(cleaned_text.clone())
    });
    trace.measure("pipeline_status_idle", || {
        shared.set_status(RuntimeStatus::Idle, "Ready for dictation")
    });
    eprintln!(
        "[glide] Pipeline: completed in {} ms",
        elapsed_ms(pipeline_started)
    );
    Ok(())
}

fn profile_for_trace(trace: &TraceSession) -> ProfileCollector {
    if trace.is_enabled() {
        ProfileCollector::enabled()
    } else {
        ProfileCollector::disabled()
    }
}

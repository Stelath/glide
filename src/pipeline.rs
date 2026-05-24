use std::time::Instant;

use anyhow::{Context, Result};

use crate::{
    audio::RecordedAudio,
    config::ReplacementRule,
    llm::{self, CleanupContext},
    paste,
    state::{RuntimeStatus, SharedState},
    stt,
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
) -> Result<()> {
    let pipeline_started = Instant::now();
    let config = shared.config();
    shared.set_status(
        RuntimeStatus::Processing,
        format!("Transcribing {} samples", audio.sample_count),
    );

    // Match style by target app
    let matched_style = target_app.as_ref().and_then(|target| {
        config
            .dictation
            .styles
            .iter()
            .find(|s| s.apps.iter().any(|a| a.eq_ignore_ascii_case(target)))
    });

    // Resolve effective STT settings
    let stt_sel = matched_style
        .and_then(|s| s.stt.as_ref())
        .unwrap_or(&config.dictation.stt);

    eprintln!(
        "[glide] STT: transcribing {} samples via {:?} / {}...",
        audio.sample_count, stt_sel.provider, stt_sel.model
    );
    let vocab_prompt = if config.dictionary.vocabulary.is_empty() {
        None
    } else {
        Some(config.dictionary.vocabulary.join(", "))
    };

    let stt_build_started = Instant::now();
    let stt_provider = stt::build_provider(
        stt_sel.provider,
        &stt_sel.model,
        &config.providers,
        vocab_prompt,
    )
    .context("failed to build STT provider")?;
    let stt_name = stt_provider.name();
    eprintln!(
        "[glide] STT: provider ready in {} ms",
        elapsed_ms(stt_build_started)
    );

    let stt_started = Instant::now();
    let raw_text = stt_provider
        .transcribe(&audio.bytes, audio.format)
        .await
        .with_context(|| format!("{stt_name} transcription failed"))?;

    anyhow::ensure!(
        !raw_text.trim().is_empty(),
        "transcription returned no text"
    );

    let raw_text = apply_replacements(&raw_text, &config.dictionary.replacements);
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
        eprintln!(
            "[glide] LLM: cleaning up via {:?} / {}...",
            llm.provider, llm.model
        );
        let llm_build_started = Instant::now();
        let llm_provider =
            llm::build_provider(llm.provider, &llm.model, system_prompt, &config.providers)
                .context("failed to build LLM provider")?;
        let llm_name = llm_provider.name();
        eprintln!(
            "[glide] LLM: provider ready in {} ms",
            elapsed_ms(llm_build_started)
        );
        let llm_started = Instant::now();
        llm_provider
            .clean(
                &raw_text,
                &CleanupContext {
                    target_app,
                    mode_hint: Some("general dictation".to_string()),
                },
            )
            .await
            .with_context(|| format!("{llm_name} cleanup failed"))
            .map(|text| {
                eprintln!(
                    "[glide] LLM: cleanup returned {} chars in {} ms",
                    text.len(),
                    elapsed_ms(llm_started)
                );
                text
            })?
    } else {
        eprintln!("[glide] LLM: disabled, using raw transcript");
        raw_text.clone()
    };

    // Strip <think>...</think> tags some models emit (e.g. DeepSeek reasoning)
    let cleaned_text = llm::strip_think_tags(&cleaned_text);

    eprintln!("[glide] Pasting {} chars", cleaned_text.len());
    let paste_started = Instant::now();
    paste::paste_text(&cleaned_text, &config.paste).context("failed to paste transcript")?;
    eprintln!(
        "[glide] Paste: request returned in {} ms",
        elapsed_ms(paste_started)
    );
    shared.set_last_transcript(cleaned_text.clone());
    shared.set_status(RuntimeStatus::Idle, "Ready for dictation");
    eprintln!(
        "[glide] Pipeline: completed in {} ms",
        elapsed_ms(pipeline_started)
    );
    Ok(())
}

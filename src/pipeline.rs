use anyhow::{Context, Result};

/// Remove `<think>...</think>` blocks from LLM output.
/// NOTE: Duplicated in crates/glide-core/src/llm.rs (separate C FFI crate, no shared dependency).
fn strip_think_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.to_lowercase().find("<think") {
        result.push_str(&remaining[..start]);
        // Find the closing tag
        if let Some(end) = remaining[start..].to_lowercase().find("</think") {
            let close_end = remaining[start + end..]
                .find('>')
                .map(|i| start + end + i + 1)
                .unwrap_or(remaining.len());
            remaining = &remaining[close_end..];
        } else {
            // No closing tag — strip everything from <think onward
            remaining = "";
        }
    }
    result.push_str(remaining);
    result.trim().to_string()
}

use crate::{
    app::{RuntimeStatus, SharedState},
    audio::RecordedAudio,
    llm::{self, CleanupContext},
    paste,
    stt,
};

pub async fn process_recording(
    shared: SharedState,
    audio: RecordedAudio,
    target_app: Option<String>,
) -> Result<()> {
    let config = shared.config();
    shared.set_status(
        RuntimeStatus::Processing,
        format!("Transcribing {} samples", audio.sample_count),
    );

    // Match style by target app
    let matched_style = target_app.as_ref().and_then(|target| {
        config.dictation.styles.iter().find(|s| {
            s.apps.iter().any(|a| a.eq_ignore_ascii_case(target))
        })
    });

    // Resolve effective STT settings
    let stt_sel = matched_style
        .and_then(|s| s.stt.as_ref())
        .unwrap_or(&config.dictation.stt);

    eprintln!("[glide] STT: transcribing {} samples via {:?} / {}...", audio.sample_count, stt_sel.provider, stt_sel.model);
    let stt_provider = stt::build_provider(stt_sel.provider, &stt_sel.model, &config.providers)
        .context("failed to build STT provider")?;
    let raw_text = stt_provider
        .transcribe(&audio.bytes, audio.format)
        .await
        .with_context(|| format!("{} transcription failed", stt_provider.name()))?;

    anyhow::ensure!(!raw_text.trim().is_empty(), "transcription returned no text");
    eprintln!("[glide] STT: got transcript ({} chars)", raw_text.len());

    // Resolve effective LLM settings
    let llm_sel = matched_style
        .and_then(|s| s.llm.as_ref())
        .or(config.dictation.llm.as_ref());
    let system_prompt = matched_style
        .map(|s| s.prompt.as_str())
        .unwrap_or(&config.dictation.system_prompt);

    let cleaned_text = if let Some(llm) = llm_sel {
        eprintln!("[glide] LLM: cleaning up via {:?} / {}...", llm.provider, llm.model);
        let llm_provider = llm::build_provider(llm.provider, &llm.model, system_prompt, &config.providers)
            .with_context(|| format!("failed to build LLM provider"))?;
        llm_provider
            .clean(
                &raw_text,
                &CleanupContext {
                    target_app,
                    mode_hint: Some("general dictation".to_string()),
                },
            )
            .await
            .with_context(|| format!("{} cleanup failed", llm_provider.name()))?
    } else {
        eprintln!("[glide] LLM: disabled, using raw transcript");
        raw_text.clone()
    };

    // Strip <think>...</think> tags some models emit (e.g. DeepSeek reasoning)
    let cleaned_text = strip_think_tags(&cleaned_text);

    eprintln!("[glide] Pasting {} chars", cleaned_text.len());
    paste::paste_text(&cleaned_text, &config.paste).context("failed to paste transcript")?;
    shared.set_last_transcript(cleaned_text.clone());
    shared.set_status(RuntimeStatus::Idle, "Ready for dictation");
    Ok(())
}

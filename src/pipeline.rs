use anyhow::{Context, Result};

use crate::{
    app::{RuntimeStatus, SharedState},
    audio::RecordedAudio,
    llm::{self, CleanupContext},
    paste,
    stt,
};

pub async fn process_recording(shared: SharedState, audio: RecordedAudio) -> Result<()> {
    let config = shared.config();
    shared.set_status(
        RuntimeStatus::Processing,
        format!("Transcribing {} samples", audio.sample_count),
    );

    let stt_provider = stt::build_provider(&config).context("failed to build STT provider")?;
    let raw_text = stt_provider
        .transcribe(&audio.bytes, audio.format)
        .await
        .with_context(|| format!("{} transcription failed", stt_provider.name()))?;

    anyhow::ensure!(!raw_text.trim().is_empty(), "transcription returned no text");

    let cleaned_text = if let Some(llm_provider) = llm::build_provider(&config)? {
        llm_provider
            .clean(
                &raw_text,
                &CleanupContext {
                    target_app: None,
                    mode_hint: Some("general dictation".to_string()),
                },
            )
            .await
            .with_context(|| format!("{} cleanup failed", llm_provider.name()))?
    } else {
        raw_text.clone()
    };

    paste::paste_text(&cleaned_text, &config.paste).context("failed to paste transcript")?;
    shared.set_last_transcript(cleaned_text.clone());
    shared.set_status(RuntimeStatus::Idle, "Ready for dictation");
    Ok(())
}

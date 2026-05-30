use std::sync::Arc;

use tokio::runtime::Runtime;

use crate::{
    apple_helper,
    config::{GlideConfig, ModelSelection, Provider},
    llm,
    state::SharedState,
    stt,
};

pub(crate) fn start_app_prewarm(shared: SharedState, runtime: Arc<Runtime>) {
    let config = shared.config();
    std::mem::drop(runtime.spawn(async move {
        run_blocking_prewarm("app", move || prewarm_app_config(&config)).await;
    }));
}

pub(crate) fn start_recording_prewarm(
    shared: SharedState,
    runtime: Arc<Runtime>,
    target_app: Option<String>,
) {
    let config = shared.config();
    std::mem::drop(runtime.spawn(async move {
        run_blocking_prewarm("recording", move || {
            prewarm_effective_recording_config(&config, target_app.as_deref())
        })
        .await;
    }));
}

async fn run_blocking_prewarm(label: &'static str, f: impl FnOnce() + Send + 'static) {
    if let Err(error) = tokio::task::spawn_blocking(f).await {
        eprintln!("[glide] provider prewarm {label} task failed: {error:#}");
    }
}

fn prewarm_app_config(config: &GlideConfig) {
    prewarm_stt_selection(&config.dictation.stt);

    if let Some(llm) = &config.dictation.llm {
        prewarm_llm_selection(llm, &config.dictation.system_prompt);
    }

    for style in &config.dictation.styles {
        if style.apps.is_empty() {
            continue;
        }

        let effective_stt = style.stt.as_ref().unwrap_or(&config.dictation.stt);
        prewarm_stt_selection(effective_stt);

        let effective_llm = style.llm.as_ref().or(config.dictation.llm.as_ref());
        if let Some(llm) = effective_llm {
            prewarm_llm_selection(llm, &style.prompt);
        }
    }
}

fn prewarm_effective_recording_config(config: &GlideConfig, target_app: Option<&str>) {
    let matched_style = target_app.and_then(|target| {
        config.dictation.styles.iter().find(|style| {
            style
                .apps
                .iter()
                .any(|app| app.eq_ignore_ascii_case(target))
        })
    });

    let effective_stt = matched_style
        .and_then(|style| style.stt.as_ref())
        .unwrap_or(&config.dictation.stt);
    prewarm_stt_selection(effective_stt);

    let effective_llm = matched_style
        .and_then(|style| style.llm.as_ref())
        .or(config.dictation.llm.as_ref());
    let system_prompt = matched_style
        .map(|style| style.prompt.as_str())
        .unwrap_or(&config.dictation.system_prompt);

    if let Some(llm) = effective_llm {
        prewarm_llm_selection(llm, system_prompt);
    }
}

fn prewarm_stt_selection(selection: &ModelSelection) {
    if selection.provider != Provider::Parakeet {
        return;
    }

    if let Err(error) = stt::prewarm_provider(selection.provider, &selection.model) {
        eprintln!(
            "[glide] Parakeet prewarm failed for {}: {error:#}",
            selection.model
        );
    }
}

fn prewarm_llm_selection(selection: &ModelSelection, system_prompt: &str) {
    if selection.provider != Provider::AppleLocal {
        return;
    }

    let system_prompt = llm::build_cleanup_system_prompt(system_prompt);
    if let Err(error) = apple_helper::prewarm_foundation(&selection.model, &system_prompt) {
        eprintln!(
            "[glide] Apple Foundation prewarm failed for {}: {error:#}",
            selection.model
        );
    }
}

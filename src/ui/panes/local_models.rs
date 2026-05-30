use std::time::Duration;

use gpui::prelude::*;
use gpui::{Animation, AnimationExt as _, SharedString, div, percentage};
use gpui_component::ActiveTheme;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Icon, IconName};

use crate::config::Provider;

use super::super::SettingsApp;

pub(super) fn animated_loader(id: SharedString) -> impl IntoElement {
    Icon::new(IconName::Loader).xsmall().with_animation(
        id,
        Animation::new(Duration::from_millis(900)).repeat(),
        |icon, delta| icon.rotate(percentage(delta)),
    )
}

pub(super) fn local_model_row(
    label: &str,
    detail: &str,
    available: bool,
    reason: &str,
    cx: &mut gpui::Context<SettingsApp>,
) -> gpui::Div {
    let status = if available {
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                Icon::new(IconName::Check)
                    .xsmall()
                    .text_color(gpui::rgb(0x22C55E)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Available"),
            )
            .into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                Icon::new(IconName::CircleX)
                    .xsmall()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .max_w(gpui::px(220.0))
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .truncate()
                    .child(reason.to_string()),
            )
            .into_any_element()
    };

    div()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(detail.to_string()),
                ),
        )
        .child(status)
}

pub(super) fn apple_speech_progress_label(progress: Option<f64>) -> String {
    progress
        .map(|progress| format!("{:.0}%", (progress * 100.0).clamp(0.0, 100.0)))
        .unwrap_or_else(|| "Preparing".to_string())
}

pub(super) fn apple_speech_model_row(
    status: crate::local_models::AppleSpeechModelStatus,
    show_not_installed: bool,
    entity: gpui::WeakEntity<SettingsApp>,
    cx: &mut gpui::App,
) -> Option<gpui::Div> {
    let model_id = status.definition.id.clone();
    let detail = if status.definition.locale_id.is_empty() {
        status.definition.asset_status.clone()
    } else {
        format!(
            "{} · {}",
            status.definition.locale_id, status.definition.asset_status
        )
    };

    let action = match status.state.clone() {
        crate::local_models::AppleSpeechInstallState::NotInstalled => {
            if !show_not_installed {
                return None;
            }
            let model_id = model_id.clone();
            let entity = entity.clone();
            Button::new(SharedString::from(format!("download-apple-{model_id}")))
                .label("Download")
                .icon(IconName::ArrowDown)
                .small()
                .compact()
                .on_click(move |_, _, cx| {
                    let _ = crate::local_models::start_apple_speech_model_download(&model_id);
                    let _ = entity.update(cx, |_this, cx| {
                        poll_apple_speech_downloads(cx);
                        cx.notify();
                    });
                })
                .into_any_element()
        }
        crate::local_models::AppleSpeechInstallState::Downloading { progress } => div()
            .flex()
            .items_center()
            .gap_2()
            .child(animated_loader(SharedString::from(format!(
                "apple-download-spinner-{model_id}"
            ))))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(apple_speech_progress_label(progress)),
            )
            .into_any_element(),
        crate::local_models::AppleSpeechInstallState::Cancelling => div()
            .flex()
            .items_center()
            .gap_2()
            .child(animated_loader(SharedString::from(format!(
                "apple-cancel-spinner-{model_id}"
            ))))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Cancelling..."),
            )
            .into_any_element(),
        crate::local_models::AppleSpeechInstallState::Installed => {
            let model_id = model_id.clone();
            let entity = entity.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("Installed"),
                )
                .child(
                    Button::new(SharedString::from(format!("delete-apple-{model_id}")))
                        .icon(IconName::Close)
                        .small()
                        .compact()
                        .ghost()
                        .tooltip("Delete locale")
                        .on_click(move |_, _, cx| {
                            let _ = crate::local_models::release_apple_speech_model(&model_id);
                            let deleted_model = model_id.clone();
                            let _ = entity.update(cx, |this, cx| {
                                let _ = this.shared.update_config(|config| {
                                    if config.dictation.stt.provider == Provider::AppleLocal
                                        && config.dictation.stt.model == deleted_model
                                        && let Some(default) =
                                            crate::model_catalog::smart_stt_default()
                                    {
                                        config.dictation.stt = default;
                                    }
                                    for style in &mut config.dictation.styles {
                                        if style
                                            .stt
                                            .as_ref()
                                            .map(|selection| {
                                                selection.provider == Provider::AppleLocal
                                                    && selection.model == deleted_model
                                            })
                                            .unwrap_or(false)
                                        {
                                            style.stt = None;
                                        }
                                    }
                                });
                                cx.notify();
                            });
                        }),
                )
                .into_any_element()
        }
        crate::local_models::AppleSpeechInstallState::Failed(error) => {
            let model_id = model_id.clone();
            let entity = entity.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .max_w(gpui::px(220.0))
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
                .child(
                    Button::new(SharedString::from(format!("retry-apple-{model_id}")))
                        .label("Retry")
                        .small()
                        .compact()
                        .on_click(move |_, _, cx| {
                            let _ =
                                crate::local_models::start_apple_speech_model_download(&model_id);
                            let _ = entity.update(cx, |_this, cx| {
                                poll_apple_speech_downloads(cx);
                                cx.notify();
                            });
                        }),
                )
                .into_any_element()
        }
    };

    Some(
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .truncate()
                            .child(status.definition.display_name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .truncate()
                            .child(detail),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end()
                    .flex_shrink_0()
                    .child(action),
            ),
    )
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ParakeetRowActionKind {
    Download,
    Cancel,
    None,
    Delete,
    Retry,
}

#[cfg(test)]
pub(super) fn parakeet_row_action_kind(
    state: &crate::local_models::LocalModelInstallState,
) -> ParakeetRowActionKind {
    match state {
        crate::local_models::LocalModelInstallState::NotInstalled => {
            ParakeetRowActionKind::Download
        }
        crate::local_models::LocalModelInstallState::Downloading { .. } => {
            ParakeetRowActionKind::Cancel
        }
        crate::local_models::LocalModelInstallState::Cancelling { .. } => {
            ParakeetRowActionKind::None
        }
        crate::local_models::LocalModelInstallState::Installed { .. } => {
            ParakeetRowActionKind::Delete
        }
        crate::local_models::LocalModelInstallState::Failed(_) => ParakeetRowActionKind::Retry,
    }
}

pub(super) fn parakeet_model_row(
    status: crate::local_models::ParakeetModelStatus,
    cx: &mut gpui::Context<SettingsApp>,
) -> gpui::Div {
    let model_id = status.definition.id.to_string();
    let state = status.state.clone();
    let action = match state {
        crate::local_models::LocalModelInstallState::NotInstalled => {
            let model_id = model_id.clone();
            Button::new(SharedString::from(format!("download-{model_id}")))
                .label("Download")
                .icon(IconName::ArrowDown)
                .small()
                .compact()
                .on_click(cx.listener(move |_this, _, _window, cx| {
                    let _ = crate::local_models::start_parakeet_download(&model_id);
                    poll_parakeet_downloads(cx);
                    cx.notify();
                }))
                .into_any_element()
        }
        crate::local_models::LocalModelInstallState::Downloading {
            downloaded_bytes,
            total_bytes,
        } => {
            let cancel_model_id = model_id.clone();
            let label = if let Some(total) = total_bytes {
                format!(
                    "{} / {}",
                    format_bytes(downloaded_bytes),
                    format_bytes(total)
                )
            } else {
                format!("{} downloaded", format_bytes(downloaded_bytes))
            };
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(animated_loader(SharedString::from(format!(
                    "download-spinner-{model_id}"
                ))))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(label),
                )
                .child(
                    Button::new(SharedString::from(format!("cancel-{model_id}")))
                        .icon(IconName::Close)
                        .small()
                        .compact()
                        .ghost()
                        .tooltip("Cancel download")
                        .on_click(cx.listener(move |_this, _, _window, cx| {
                            let _ = crate::local_models::cancel_parakeet_download(&cancel_model_id);
                            poll_parakeet_downloads(cx);
                            cx.notify();
                        })),
                )
                .into_any_element()
        }
        crate::local_models::LocalModelInstallState::Cancelling { .. } => div()
            .flex()
            .items_center()
            .gap_2()
            .child(animated_loader(SharedString::from(format!(
                "cancel-spinner-{model_id}"
            ))))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child("Cancelling..."),
            )
            .into_any_element(),
        crate::local_models::LocalModelInstallState::Installed { size_bytes } => {
            let model_id = model_id.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format_bytes(size_bytes)),
                )
                .child(
                    Button::new(SharedString::from(format!("delete-{model_id}")))
                        .icon(IconName::Close)
                        .small()
                        .compact()
                        .danger()
                        .tooltip("Delete model")
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            let _ = crate::local_models::delete_parakeet_model(&model_id);
                            let deleted_model = model_id.clone();
                            let _ = this.shared.update_config(|config| {
                                if config.dictation.stt.provider == Provider::Parakeet
                                    && config.dictation.stt.model == deleted_model
                                    && let Some(default) = crate::model_catalog::smart_stt_default()
                                {
                                    config.dictation.stt = default;
                                }
                                for style in &mut config.dictation.styles {
                                    if style
                                        .stt
                                        .as_ref()
                                        .map(|selection| {
                                            selection.provider == Provider::Parakeet
                                                && selection.model == deleted_model
                                        })
                                        .unwrap_or(false)
                                    {
                                        style.stt = None;
                                    }
                                }
                            });
                            cx.notify();
                        })),
                )
                .into_any_element()
        }
        crate::local_models::LocalModelInstallState::Failed(error) => {
            let model_id = model_id.clone();
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .max_w(gpui::px(220.0))
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
                .child(
                    Button::new(SharedString::from(format!("retry-{model_id}")))
                        .label("Retry")
                        .small()
                        .compact()
                        .on_click(cx.listener(move |_this, _, _window, cx| {
                            let _ = crate::local_models::start_parakeet_download(&model_id);
                            poll_parakeet_downloads(cx);
                            cx.notify();
                        })),
                )
                .into_any_element()
        }
    };

    div()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(status.definition.label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate()
                        .child(status.definition.language.to_string()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .flex_shrink_0()
                .child(action),
        )
}

pub(super) fn poll_parakeet_downloads(cx: &mut gpui::Context<SettingsApp>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(750))
                .await;
            let downloading = crate::local_models::parakeet_models_status()
                .iter()
                .any(|status| {
                    matches!(
                        status.state,
                        crate::local_models::LocalModelInstallState::Downloading { .. }
                            | crate::local_models::LocalModelInstallState::Cancelling { .. }
                    )
                });
            let _ = this.update(cx, |_this, cx| cx.notify());
            if !downloading {
                break;
            }
        }
    })
    .detach();
}

pub(super) fn poll_apple_speech_downloads(cx: &mut gpui::Context<SettingsApp>) {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(750))
                .await;
            let downloading = crate::local_models::apple_speech_has_active_downloads();
            let _ = this.update(cx, |_this, cx| cx.notify());
            if !downloading {
                break;
            }
        }
    })
    .detach();
}

pub(super) fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.0} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_models::LocalModelInstallState;

    #[test]
    fn parakeet_row_action_kind_matches_install_state() {
        let cases = [
            (
                LocalModelInstallState::Downloading {
                    downloaded_bytes: 512,
                    total_bytes: Some(1024),
                },
                ParakeetRowActionKind::Cancel,
            ),
            (
                LocalModelInstallState::Installed { size_bytes: 4096 },
                ParakeetRowActionKind::Delete,
            ),
            (
                LocalModelInstallState::Cancelling {
                    downloaded_bytes: 512,
                    total_bytes: Some(1024),
                },
                ParakeetRowActionKind::None,
            ),
        ];

        for (state, expected) in cases {
            assert_eq!(parakeet_row_action_kind(&state), expected);
        }
    }
}

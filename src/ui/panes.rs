use std::time::Duration;

use gpui::prelude::*;
use gpui::{Animation, AnimationExt as _, SharedString, div, img, percentage};
use gpui_component::ActiveTheme;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName};

use crate::app::AppSnapshot;
use crate::config::{
    ColorAccent, HotkeyTrigger, ModelSelection, OverlayStyle, Style, ThemePreference,
};

use super::helpers::*;
use super::{ProviderInputs, SettingsApp, apply_theme_preference};
use crate::config::Provider;

fn animated_loader(id: SharedString) -> impl IntoElement {
    Icon::new(IconName::Loader).xsmall().with_animation(
        id,
        Animation::new(Duration::from_millis(900)).repeat(),
        |icon, delta| icon.rotate(percentage(delta)),
    )
}

fn local_model_row(
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

fn apple_speech_progress_label(progress: Option<f64>) -> String {
    progress
        .map(|progress| format!("{:.0}%", (progress * 100.0).clamp(0.0, 100.0)))
        .unwrap_or_else(|| "Preparing".to_string())
}

fn apple_speech_model_row(
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
                                        && let Some(default) = crate::config::smart_stt_default()
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
enum ParakeetRowActionKind {
    Download,
    Cancel,
    None,
    Delete,
    Retry,
}

#[cfg(test)]
fn parakeet_row_action_kind(
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

fn parakeet_model_row(
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
                                    && let Some(default) = crate::config::smart_stt_default()
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

fn poll_parakeet_downloads(cx: &mut gpui::Context<SettingsApp>) {
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

fn poll_apple_speech_downloads(cx: &mut gpui::Context<SettingsApp>) {
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

fn format_bytes(bytes: u64) -> String {
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

impl SettingsApp {
    pub(super) fn render_providers_pane(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        _snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        let mut container = div().flex().flex_col().gap_2();

        let remote_providers: [(Provider, &ProviderInputs); 2] = [
            (Provider::OpenAi, &self.openai_inputs),
            (Provider::Groq, &self.groq_inputs),
        ];

        for (idx, (provider, inputs)) in remote_providers.iter().enumerate() {
            let is_expanded = self.expanded_provider == Some(idx);
            let logo_path = Some(crate::config::asset_path(provider.logo()));

            let chevron = if is_expanded { "▼" } else { "▶" };
            let header = div()
                .id(SharedString::from(format!("provider-header-{idx}")))
                .flex()
                .items_center()
                .gap_3()
                .px_4()
                .py_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(chevron),
                )
                .when_some(logo_path.clone(), |this, path| {
                    this.child(img(path).w(gpui::px(24.0)).h(gpui::px(24.0)).rounded_md())
                })
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .child(provider.label()),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.expanded_provider = if this.expanded_provider == Some(idx) {
                        None
                    } else {
                        Some(idx)
                    };
                    cx.notify();
                }));

            let is_verified = crate::config::provider_verified(*provider);
            let body = if is_expanded {
                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_4()
                        .py_3()
                        .ml_4()
                        .border_l_2()
                        .border_color(cx.theme().border)
                        .child(field_label("API Key", cx))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().flex_1().child(
                                    Input::new(&inputs.api_key).mask_toggle().cleanable(true),
                                ))
                                .when(is_verified, |this| {
                                    this.child(
                                        Icon::new(IconName::Check)
                                            .xsmall()
                                            .text_color(gpui::rgb(0x22C55E)),
                                    )
                                }),
                        )
                        .child(field_label("Base URL", cx))
                        .child(Input::new(&inputs.base_url)),
                )
            } else {
                None
            };

            container = container.child(
                div()
                    .flex()
                    .flex_col()
                    .child(header)
                    .when_some(body, |this, body| this.child(body)),
            );
        }

        container = container
            .child(self.render_apple_local_provider_card(cx, 2))
            .child(self.render_parakeet_provider_card(cx, 3));

        container
    }

    fn render_apple_local_provider_card(
        &self,
        cx: &mut gpui::Context<Self>,
        idx: usize,
    ) -> impl IntoElement {
        let provider = Provider::AppleLocal;
        let is_expanded = self.expanded_provider == Some(idx);
        let caps = crate::apple_helper::cached_capabilities();
        let speech_statuses = crate::local_models::apple_speech_models_status();
        let foundation_statuses = crate::local_models::apple_foundation_models_status();
        let active_count = speech_statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.state,
                    crate::local_models::AppleSpeechInstallState::Downloading { .. }
                        | crate::local_models::AppleSpeechInstallState::Cancelling
                )
            })
            .count();
        let header_status = if active_count > 0 {
            Some(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(animated_loader(SharedString::from(
                        "apple-header-download-spinner",
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{active_count} downloading")),
                    )
                    .into_any_element(),
            )
        } else {
            None
        };
        let chevron = if is_expanded { "▼" } else { "▶" };

        let header = div()
            .id(SharedString::from(format!("provider-header-{idx}")))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(chevron),
            )
            .child(
                img(crate::config::asset_path(provider.logo()))
                    .w(gpui::px(24.0))
                    .h(gpui::px(24.0))
                    .rounded_md(),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(provider.label()),
            )
            .child(div().flex_1())
            .when_some(header_status, |this, header_status| {
                this.child(header_status)
            })
            .on_click(cx.listener(move |this, _, _window, cx| {
                if this.expanded_provider != Some(idx) {
                    crate::local_models::refresh_apple_local_models();
                }
                this.expanded_provider = if this.expanded_provider == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                cx.notify();
            }));

        let body = is_expanded.then(|| {
            let add_popover_statuses = speech_statuses.clone();
            let speech_unavailable_reason =
                crate::local_models::apple_speech_models_unavailable_reason();
            let add_search = self.apple_speech_search.clone();
            let settings_entity = cx.weak_entity();
            let popover_entity = settings_entity.clone();
            let popover_empty_message = speech_unavailable_reason
                .clone()
                .unwrap_or_else(|| "No speech locales found".to_string());
            let add_popover = Popover::new("apple-speech-model-picker")
                .trigger(
                    Button::new("add-apple-speech-model")
                        .icon(IconName::Plus)
                        .small()
                        .compact(),
                )
                .content(move |_state, _window, cx| {
                    let query = add_search.read(cx).value().to_string();
                    let mut filtered: Vec<_> = if query.trim().is_empty() {
                        add_popover_statuses.clone()
                    } else {
                        let mut scored: Vec<_> = add_popover_statuses
                            .iter()
                            .filter_map(|status| {
                                let label = &status.definition.display_name;
                                let locale = &status.definition.locale_id;
                                let label_score = crate::config::fuzzy_match(&query, label);
                                let locale_score = crate::config::fuzzy_match(&query, locale);
                                label_score
                                    .or(locale_score)
                                    .map(|score| (status.clone(), score))
                            })
                            .collect();
                        scored.sort_by(|a, b| b.1.cmp(&a.1));
                        scored.into_iter().map(|(status, _)| status).collect()
                    };
                    if query.trim().is_empty() {
                        filtered.sort_by(|a, b| {
                            a.definition.display_name.cmp(&b.definition.display_name)
                        });
                    }

                    let mut list = div().flex().flex_col().gap_1().p_1();
                    let filtered_count = filtered.len();
                    for status in filtered {
                        if let Some(row) =
                            apple_speech_model_row(status, true, popover_entity.clone(), cx)
                        {
                            list = list.child(row);
                        }
                    }
                    if filtered_count == 0 {
                        list = list.child(
                            div()
                                .px_3()
                                .py_2()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(popover_empty_message.clone()),
                        );
                    }

                    div()
                        .w(gpui::px(440.0))
                        .max_h(gpui::px(420.0))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .child(
                            div()
                                .p_2()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(Input::new(&add_search).prefix(IconName::Search)),
                        )
                        .child(
                            div()
                                .id(SharedString::from("apple-speech-model-scroll"))
                                .flex_1()
                                .overflow_y_scroll()
                                .child(list),
                        )
                        .into_any_element()
                });

            let mut speech_rows = div().flex().flex_col().gap_2();
            let mut visible_count = 0usize;
            for status in speech_statuses {
                if let Some(row) =
                    apple_speech_model_row(status, false, settings_entity.clone(), cx)
                {
                    visible_count += 1;
                    speech_rows = speech_rows.child(row);
                }
            }
            if visible_count == 0 {
                let empty_message = if let Some(reason) = speech_unavailable_reason {
                    reason
                } else if caps.apple_speech_available {
                    "No speech locales installed".to_string()
                } else {
                    caps.apple_speech_reason.clone()
                };
                speech_rows = speech_rows.child(
                    div()
                        .w_full()
                        .px_3()
                        .py_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(empty_message),
                );
            }

            let mut foundation_rows = div().flex().flex_col().gap_2();
            if foundation_statuses.is_empty() {
                foundation_rows = foundation_rows.child(local_model_row(
                    "Apple Foundation Model",
                    "SystemLanguageModel.default",
                    false,
                    &caps.foundation_models_reason,
                    cx,
                ));
            } else {
                for status in foundation_statuses {
                    foundation_rows = foundation_rows.child(local_model_row(
                        &status.display_name,
                        &status.model_name,
                        status.available,
                        &status.reason,
                        cx,
                    ));
                }
            }

            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .ml_4()
                .border_l_2()
                .border_color(cx.theme().border)
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .child("Foundation Models"),
                )
                .child(foundation_rows)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .pt_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child("Speech Locales"),
                        )
                        .child(div().flex_1())
                        .child(add_popover),
                )
                .child(speech_rows)
        });

        div()
            .flex()
            .flex_col()
            .child(header)
            .when_some(body, |this, body| this.child(body))
    }

    fn render_parakeet_provider_card(
        &self,
        cx: &mut gpui::Context<Self>,
        idx: usize,
    ) -> impl IntoElement {
        let provider = Provider::Parakeet;
        let is_expanded = self.expanded_provider == Some(idx);
        let statuses = crate::local_models::parakeet_models_status();
        let installed_count = statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.state,
                    crate::local_models::LocalModelInstallState::Installed { .. }
                )
            })
            .count();
        let downloading_count = statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.state,
                    crate::local_models::LocalModelInstallState::Downloading { .. }
                )
            })
            .count();
        let cancelling_count = statuses
            .iter()
            .filter(|status| {
                matches!(
                    status.state,
                    crate::local_models::LocalModelInstallState::Cancelling { .. }
                )
            })
            .count();
        let header_status = if installed_count > 0 {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Icon::new(IconName::Check)
                        .xsmall()
                        .text_color(gpui::rgb(0x22C55E)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{installed_count} installed")),
                )
                .into_any_element()
        } else if cancelling_count > 0 {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(animated_loader(SharedString::from(
                    "parakeet-header-cancelling-spinner",
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{cancelling_count} cancelling")),
                )
                .into_any_element()
        } else if downloading_count > 0 {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(animated_loader(SharedString::from(
                    "parakeet-header-download-spinner",
                )))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{downloading_count} downloading")),
                )
                .into_any_element()
        } else {
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} available", statuses.len()))
                .into_any_element()
        };
        let chevron = if is_expanded { "▼" } else { "▶" };

        let header = div()
            .id(SharedString::from(format!("provider-header-{idx}")))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(chevron),
            )
            .child(
                img(crate::config::asset_path(provider.logo()))
                    .w(gpui::px(24.0))
                    .h(gpui::px(24.0))
                    .rounded_md(),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(cx.theme().foreground)
                    .child(provider.label()),
            )
            .child(div().flex_1())
            .child(header_status)
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.expanded_provider = if this.expanded_provider == Some(idx) {
                    None
                } else {
                    Some(idx)
                };
                cx.notify();
            }));

        let body = is_expanded.then(|| {
            let mut rows = div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .ml_4()
                .border_l_2()
                .border_color(cx.theme().border);

            for status in statuses {
                rows = rows.child(parakeet_model_row(status, cx));
            }
            rows
        });

        div()
            .flex()
            .flex_col()
            .child(header)
            .when_some(body, |this, body| this.child(body))
    }

    pub(super) fn render_styles_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let available_apps = crate::config::list_applications();
        let mut container = div().flex().flex_col().gap_4();

        // -- Default Prompt & Models --
        let snapshot = self.shared.snapshot();
        let default_stt = snapshot.config.dictation.stt.model.clone();
        let default_llm = snapshot
            .config
            .dictation
            .llm
            .as_ref()
            .map(|s| s.model.clone())
            .unwrap_or_else(|| "Disabled".to_string());
        let stt_models = crate::config::cached_stt_models();
        let llm_models = crate::config::cached_llm_models();

        let shared_stt = self.shared.clone();
        let shared_llm = self.shared.clone();
        let has_provider = crate::config::any_provider_verified();

        let prompt_expanded = self.prompt_expanded;
        let default_prompt_entity = self.default_prompt.clone();
        let prompt_chevron = if prompt_expanded { "▼" } else { "▶" };
        let prompt_header = div()
            .id("prompt-accordion")
            .flex()
            .items_center()
            .gap_2()
            .cursor_pointer()
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(prompt_chevron),
            )
            .child(field_label("System Prompt", cx))
            .on_click(cx.listener(|this, _, _window, cx| {
                this.prompt_expanded = !this.prompt_expanded;
                cx.notify();
            }));

        container = container.child(
            section_block("Defaults", cx).child(
                settings_card(cx)
                    .child(hint_row(
                        "Used for all applications unless a style overrides it.",
                        cx,
                    ))
                    .child(prompt_header)
                    .when(prompt_expanded, |this| {
                        this.child(Input::new(&default_prompt_entity))
                    })
                    .when(!has_provider, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .py_4()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("Add a provider in the Providers tab to configure models."),
                        )
                    })
                    .when(has_provider, |this| {
                        let recommended_stt = crate::config::smart_stt_default().map(|s| s.model);
                        let recommended_llm = crate::config::smart_llm_default().map(|s| s.model);
                        this.child(setting_row("Voice Model", "Speech-to-text", cx).child(
                            model_dropdown_button(
                                "default-stt",
                                &default_stt,
                                &stt_models,
                                recommended_stt.as_deref(),
                                shared_stt,
                                self.default_stt_search.clone(),
                                |config, model, provider| {
                                    config.dictation.stt = ModelSelection { provider, model };
                                },
                            ),
                        ))
                        .child(
                            setting_row("LLM Model", "Text cleanup", cx).child(
                                model_dropdown_button(
                                    "default-llm",
                                    &default_llm,
                                    &llm_models,
                                    recommended_llm.as_deref(),
                                    shared_llm,
                                    self.default_llm_search.clone(),
                                    |config, model, provider| {
                                        config.dictation.llm =
                                            Some(ModelSelection { provider, model });
                                    },
                                ),
                            ),
                        )
                    }),
            ),
        );

        // -- Styles (accordion cards) --
        for (index, style) in self.styles.iter().enumerate() {
            let is_expanded = self.expanded_style == Some(index);
            let style_prompt_expanded = style.prompt_expanded;
            let name_input = Input::new(&style.name)
                .appearance(false)
                .bordered(false)
                .focus_bordered(false)
                .small();

            let header = div()
                .id(SharedString::from(format!("style-header-{index}")))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_4()
                .py_3()
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .hover(|s| s.bg(cx.theme().accent.opacity(0.1)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .flex_1()
                        .min_w_0()
                        .child(
                            Button::new(SharedString::from(format!("toggle-style-{index}")))
                                .icon(if is_expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .ghost()
                                .xsmall()
                                .compact()
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    this.expanded_style = if this.expanded_style == Some(index) {
                                        None
                                    } else {
                                        Some(index)
                                    };
                                    cx.notify();
                                })),
                        )
                        .child(div().flex_1().min_w_0().child(name_input)),
                )
                .child(
                    Button::new(SharedString::from(format!("remove-style-{index}")))
                        .icon(IconName::Close)
                        .ghost()
                        .small()
                        .compact()
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.styles.remove(index);
                            this.expanded_style = match this.expanded_style {
                                Some(current) if current == index => None,
                                Some(current) if current > index => Some(current - 1),
                                current => current,
                            };
                            this.schedule_autosave(cx);
                            cx.notify();
                        })),
                );

            let body = if is_expanded {
                let apps = &style.apps;
                let max_shown = 5;
                let mut app_row = div().flex().items_center().gap_1().flex_wrap();
                for (ai, app) in apps.iter().take(max_shown).enumerate() {
                    let app_clone = app.clone();
                    let icon_el = if let Some(icon_path) = crate::config::app_icon_path(app) {
                        img(icon_path)
                            .w(gpui::px(20.0))
                            .h(gpui::px(20.0))
                            .rounded_sm()
                            .into_any_element()
                    } else {
                        let ch = app.chars().next().unwrap_or('?').to_uppercase().to_string();
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(gpui::px(20.0))
                            .h(gpui::px(20.0))
                            .rounded_sm()
                            .bg(cx.theme().accent.opacity(0.2))
                            .text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(cx.theme().accent_foreground)
                            .child(ch)
                            .into_any_element()
                    };
                    app_row = app_row.child(
                        div()
                            .id(SharedString::from(format!("app-{index}-{ai}")))
                            .flex()
                            .items_center()
                            .gap_1()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .bg(cx.theme().secondary)
                            .border_1()
                            .border_color(cx.theme().border)
                            .text_xs()
                            .child(icon_el)
                            .child(div().text_color(cx.theme().foreground).child(app.clone()))
                            .child(
                                Button::new(SharedString::from(format!("rm-app-{index}-{ai}")))
                                    .label("×")
                                    .ghost()
                                    .xsmall()
                                    .compact()
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.styles[index].apps.retain(|a| a != &app_clone);
                                        this.schedule_autosave(cx);
                                        cx.notify();
                                    })),
                            ),
                    );
                }
                if apps.len() > max_shown {
                    app_row = app_row.child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("+{}", apps.len() - max_shown)),
                    );
                }

                let snap = self.shared.snapshot();
                let all_assigned: std::collections::HashSet<String> = snap
                    .config
                    .dictation
                    .styles
                    .iter()
                    .flat_map(|s| s.apps.iter().cloned())
                    .collect();
                let popover_apps: Vec<String> = available_apps
                    .iter()
                    .filter(|a| !all_assigned.contains(*a))
                    .cloned()
                    .collect();
                let entity_for_apps = cx.weak_entity();
                let search_entity = style.search.clone();
                let add_app_popover =
                    Popover::new(SharedString::from(format!("app-picker-{index}")))
                        .trigger(
                            Button::new(SharedString::from(format!("add-app-{index}")))
                                .label("+ Add App")
                                .ghost()
                                .small()
                                .compact(),
                        )
                        .content(move |_state, _window, cx| {
                            let query = search_entity.read(cx).value().to_string();
                            let mut scored: Vec<(&String, i32)> = popover_apps
                                .iter()
                                .filter_map(|a| {
                                    crate::config::fuzzy_match(&query, a).map(|s| (a, s))
                                })
                                .collect();
                            scored.sort_by(|a, b| b.1.cmp(&a.1));
                            let mut grid = div().flex().flex_wrap().gap_2().p_2();
                            for (app, _) in &scored {
                                let app_name = (*app).clone();
                                let entity = entity_for_apps.clone();
                                let tile_icon = if let Some(p) = crate::config::app_icon_path(app) {
                                    img(p)
                                        .w(gpui::px(40.0))
                                        .h(gpui::px(40.0))
                                        .rounded_lg()
                                        .into_any_element()
                                } else {
                                    let ch = app
                                        .chars()
                                        .next()
                                        .unwrap_or('?')
                                        .to_uppercase()
                                        .to_string();
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .w(gpui::px(40.0))
                                        .h(gpui::px(40.0))
                                        .rounded_lg()
                                        .bg(cx.theme().accent.opacity(0.2))
                                        .text_lg()
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(cx.theme().accent_foreground)
                                        .child(ch)
                                        .into_any_element()
                                };
                                grid = grid.child(
                                    div()
                                        .id(SharedString::from(format!("pick-{app_name}")))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_1()
                                        .w(gpui::px(80.0))
                                        .py_2()
                                        .rounded_md()
                                        .cursor_pointer()
                                        .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
                                        .child(tile_icon)
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().foreground)
                                                .overflow_x_hidden()
                                                .max_w(gpui::px(76.0))
                                                .child(app_name.clone()),
                                        )
                                        .on_click(move |_, _, cx| {
                                            let name = app_name.clone();
                                            let _ = entity.update(cx, |this, cx| {
                                                if !this.styles[index].apps.contains(&name) {
                                                    this.styles[index].apps.push(name);
                                                    this.schedule_autosave(cx);
                                                    cx.notify();
                                                }
                                            });
                                        }),
                                );
                            }
                            div()
                                .w(gpui::px(380.0))
                                .max_h(gpui::px(400.0))
                                .flex()
                                .flex_col()
                                .overflow_hidden()
                                .child(
                                    div()
                                        .p_2()
                                        .border_b_1()
                                        .border_color(cx.theme().border)
                                        .child(Input::new(&search_entity)),
                                )
                                .child(
                                    div()
                                        .id(SharedString::from(format!("app-grid-scroll-{index}")))
                                        .flex_1()
                                        .overflow_y_scrollbar()
                                        .child(grid),
                                )
                                .into_any_element()
                        });

                let style_prompt_chevron = if style_prompt_expanded { "▼" } else { "▶" };
                let style_prompt_header = div()
                    .id(SharedString::from(format!(
                        "style-prompt-accordion-{index}"
                    )))
                    .flex()
                    .items_center()
                    .gap_2()
                    .cursor_pointer()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(style_prompt_chevron),
                    )
                    .child(field_label("System Prompt", cx))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if let Some(style) = this.styles.get_mut(index) {
                            style.prompt_expanded = !style.prompt_expanded;
                        }
                        cx.notify();
                    }));

                let cfg_styles = &snapshot.config.dictation.styles;
                let stt_display = cfg_styles
                    .get(index)
                    .and_then(|s| s.stt.as_ref())
                    .map(|sel| model_label_for(&sel.model, &stt_models))
                    .unwrap_or_else(|| {
                        format!("Default ({})", model_label_for(&default_stt, &stt_models))
                    });
                let llm_display = cfg_styles
                    .get(index)
                    .and_then(|s| s.llm.as_ref())
                    .map(|sel| model_label_for(&sel.model, &llm_models))
                    .unwrap_or_else(|| {
                        format!("Default ({})", model_label_for(&default_llm, &llm_models))
                    });

                Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .px_4()
                        .py_3()
                        .ml_4()
                        .border_l_2()
                        .border_color(cx.theme().border)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(app_row)
                                .child(add_app_popover),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(style_prompt_header)
                                .when(style_prompt_expanded, |this| {
                                    this.child(Input::new(&style.prompt))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(style_model_dropdown(
                                    &format!("style-stt-{index}"),
                                    &stt_display,
                                    &stt_models,
                                    self.shared.clone(),
                                    style.stt_model_search.clone(),
                                    index,
                                    true,
                                ))
                                .child(style_model_dropdown(
                                    &format!("style-llm-{index}"),
                                    &llm_display,
                                    &llm_models,
                                    self.shared.clone(),
                                    style.llm_model_search.clone(),
                                    index,
                                    false,
                                )),
                        ),
                )
            } else {
                None
            };

            container = container.child(
                div()
                    .flex()
                    .flex_col()
                    .child(header)
                    .when_some(body, |this, body| this.child(body)),
            );
        }

        // -- Add Style button --
        container = container.child(
            div().child(
                Button::new("add-style")
                    .label("+ Add Style")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let default_prompt = this.default_prompt.read(cx).value().to_string();
                        let style_num = this.styles.len() + 1;
                        let entry = Style {
                            name: format!("Style {style_num}"),
                            apps: Vec::new(),
                            prompt: default_prompt,
                            stt: None,
                            llm: None,
                        };
                        let (inputs, subs) = Self::create_style_inputs(&entry, window, cx);
                        this.styles.push(inputs);
                        this._subscriptions.extend(subs);
                        this.schedule_autosave(cx);
                        cx.notify();
                    })),
            ),
        );

        container
    }

    pub(super) fn render_general_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        let current_trigger = snapshot.config.hotkey.trigger;
        let current_theme = snapshot.config.app.theme;
        let current_overlay = snapshot.config.overlay.style;

        let mut container = div().flex().flex_col().gap_4();

        // -- Recording Window --
        let mut style_cards = div().flex().gap_3().flex_1();
        let is_macos = cfg!(target_os = "macos");
        let has_notch = crate::config::notch_width().is_some();
        for style in OverlayStyle::ALL {
            if style == OverlayStyle::Glow && !is_macos {
                continue;
            }
            let is_active = style == current_overlay;
            let icon_text = match style {
                OverlayStyle::Classic => "▁▂▃▅▃▂▁",
                OverlayStyle::Glow => "◎",
                OverlayStyle::None => "⊘",
            };
            style_cards = style_cards.child(
                div()
                    .id(SharedString::from(format!("overlay-{}", style.label())))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .py_4()
                    .px_6()
                    .flex_1()
                    .rounded_lg()
                    .border_2()
                    .cursor_pointer()
                    .map(|d| {
                        if is_active {
                            d.border_color(cx.theme().primary)
                        } else {
                            d.border_color(cx.theme().border)
                        }
                    })
                    .bg(cx.theme().secondary)
                    .child(
                        div()
                            .text_lg()
                            .text_color(cx.theme().foreground)
                            .child(icon_text),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(style.label()),
                    )
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        let _ = this.shared.update_config(|config| {
                            config.overlay.style = style;
                        });
                        cx.notify();
                    })),
            );
        }

        let current_position = snapshot.config.overlay.position;
        let show_position = current_overlay == OverlayStyle::Classic;
        container = container.child(
            section_block("Recording Window", cx).child(
                settings_card(cx)
                    .child(setting_row("Style", "Overlay shown while recording", cx))
                    .child(style_cards)
                    .when(show_position, |card| {
                        card.child(setting_row(
                            "Position",
                            "Where the overlay appears on screen",
                            cx,
                        ))
                        .child({
                            let mut pos_cards = div().flex().gap_3().flex_1();
                            let positions: Vec<_> = crate::config::OverlayPosition::ALL
                                .iter()
                                .filter(|p| {
                                    **p != crate::config::OverlayPosition::Notch || has_notch
                                })
                                .copied()
                                .collect();
                            for pos in positions {
                                let is_active = pos == current_position;
                                pos_cards = pos_cards.child(
                                    div()
                                        .id(SharedString::from(format!("pos-{}", pos.label())))
                                        .flex()
                                        .flex_col()
                                        .items_center()
                                        .gap_3()
                                        .py_4()
                                        .px_6()
                                        .flex_1()
                                        .rounded_lg()
                                        .border_2()
                                        .cursor_pointer()
                                        .map(|d| {
                                            if is_active {
                                                d.border_color(cx.theme().primary)
                                            } else {
                                                d.border_color(cx.theme().border)
                                            }
                                        })
                                        .bg(cx.theme().secondary)
                                        .child(position_button_preview(pos))
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(pos.label()),
                                        )
                                        .on_click(cx.listener(move |this, _, _window, cx| {
                                            let _ = this.shared.update_config(|config| {
                                                config.overlay.position = pos;
                                            });
                                            cx.notify();
                                        })),
                                );
                            }
                            pos_cards
                        })
                    }),
            ),
        );

        // -- Keyboard Shortcuts --
        let hotkey_label = current_trigger
            .map(|t| t.label())
            .unwrap_or_else(|| "Not Set".to_string());
        let toggle_trigger = snapshot.config.hotkey.toggle_trigger;
        let toggle_label = toggle_trigger
            .map(|t| t.label())
            .unwrap_or_else(|| "Not Set".to_string());

        if self.recording_hotkey {
            if let Some(code) = self.shared.poll_recorded_keycode() {
                let trigger = HotkeyTrigger::from_keycode(code);
                let _ = self.shared.update_config(|config| {
                    config.hotkey.trigger = Some(trigger);
                });
                self.recording_hotkey = false;
                self.schedule_autosave(cx);
                cx.notify();
            }
        }
        if self.recording_toggle_hotkey {
            if let Some(code) = self.shared.poll_recorded_keycode() {
                let trigger = HotkeyTrigger::from_keycode(code);
                let _ = self.shared.update_config(|config| {
                    config.hotkey.toggle_trigger = Some(trigger);
                });
                self.recording_toggle_hotkey = false;
                self.schedule_autosave(cx);
                cx.notify();
            }
        }

        let is_recording = self.recording_hotkey;
        let is_recording_toggle = self.recording_toggle_hotkey;

        let shortcut_button = if is_recording {
            Button::new("record-hotkey")
                .label("Press a key...")
                .small()
                .compact()
                .primary()
        } else {
            Button::new("record-hotkey")
                .label(SharedString::from(hotkey_label))
                .small()
                .compact()
                .ghost()
        };

        let toggle_button = if is_recording_toggle {
            Button::new("record-toggle-hotkey")
                .label("Press a key...")
                .small()
                .compact()
                .primary()
        } else {
            Button::new("record-toggle-hotkey")
                .label(SharedString::from(toggle_label))
                .small()
                .compact()
                .ghost()
        };

        fn start_hotkey_poll(
            cx: &mut gpui::Context<SettingsApp>,
            field: fn(&mut SettingsApp) -> &mut bool,
        ) {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let still_recording = this.update(cx, |this, _| *field(this)).unwrap_or(false);
                    if !still_recording {
                        break;
                    }
                    let _ = this.update(cx, |_, cx| cx.notify());
                }
            })
            .detach();
        }

        container = container.child(
            section_block("Keyboard Shortcuts", cx).child(
                settings_card(cx)
                    .child(
                        setting_row(
                            "Dictation Hotkey",
                            "Hold to record, release to transcribe",
                            cx,
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(shortcut_button.on_click(cx.listener(
                                    |this, _, _window, cx| {
                                        this.shared.start_hotkey_recording();
                                        this.recording_hotkey = true;
                                        cx.notify();
                                        start_hotkey_poll(cx, |s| &mut s.recording_hotkey);
                                    },
                                )))
                                .child(
                                    Button::new("clear-hotkey")
                                        .icon(IconName::CircleX)
                                        .ghost()
                                        .xsmall()
                                        .on_click(cx.listener(|this, _, _window, cx| {
                                            let _ = this.shared.update_config(|config| {
                                                config.hotkey.trigger = None;
                                            });
                                            this.schedule_autosave(cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                    )
                    .child(
                        setting_row(
                            "Toggle Dictation",
                            "Press once to start, press again to stop",
                            cx,
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .child(toggle_button.on_click(cx.listener(
                                    |this, _, _window, cx| {
                                        this.shared.start_hotkey_recording();
                                        this.recording_toggle_hotkey = true;
                                        cx.notify();
                                        start_hotkey_poll(cx, |s| &mut s.recording_toggle_hotkey);
                                    },
                                )))
                                .child(
                                    Button::new("clear-toggle-hotkey")
                                        .icon(IconName::CircleX)
                                        .ghost()
                                        .xsmall()
                                        .on_click(cx.listener(|this, _, _window, cx| {
                                            let _ = this.shared.update_config(|config| {
                                                config.hotkey.toggle_trigger = None;
                                            });
                                            this.schedule_autosave(cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                    ),
            ),
        );

        // -- Appearance --
        container = container.child(
            section_block("Appearance", cx).child(
                settings_card(cx)
                    .child(
                        setting_row("Appearance", "Switch between light and dark", cx).child({
                            let mut pills = div().flex().gap_1().flex_shrink_0();
                            for pref in ThemePreference::ALL {
                                let is_active = pref == current_theme;
                                pills = pills.child(
                                    Button::new(SharedString::from(format!(
                                        "theme-{}",
                                        pref.label()
                                    )))
                                    .label(pref.label())
                                    .small()
                                    .compact()
                                    .map(|btn| {
                                        if is_active {
                                            btn.primary()
                                        } else {
                                            btn.ghost()
                                        }
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let accent = this.shared.snapshot().config.app.accent;
                                        let _ = this.shared.update_config(|config| {
                                            config.app.theme = pref;
                                        });
                                        apply_theme_preference(pref, accent, Some(window), cx);
                                        cx.notify();
                                    })),
                                );
                            }
                            pills
                        }),
                    )
                    .child(div().h(gpui::px(1.0)).bg(cx.theme().border))
                    .child(setting_row("Theme", "Choose your accent color", cx).child({
                        let current_accent = snapshot.config.app.accent;
                        let mut pills = div().flex().gap_1().flex_shrink_0();
                        for accent in ColorAccent::ALL {
                            let is_active = accent == current_accent;
                            pills =
                                pills.child(
                                    Button::new(SharedString::from(format!(
                                        "accent-{}",
                                        accent.label()
                                    )))
                                    .label(accent.label())
                                    .small()
                                    .compact()
                                    .map(|btn| {
                                        if is_active {
                                            btn.primary()
                                        } else {
                                            btn.ghost()
                                        }
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        let pref = this.shared.snapshot().config.app.theme;
                                        let _ = this.shared.update_config(|config| {
                                            config.app.accent = accent;
                                        });
                                        apply_theme_preference(pref, accent, Some(window), cx);
                                        cx.notify();
                                    })),
                                );
                        }
                        pills
                    })),
            ),
        );

        // -- Permissions --
        let perms = crate::permissions::check_all();
        let mut perm_card = settings_card(cx);
        for (i, perm) in perms.iter().enumerate() {
            if i > 0 {
                perm_card = perm_card.child(div().h(gpui::px(1.0)).bg(cx.theme().border));
            }
            let feature_icon = match perm.icon {
                "bell" => Icon::new(IconName::Bell).size_4(),
                "user" => Icon::new(IconName::User).size_4(),
                "eye" => Icon::new(IconName::Eye).size_4(),
                _ => Icon::new(IconName::Info).size_4(),
            };

            let url = perm.settings_url;
            let right_side = if perm.granted {
                Icon::new(IconName::CircleCheck).size_4().into_any_element()
            } else {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(IconName::CircleX).size_4())
                    .child(
                        Button::new(SharedString::from(format!("open-{}", perm.name)))
                            .label("Open Settings")
                            .small()
                            .compact()
                            .danger()
                            .on_click(cx.listener(move |_this, _, _window, _cx| {
                                let _ = std::process::Command::new("open").arg(url).spawn();
                            })),
                    )
                    .into_any_element()
            };

            perm_card = perm_card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(feature_icon)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child(perm.name),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(perm.description),
                                    ),
                            ),
                    )
                    .child(right_side),
            );
        }

        container = container.child(section_block("Permissions", cx).child(perm_card));

        container
    }

    pub(super) fn render_dictionary_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let snapshot = self.shared.snapshot();
        let mut container = div().flex().flex_col().gap_4();

        // -- Vocabulary Section --
        let mut vocab_card = settings_card(cx)
            .child(hint_row(
                "Words and phrases that help the transcription model recognize specific terms, names, and acronyms.",
                cx,
            ))
            .gap_2();

        let vocab = &snapshot.config.dictionary.vocabulary;
        if !vocab.is_empty() {
            let mut chips = div().flex().flex_wrap().gap_1().py_1();
            for (i, word) in vocab.iter().enumerate() {
                let shared = self.shared.clone();
                chips = chips.child(
                    div()
                        .id(SharedString::from(format!("vocab-{i}")))
                        .flex()
                        .items_center()
                        .gap_1()
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(cx.theme().secondary)
                        .border_1()
                        .border_color(cx.theme().border)
                        .text_xs()
                        .child(div().text_color(cx.theme().foreground).child(word.clone()))
                        .child(
                            Button::new(SharedString::from(format!("rm-vocab-{i}")))
                                .label("×")
                                .ghost()
                                .xsmall()
                                .compact()
                                .on_click(cx.listener(move |_this, _, _window, cx| {
                                    let _ = shared.update_config(|config| {
                                        if i < config.dictionary.vocabulary.len() {
                                            config.dictionary.vocabulary.remove(i);
                                        }
                                    });
                                    cx.notify();
                                })),
                        ),
                );
            }
            vocab_card = vocab_card.child(chips);
        }

        let shared_add_vocab = self.shared.clone();
        let vocab_input_entity = self.vocabulary_input.clone();
        vocab_card = vocab_card.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().child(Input::new(&self.vocabulary_input)))
                .child(
                    Button::new("add-vocab")
                        .label("Add")
                        .ghost()
                        .small()
                        .compact()
                        .on_click(cx.listener(move |_this, _, window, cx| {
                            let word = vocab_input_entity
                                .read(cx)
                                .value()
                                .to_string()
                                .trim()
                                .to_string();
                            if !word.is_empty() {
                                let _ = shared_add_vocab.update_config(|config| {
                                    config.dictionary.vocabulary.push(word);
                                });
                                vocab_input_entity.update(cx, |state, cx| {
                                    use gpui::EntityInputHandler;
                                    state.replace_text_in_range(
                                        Some(std::ops::Range {
                                            start: 0,
                                            end: state.value().len(),
                                        }),
                                        "",
                                        window,
                                        cx,
                                    );
                                });
                                cx.notify();
                            }
                        })),
                ),
        );

        container = container.child(section_block("Vocabulary", cx).child(vocab_card));

        // -- Replacements Section --
        let mut repl_card = settings_card(cx)
            .child(hint_row(
                "Auto-replace rules applied after transcription.",
                cx,
            ))
            .gap_2();

        let replacements = &snapshot.config.dictionary.replacements;
        for (i, rule) in replacements.iter().enumerate() {
            let shared = self.shared.clone();
            let cs_label = if rule.case_sensitive { " (Aa)" } else { "" };
            repl_card = repl_card.child(
                div()
                    .id(SharedString::from(format!("repl-{i}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .py_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .child(
                                div()
                                    .text_color(cx.theme().foreground)
                                    .child(rule.find.clone()),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_xs()
                                    .child("→"),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().foreground)
                                    .child(rule.replace.clone()),
                            )
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .text_xs()
                                    .child(cs_label.to_string()),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("rm-repl-{i}")))
                            .label("×")
                            .ghost()
                            .small()
                            .compact()
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                let _ = shared.update_config(|config| {
                                    if i < config.dictionary.replacements.len() {
                                        config.dictionary.replacements.remove(i);
                                    }
                                });
                                cx.notify();
                            })),
                    ),
            );
        }

        let shared_add_repl = self.shared.clone();
        let find_entity = self.replacement_find_input.clone();
        let replace_entity = self.replacement_replace_input.clone();
        repl_card = repl_card.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.replacement_find_input)),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .text_xs()
                        .child("→"),
                )
                .child(
                    div()
                        .flex_1()
                        .child(Input::new(&self.replacement_replace_input)),
                )
                .child(
                    Button::new("add-repl")
                        .label("Add")
                        .ghost()
                        .small()
                        .compact()
                        .on_click(cx.listener(move |_this, _, window, cx| {
                            let find = find_entity.read(cx).value().to_string().trim().to_string();
                            let replace = replace_entity
                                .read(cx)
                                .value()
                                .to_string()
                                .trim()
                                .to_string();
                            if !find.is_empty() {
                                let _ = shared_add_repl.update_config(|config| {
                                    config.dictionary.replacements.push(
                                        crate::config::ReplacementRule {
                                            find,
                                            replace,
                                            case_sensitive: false,
                                        },
                                    );
                                });
                                find_entity.update(cx, |state, cx| {
                                    use gpui::EntityInputHandler;
                                    state.replace_text_in_range(
                                        Some(std::ops::Range {
                                            start: 0,
                                            end: state.value().len(),
                                        }),
                                        "",
                                        window,
                                        cx,
                                    );
                                });
                                replace_entity.update(cx, |state, cx| {
                                    use gpui::EntityInputHandler;
                                    state.replace_text_in_range(
                                        Some(std::ops::Range {
                                            start: 0,
                                            end: state.value().len(),
                                        }),
                                        "",
                                        window,
                                        cx,
                                    );
                                });
                                cx.notify();
                            }
                        })),
                ),
        );

        container = container.child(section_block("Replacements", cx).child(repl_card));

        container
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_models::LocalModelInstallState;

    #[test]
    fn parakeet_row_x_action_cancels_download_or_deletes_installed_model() {
        assert_eq!(
            parakeet_row_action_kind(&LocalModelInstallState::Downloading {
                downloaded_bytes: 512,
                total_bytes: Some(1024),
            }),
            ParakeetRowActionKind::Cancel
        );
        assert_eq!(
            parakeet_row_action_kind(&LocalModelInstallState::Installed { size_bytes: 4096 }),
            ParakeetRowActionKind::Delete
        );
    }

    #[test]
    fn parakeet_row_has_no_x_action_while_cancelling() {
        assert_eq!(
            parakeet_row_action_kind(&LocalModelInstallState::Cancelling {
                downloaded_bytes: 512,
                total_bytes: Some(1024),
            }),
            ParakeetRowActionKind::None
        );
    }
}

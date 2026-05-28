use gpui::prelude::*;
use gpui::{SharedString, div, img};
use gpui_component::ActiveTheme;
use gpui_component::Sizable;
use gpui_component::button::Button;
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::{Icon, IconName};

use crate::config::Provider;
use crate::state::AppSnapshot;

use super::super::helpers::*;
use super::super::{ProviderInputs, SettingsApp};
use super::local_models::*;

impl SettingsApp {
    pub(in crate::ui) fn render_providers_pane(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        _snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        let mut container = div().flex().flex_col().gap_2();

        let remote_providers: [(Provider, &ProviderInputs); 5] = [
            (Provider::OpenAi, &self.openai_inputs),
            (Provider::Groq, &self.groq_inputs),
            (Provider::Fireworks, &self.fireworks_inputs),
            (Provider::ElevenLabs, &self.elevenlabs_inputs),
            (Provider::Cerebras, &self.cerebras_inputs),
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

            let is_verified = crate::model_catalog::provider_verified(*provider);
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
            .child(self.render_apple_local_provider_card(cx, 5))
            .child(self.render_parakeet_provider_card(cx, 6));

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
                                let label_score = crate::platform::fuzzy_match(&query, label);
                                let locale_score = crate::platform::fuzzy_match(&query, locale);
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
}

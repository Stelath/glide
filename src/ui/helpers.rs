use gpui::prelude::*;
use gpui::{App, Entity, SharedString, div, img};
use gpui_component::ActiveTheme;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::popover::Popover;
use gpui_component::{Icon, IconName};

use crate::config::{GlideConfig, ModelSelection, OverlayPosition, Provider};
use crate::state::SharedState;

pub(super) fn field_label(text: &str, cx: &App) -> gpui::Div {
    div()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .mt_1()
        .child(text.to_string())
}

pub(super) fn settings_card(cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .px_4()
        .py_3()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
}

pub(super) fn section_block(heading: &str, cx: &App) -> gpui::Div {
    div().flex().flex_col().gap_2().child(
        div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(heading.to_string()),
    )
}

pub(super) fn setting_row(label: &str, description: &str, cx: &App) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .flex_1()
                .child(
                    div()
                        .text_sm()
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(cx.theme().foreground)
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(description.to_string()),
                ),
        )
}

pub(super) fn position_button_preview(pos: OverlayPosition) -> gpui::Div {
    let shell = gpui::hsla(0.0, 0.0, 0.07, 0.98);
    let shell_edge = gpui::hsla(0.0, 0.0, 0.16, 1.0);
    let bar = gpui::hsla(0.0, 0.0, 0.92, 0.96);

    match pos {
        OverlayPosition::Notch => {
            let mut bars = div()
                .flex()
                .items_start()
                .justify_center()
                .gap(gpui::px(2.0))
                .pt(gpui::px(4.0));
            for h in [6.0, 10.0, 14.0, 9.0, 5.0] {
                bars = bars.child(div().w(gpui::px(3.0)).h(gpui::px(h)).bg(bar));
            }

            div()
                .w(gpui::px(120.0))
                .h(gpui::px(44.0))
                .flex()
                .items_start()
                .justify_center()
                .pt(gpui::px(2.0))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_center()
                        .gap(gpui::px(0.0))
                        .child(div().w(gpui::px(32.0)).h(gpui::px(4.0)).bg(shell))
                        .child(
                            div()
                                .w(gpui::px(36.0))
                                .h(gpui::px(22.0))
                                .flex()
                                .justify_center()
                                .bg(shell)
                                .child(bars),
                        )
                        .child(div().w(gpui::px(32.0)).h(gpui::px(4.0)).bg(shell)),
                )
        }
        OverlayPosition::Floating => {
            let mut bars = div()
                .flex()
                .items_end()
                .justify_center()
                .gap(gpui::px(2.0))
                .pb(gpui::px(4.0));
            for h in [5.0, 10.0, 14.0, 9.0, 6.0] {
                bars = bars.child(div().w(gpui::px(3.0)).h(gpui::px(h)).bg(bar));
            }

            div()
                .w(gpui::px(120.0))
                .h(gpui::px(44.0))
                .flex()
                .items_start()
                .justify_center()
                .pt(gpui::px(8.0))
                .child(
                    div()
                        .w(gpui::px(48.0))
                        .h(gpui::px(24.0))
                        .flex()
                        .items_end()
                        .justify_center()
                        .rounded(gpui::px(7.0))
                        .border_1()
                        .border_color(shell_edge)
                        .bg(shell)
                        .child(bars),
                )
        }
    }
}

pub(super) fn hint_row(text: &str, cx: &App) -> gpui::Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

fn strip_segment_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = value.strip_prefix(prefix)?.strip_prefix('/')?;
    (!rest.is_empty()).then_some(rest)
}

fn strip_model_display_prefix<'a>(value: &'a str, provider: Option<&str>) -> &'a str {
    let prefixes: &[&str] = match provider {
        Some("Fireworks") => &["accounts/fireworks/models"],
        Some("Groq") => &["openai", "meta-llama"],
        None => &["accounts/fireworks/models", "openai", "meta-llama"],
        _ => &[],
    };

    prefixes
        .iter()
        .find_map(|prefix| strip_segment_prefix(value, prefix))
        .unwrap_or(value)
}

pub(super) fn display_model_name(id: &str) -> &str {
    let id = strip_model_display_prefix(id, None);
    id.rsplit_once('/').map(|(_, name)| name).unwrap_or(id)
}

pub(super) fn model_display_name(model: &crate::model_catalog::ModelInfo) -> String {
    if model.display_name.trim().is_empty() {
        display_model_name(&model.id).to_string()
    } else {
        strip_model_display_prefix(&model.display_name, Some(&model.provider)).to_string()
    }
}

pub(super) fn model_label_for(id: &str, models: &[crate::model_catalog::ModelInfo]) -> String {
    models
        .iter()
        .find(|model| model.id == id)
        .map(model_display_name)
        .unwrap_or_else(|| display_model_name(id).to_string())
}

pub(super) fn model_picker_item(
    model: &crate::model_catalog::ModelInfo,
    recommended: bool,
    cx: &App,
) -> gpui::Stateful<gpui::Div> {
    let logo_path = crate::config::asset_path(&model.logo);
    let mut row = div()
        .id(SharedString::from(format!("pick-{}", model.id)))
        .flex()
        .items_center()
        .gap_2()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
        .child(
            img(logo_path)
                .w(gpui::px(16.0))
                .h(gpui::px(16.0))
                .rounded_sm(),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(model_display_name(model)),
        );
    if recommended {
        row = row.child(
            Icon::new(IconName::Star)
                .size_3()
                .text_color(cx.theme().muted_foreground),
        );
    }
    row
}

fn filtered_models<'a>(
    models: &'a [crate::model_catalog::ModelInfo],
    query: &str,
) -> Vec<&'a crate::model_catalog::ModelInfo> {
    if query.is_empty() {
        return models.iter().collect();
    }

    let mut scored: Vec<_> = models
        .iter()
        .filter_map(|model| {
            let display = model_display_name(model);
            let id_score = crate::platform::fuzzy_match(query, &model.id);
            let display_score = crate::platform::fuzzy_match(query, &display);
            id_score.or(display_score).map(|score| (model, score))
        })
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(model, _)| model).collect()
}

fn model_picker_trigger(id: &str, label: impl Into<String>) -> Button {
    Button::new(SharedString::from(format!("trigger-{id}")))
        .ghost()
        .small()
        .compact()
        .child(div().truncate().max_w(gpui::px(180.0)).child(label.into()))
}

fn model_picker_panel(
    scroll_id: &'static str,
    search_entity: &Entity<InputState>,
    list: gpui::Div,
    cx: &App,
) -> gpui::Div {
    div()
        .w(gpui::px(280.0))
        .max_h(gpui::px(300.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .child(
            div()
                .p_2()
                .border_b_1()
                .border_color(cx.theme().border)
                .child(Input::new(search_entity)),
        )
        .child(
            div()
                .id(SharedString::from(scroll_id))
                .flex_1()
                .overflow_y_scroll()
                .child(list),
        )
}

fn model_dropdown_wrapper(current_logo: Option<String>, popover: impl IntoElement) -> gpui::Div {
    let mut wrapper = div()
        .flex()
        .items_center()
        .gap_1()
        .min_w_0()
        .overflow_hidden();
    if let Some(logo) = current_logo {
        wrapper = wrapper.child(
            img(crate::config::asset_path(&logo))
                .w(gpui::px(16.0))
                .h(gpui::px(16.0))
                .rounded_sm()
                .flex_shrink_0(),
        );
    }
    wrapper.child(div().min_w_0().overflow_hidden().child(popover))
}

#[derive(Clone, Copy)]
pub(super) struct ModelDropdownAction {
    pub id: &'static str,
    pub label: &'static str,
    pub updater: fn(&mut GlideConfig),
}

pub(super) fn disable_llm_rewrite(config: &mut GlideConfig) {
    config.dictation.llm = None;
    config.dictation.smart_defaults_applied = true;
}

#[allow(clippy::too_many_arguments)]
pub(super) fn model_dropdown_button(
    id: &str,
    current_model: &str,
    models: &[crate::model_catalog::ModelInfo],
    recommended_model: Option<&str>,
    action: Option<ModelDropdownAction>,
    shared: SharedState,
    search_entity: Entity<InputState>,
    updater: fn(&mut GlideConfig, String, Provider),
) -> gpui::Div {
    let current_logo = models
        .iter()
        .find(|m| m.id == current_model)
        .map(|m| m.logo.clone());
    let current_display = model_label_for(current_model, models);
    let models = models.to_vec();
    let recommended = recommended_model.map(|s| s.to_string());

    let popover_id = SharedString::from(format!("model-picker-{id}"));
    let trigger = model_picker_trigger(id, current_display);

    let popover = Popover::new(popover_id)
        .trigger(trigger)
        .content(move |_state, _window, cx| {
            let popover = cx.entity().clone();
            let query = search_entity.read(cx).value().to_string();
            let filtered = filtered_models(&models, &query);

            let mut list = div().flex().flex_col().gap_0p5().p_1();
            if let Some(action) = action {
                let shared = shared.clone();
                let popover = popover.clone();
                list = list.child(
                    div()
                        .id(SharedString::from(format!("pick-{}", action.id)))
                        .flex()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(action.label)
                        .on_click(move |_, window, cx| {
                            let _ = shared.update_config(|config| {
                                (action.updater)(config);
                            });
                            popover.update(cx, |state, cx| {
                                state.dismiss(window, cx);
                            });
                        }),
                );
            }
            for model in &filtered {
                let is_recommended = recommended.as_deref() == Some(&model.id);
                let model_id = model.id.clone();
                let model_provider_str = model.provider.clone();
                let shared = shared.clone();
                let popover = popover.clone();
                list = list.child(model_picker_item(model, is_recommended, cx).on_click(
                    move |_, window, cx| {
                        let id = model_id.clone();
                        let provider =
                            crate::config::Provider::from_model_info_provider(&model_provider_str)
                                .unwrap_or(crate::config::Provider::OpenAi);
                        let _ = shared.update_config(|config| {
                            updater(config, id, provider);
                        });
                        popover.update(cx, |state, cx| {
                            state.dismiss(window, cx);
                        });
                    },
                ));
            }

            model_picker_panel("model-scroll", &search_entity, list, cx)
        });

    model_dropdown_wrapper(current_logo, popover)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(provider: &str, id: &str, display_name: &str) -> crate::model_catalog::ModelInfo {
        crate::model_catalog::ModelInfo {
            id: id.to_string(),
            display_name: display_name.to_string(),
            provider: provider.to_string(),
            logo: String::new(),
            installed: false,
        }
    }

    #[test]
    fn model_display_name_strips_only_known_provider_prefixes() {
        let cases = [
            (
                "Fireworks",
                "accounts/fireworks/models/gpt-oss-20b",
                "accounts/fireworks/models/gpt-oss-20b",
                "gpt-oss-20b",
            ),
            (
                "Groq",
                "openai/gpt-oss-120b",
                "openai/gpt-oss-120b",
                "gpt-oss-120b",
            ),
            (
                "Groq",
                "meta-llama/llama-4-scout-17b-16e-instruct",
                "meta-llama/llama-4-scout-17b-16e-instruct",
                "llama-4-scout-17b-16e-instruct",
            ),
            ("Other", "vendor/model", "vendor/model", "vendor/model"),
        ];

        for (provider, id, display_name, expected) in cases {
            assert_eq!(
                model_display_name(&model(provider, id, display_name)),
                expected
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn style_model_dropdown(
    id: &str,
    current_display: &str,
    current_model_id: Option<&str>,
    models: &[crate::model_catalog::ModelInfo],
    shared: SharedState,
    search_entity: Entity<InputState>,
    style_index: usize,
    is_stt: bool,
) -> gpui::Div {
    let raw_model = if current_display.starts_with("Default (") {
        current_display
            .strip_prefix("Default (")
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(current_display)
    } else {
        current_display
    };
    let current_logo = current_model_id
        .and_then(|current_model_id| models.iter().find(|m| m.id == current_model_id))
        .or_else(|| {
            models
                .iter()
                .find(|m| m.id == raw_model || model_display_name(m) == raw_model)
        })
        .map(|m| m.logo.clone());
    let models = models.to_vec();

    let popover_id = SharedString::from(format!("model-picker-{id}"));
    let trigger = model_picker_trigger(id, current_display);

    let popover = Popover::new(popover_id)
        .trigger(trigger)
        .content(move |_state, _window, cx| {
            let popover = cx.entity().clone();
            let query = search_entity.read(cx).value().to_string();
            let filtered = filtered_models(&models, &query);

            let shared_default = shared.clone();
            let popover_default = popover.clone();
            let default_item = div()
                .id("pick-default")
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .cursor_pointer()
                .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Default (use global)")
                .on_click(move |_, window, cx| {
                    let _ = shared_default.update_config(|config| {
                        if let Some(style) = config.dictation.styles.get_mut(style_index) {
                            if is_stt {
                                style.stt = None;
                            } else {
                                style.llm = None;
                            }
                        }
                    });
                    popover_default.update(cx, |state, cx| {
                        state.dismiss(window, cx);
                    });
                });

            let mut list = div().flex().flex_col().gap_0p5().p_1().child(default_item);
            for model in &filtered {
                let model_id = model.id.clone();
                let model_provider_str_style = model.provider.clone();
                let shared = shared.clone();
                let popover = popover.clone();
                list = list.child(model_picker_item(model, false, cx).on_click(
                    move |_, window, cx| {
                        let id = model_id.clone();
                        let model_provider = model_provider_str_style.clone();
                        let _ = shared.update_config(|config| {
                            let provider =
                                crate::config::Provider::from_model_info_provider(&model_provider)
                                    .unwrap_or(crate::config::Provider::OpenAi);
                            if let Some(style) = config.dictation.styles.get_mut(style_index) {
                                if is_stt {
                                    style.stt = Some(ModelSelection {
                                        provider,
                                        model: id,
                                    });
                                } else {
                                    style.llm = Some(ModelSelection {
                                        provider,
                                        model: id,
                                    });
                                }
                            }
                        });
                        popover.update(cx, |state, cx| {
                            state.dismiss(window, cx);
                        });
                    },
                ));
            }

            model_picker_panel("style-model-scroll", &search_entity, list, cx)
        });

    model_dropdown_wrapper(current_logo, popover)
}

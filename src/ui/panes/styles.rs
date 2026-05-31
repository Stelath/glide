use gpui::prelude::*;
use gpui::{SharedString, div, img};
use gpui_component::ActiveTheme;
use gpui_component::IconName;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::popover::Popover;
use gpui_component::scroll::ScrollableElement;

use crate::config::{ModelSelection, Style};

use super::super::SettingsApp;
use super::super::helpers::*;

impl SettingsApp {
    pub(in crate::ui) fn render_styles_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let available_apps = crate::platform::list_applications();
        let mut container = div().flex().flex_col().gap_4();

        // --- Default prompt & models ---
        let snapshot = self.shared.snapshot();
        let default_stt = snapshot.config.dictation.stt.model.clone();
        let default_llm_selection = snapshot.config.dictation.llm.as_ref();
        let default_llm = default_llm_selection
            .map(|s| s.model.clone())
            .unwrap_or_else(|| "Disabled".to_string());
        let stt_models = crate::engines::model_catalog::cached_stt_models();
        let llm_models = crate::engines::model_catalog::cached_llm_models();

        let shared_stt = self.shared.clone();
        let shared_llm = self.shared.clone();
        let has_provider = crate::engines::model_catalog::any_provider_verified();

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
            .child(field_label("Prompt Template", cx))
            .on_click(cx.listener(|this, _, _window, cx| {
                this.prompt_expanded = !this.prompt_expanded;
                cx.notify();
            }));

        container = container.child(
            section_block("Defaults", cx).child(
                settings_card(cx)
                    .child(hint_row("Used for all applications.", cx))
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
                        let recommended_stt =
                            crate::engines::model_catalog::smart_stt_default().map(|s| s.model);
                        let recommended_llm =
                            crate::engines::model_catalog::smart_llm_default().map(|s| s.model);
                        this.child(setting_row("Voice Model", "Speech-to-text", cx).child(
                            model_dropdown_button(
                                "default-stt",
                                &default_stt,
                                &stt_models,
                                recommended_stt.as_deref(),
                                None,
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
                                    Some(ModelDropdownAction {
                                        id: "disabled",
                                        label: "Disabled",
                                        updater: disable_llm_rewrite,
                                    }),
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

        // --- Styles ---
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
                    let icon_el = if let Some(icon_path) = crate::platform::app_icon_path(app) {
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
                                    crate::platform::fuzzy_match(&query, a).map(|s| (a, s))
                                })
                                .collect();
                            scored.sort_by(|a, b| b.1.cmp(&a.1));
                            let mut grid = div().flex().flex_wrap().gap_2().p_2();
                            for (app, _) in &scored {
                                let app_name = (*app).clone();
                                let entity = entity_for_apps.clone();
                                let tile_icon = if let Some(p) = crate::platform::app_icon_path(app)
                                {
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
                    .child(field_label("Style Prompt", cx))
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        if let Some(style) = this.styles.get_mut(index) {
                            style.prompt_expanded = !style.prompt_expanded;
                        }
                        cx.notify();
                    }));

                let cfg_styles = &snapshot.config.dictation.styles;
                let stt_selection = cfg_styles.get(index).and_then(|s| s.stt.as_ref());
                let stt_model_id = stt_selection
                    .map(|sel| sel.model.as_str())
                    .unwrap_or(default_stt.as_str());
                let stt_display = stt_selection
                    .map(|sel| model_label_for(&sel.model, &stt_models))
                    .unwrap_or_else(|| {
                        format!("Default ({})", model_label_for(&default_stt, &stt_models))
                    });
                let llm_selection = cfg_styles.get(index).and_then(|s| s.llm.as_ref());
                let llm_model_id = llm_selection
                    .map(|sel| sel.model.as_str())
                    .or_else(|| default_llm_selection.map(|sel| sel.model.as_str()));
                let llm_display = llm_selection
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
                                    Some(stt_model_id),
                                    &stt_models,
                                    self.shared.clone(),
                                    style.stt_model_search.clone(),
                                    index,
                                    true,
                                ))
                                .child(style_model_dropdown(
                                    &format!("style-llm-{index}"),
                                    &llm_display,
                                    llm_model_id,
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

        // --- Add style button ---
        container = container.child(
            div().child(
                Button::new("add-style")
                    .label("+ Add Style")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let style_num = this.styles.len() + 1;
                        let entry = Style {
                            name: format!("Style {style_num}"),
                            apps: Vec::new(),
                            prompt: String::new(),
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
}

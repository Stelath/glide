use std::time::Duration;

use gpui::prelude::*;
use gpui::{SharedString, div, img};
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

impl SettingsApp {
    pub(super) fn render_providers_pane(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        _snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        let mut container = div().flex().flex_col().gap_2();

        let providers: [(Provider, &ProviderInputs); 2] = [
            (Provider::OpenAi, &self.openai_inputs),
            (Provider::Groq, &self.groq_inputs),
        ];

        for (idx, (provider, inputs)) in providers.iter().enumerate() {
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

        container
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

        container = container.child(
            section_block("Defaults", cx).child(
                settings_card(cx)
                    .child(hint_row(
                        "Used for all applications unless a style overrides it.",
                        cx,
                    ))
                    .child(field_label("System Prompt", cx))
                    .child(Input::new(&self.default_prompt))
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
            let style_name = style.name.read(cx).value().to_string();
            let display_name = if style_name.is_empty() {
                format!("Style #{}", index + 1)
            } else {
                style_name.clone()
            };
            let is_expanded = self.expanded_style == Some(index);

            let chevron = if is_expanded { "▼" } else { "▶" };
            let header = div()
                .id(SharedString::from(format!("style-header-{index}")))
                .flex()
                .items_center()
                .justify_between()
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
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(chevron),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(cx.theme().foreground)
                                .child(display_name),
                        ),
                )
                .child(
                    Button::new(SharedString::from(format!("remove-style-{index}")))
                        .label("×")
                        .ghost()
                        .small()
                        .compact()
                        .on_click(cx.listener(move |this, _, _window, cx| {
                            this.styles.remove(index);
                            if this.expanded_style == Some(index) {
                                this.expanded_style = None;
                            }
                            this.schedule_autosave(cx);
                            cx.notify();
                        })),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.expanded_style = if this.expanded_style == Some(index) {
                        None
                    } else {
                        Some(index)
                    };
                    cx.notify();
                }));

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

                let name_input = Input::new(&style.name).appearance(false);

                let cfg_styles = &snapshot.config.dictation.styles;
                let stt_display = cfg_styles
                    .get(index)
                    .and_then(|s| s.stt.as_ref())
                    .map(|sel| display_model_name(&sel.model).to_string())
                    .unwrap_or_else(|| format!("Default ({})", display_model_name(&default_stt)));
                let llm_display = cfg_styles
                    .get(index)
                    .and_then(|s| s.llm.as_ref())
                    .map(|sel| display_model_name(&sel.model).to_string())
                    .unwrap_or_else(|| format!("Default ({})", display_model_name(&default_llm)));

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
                        .child(name_input)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(app_row)
                                .child(add_app_popover),
                        )
                        .child(Input::new(&style.prompt))
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
        let has_notch = crate::config::notch_width().is_some();
        for style in OverlayStyle::ALL {
            if style == OverlayStyle::Glow && !has_notch {
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
}

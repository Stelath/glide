use std::time::Duration;

use gpui::prelude::*;
use gpui::{div, App, Entity, SharedString, Subscription, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem, SidebarToggleButton};
use gpui_component::theme::{Theme, ThemeMode};
use gpui_component::{Icon, IconName};
use gpui_component::Sizable;
use gpui_component::ActiveTheme;
use gpui_component::Side;

use crate::app::{AppSnapshot, SharedState};
use crate::config::{AppPromptOverride, GlideConfig, HotkeyTrigger, LlmProviderKind, OverlayStyle, ThemePreference};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);

/// Apply the user's theme preference. Called from main.rs at startup and on
/// system appearance changes.
pub fn apply_theme_preference(
    pref: ThemePreference,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    match pref {
        ThemePreference::System => Theme::sync_system_appearance(window, cx),
        ThemePreference::Light => Theme::change(ThemeMode::Light, window, cx),
        ThemePreference::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsPane {
    Models,
    AppPrompts,
    General,
}

impl SettingsPane {
    fn label(self) -> &'static str {
        match self {
            Self::Models => "Models",
            Self::AppPrompts => "App Prompts",
            Self::General => "General",
        }
    }
}

struct AppOverrideInputs {
    app_name: Entity<InputState>,
    prompt: Entity<InputState>,
}

pub struct SettingsApp {
    shared: SharedState,
    active_pane: SettingsPane,
    sidebar_collapsed: bool,
    recording_hotkey: bool,

    stt_api_key: Entity<InputState>,
    stt_env_fallback: Entity<InputState>,
    stt_model: Entity<InputState>,
    stt_endpoint: Entity<InputState>,

    llm_api_key: Entity<InputState>,
    llm_env_fallback: Entity<InputState>,
    llm_model: Entity<InputState>,
    llm_endpoint: Entity<InputState>,

    default_prompt: Entity<InputState>,
    app_overrides: Vec<AppOverrideInputs>,

    save_pending: bool,
    _subscriptions: Vec<Subscription>,
}

impl SettingsApp {
    pub fn new(
        shared: SharedState,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> Self {
        let config = shared.snapshot().config;

        let stt_api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(&config.stt.openai.api_key)
        });
        let stt_env_fallback = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.stt.openai.api_key_env)
        });
        let stt_model = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.stt.openai.model)
        });
        let stt_endpoint = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.stt.openai.endpoint)
        });

        let llm_api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(&config.llm.openai.api_key)
        });
        let llm_env_fallback = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.llm.openai.api_key_env)
        });
        let llm_model = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.llm.openai.model)
        });
        let llm_endpoint = cx.new(|cx| {
            InputState::new(window, cx).default_value(&config.llm.openai.endpoint)
        });

        let default_prompt = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 12)
                .default_value(&config.llm.prompt.system)
        });

        let mut subs = vec![
            cx.subscribe_in(&stt_api_key, window, Self::on_input_change),
            cx.subscribe_in(&stt_env_fallback, window, Self::on_input_change),
            cx.subscribe_in(&stt_model, window, Self::on_input_change),
            cx.subscribe_in(&stt_endpoint, window, Self::on_input_change),
            cx.subscribe_in(&llm_api_key, window, Self::on_input_change),
            cx.subscribe_in(&llm_env_fallback, window, Self::on_input_change),
            cx.subscribe_in(&llm_model, window, Self::on_input_change),
            cx.subscribe_in(&llm_endpoint, window, Self::on_input_change),
            cx.subscribe_in(&default_prompt, window, Self::on_input_change),
        ];

        let app_overrides: Vec<_> = config
            .llm
            .prompt
            .app_overrides
            .iter()
            .map(|entry| {
                let (inputs, entry_subs) =
                    Self::create_override_inputs(entry, window, cx);
                subs.extend(entry_subs);
                inputs
            })
            .collect();

        // Follow system appearance changes at runtime
        let theme_shared = shared.clone();
        subs.push(cx.observe_window_appearance(window, move |_this, window, cx| {
            let pref = theme_shared.snapshot().config.app.theme;
            apply_theme_preference(pref, Some(window), cx);
        }));

        Self {
            shared,
            active_pane: SettingsPane::Models,
            sidebar_collapsed: false,
            recording_hotkey: false,
            stt_api_key,
            stt_env_fallback,
            stt_model,
            stt_endpoint,
            llm_api_key,
            llm_env_fallback,
            llm_model,
            llm_endpoint,
            default_prompt,
            app_overrides,
            save_pending: false,
            _subscriptions: subs,
        }
    }

    fn create_override_inputs(
        entry: &AppPromptOverride,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> (AppOverrideInputs, Vec<Subscription>) {
        let app_name = cx.new(|cx| {
            InputState::new(window, cx).default_value(&entry.app_name)
        });
        let prompt = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 12)
                .default_value(&entry.prompt)
        });
        let subs = vec![
            cx.subscribe_in(&app_name, window, Self::on_input_change),
            cx.subscribe_in(&prompt, window, Self::on_input_change),
        ];
        (AppOverrideInputs { app_name, prompt }, subs)
    }

    fn on_input_change(
        &mut self,
        _emitter: &Entity<InputState>,
        event: &InputEvent,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        if matches!(event, InputEvent::Change) {
            self.schedule_autosave(cx);
        }
    }

    fn schedule_autosave(&mut self, cx: &mut gpui::Context<Self>) {
        if !self.save_pending {
            self.save_pending = true;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(AUTOSAVE_DELAY).await;
                this.update(cx, |this, cx| {
                    this.save_pending = false;
                    this.save(cx);
                })
                .ok();
            })
            .detach();
        }
    }

    fn save(&mut self, cx: &gpui::Context<Self>) {
        let draft = self.draft_from_inputs(cx);
        let _ = self.shared.update_config(move |config| *config = draft);
    }

    fn draft_from_inputs(&self, cx: &gpui::Context<Self>) -> GlideConfig {
        let mut config = self.shared.snapshot().config;

        config.stt.openai.api_key = self.stt_api_key.read(cx).value().to_string();
        config.stt.openai.api_key_env = self.stt_env_fallback.read(cx).value().to_string();
        config.stt.openai.model = self.stt_model.read(cx).value().to_string();
        config.stt.openai.endpoint = self.stt_endpoint.read(cx).value().to_string();

        config.llm.openai.api_key = self.llm_api_key.read(cx).value().to_string();
        config.llm.openai.api_key_env = self.llm_env_fallback.read(cx).value().to_string();
        config.llm.openai.model = self.llm_model.read(cx).value().to_string();
        config.llm.openai.endpoint = self.llm_endpoint.read(cx).value().to_string();

        config.llm.prompt.system = self.default_prompt.read(cx).value().to_string();

        config.llm.prompt.app_overrides = self
            .app_overrides
            .iter()
            .map(|ov| AppPromptOverride {
                app_name: ov.app_name.read(cx).value().to_string(),
                prompt: ov.prompt.read(cx).value().to_string(),
            })
            .collect();

        config
    }

    fn render_sidebar(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;

        Sidebar::new(Side::Left)
            .collapsed(collapsed)
            .child(
                SidebarMenu::new()
                    .child(
                        SidebarMenuItem::new("Models")
                            .icon(Icon::new(IconName::Settings2))
                            .active(self.active_pane == SettingsPane::Models)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::Models;
                                cx.notify();
                            })),
                    )
                    .child(
                        SidebarMenuItem::new("App Prompts")
                            .icon(Icon::new(IconName::File))
                            .active(self.active_pane == SettingsPane::AppPrompts)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::AppPrompts;
                                cx.notify();
                            })),
                    )
                    .child(
                        SidebarMenuItem::new("General")
                            .icon(Icon::new(IconName::Settings))
                            .active(self.active_pane == SettingsPane::General)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::General;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_content(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let snapshot = self.shared.snapshot();

        div()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .p_6()
            .id("content-scroll")
            .overflow_y_scroll()
            .bg(cx.theme().background)
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .mb_4()
                    .child(self.active_pane.label()),
            )
            .child(match self.active_pane {
                SettingsPane::Models => {
                    self.render_models_pane(window, cx, &snapshot).into_any_element()
                }
                SettingsPane::AppPrompts => {
                    self.render_app_prompts_pane(window, cx).into_any_element()
                }
                SettingsPane::General => {
                    self.render_general_pane(window, cx, &snapshot).into_any_element()
                }
            })
    }

    fn render_models_pane(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                card(cx)
                    .child(section_heading("Speech To Text", cx))
                    .child(read_only_row(
                        "Provider",
                        snapshot.config.stt.provider.label(),
                        cx,
                    ))
                    .child(field_label("API Key", cx))
                    .child(Input::new(&self.stt_api_key).mask_toggle().cleanable(true))
                    .child(field_label("Env Fallback", cx))
                    .child(Input::new(&self.stt_env_fallback))
                    .child(field_label("Model", cx))
                    .child(Input::new(&self.stt_model))
                    .child(field_label("Endpoint", cx))
                    .child(Input::new(&self.stt_endpoint))
                    .child(hint_row(
                        &format!(
                            "Credential source: {}",
                            snapshot.config.stt.openai.credential_source()
                        ),
                        cx,
                    )),
            )
            .child(
                card(cx)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(section_heading("Text Cleanup (LLM)", cx))
                            .child({
                                let label = match snapshot.config.llm.provider {
                                    LlmProviderKind::None => "Disabled",
                                    LlmProviderKind::OpenAi => "OpenAI GPT",
                                };
                                Button::new("toggle-llm")
                                    .label(label)
                                    .primary()
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        let current =
                                            this.shared.snapshot().config.llm.provider;
                                        let _ = this.shared.update_config(|config| {
                                            config.llm.provider = current.next();
                                        });
                                        cx.notify();
                                    }))
                            }),
                    )
                    .child(field_label("API Key", cx))
                    .child(Input::new(&self.llm_api_key).mask_toggle().cleanable(true))
                    .child(field_label("Env Fallback", cx))
                    .child(Input::new(&self.llm_env_fallback))
                    .child(field_label("Model", cx))
                    .child(Input::new(&self.llm_model))
                    .child(field_label("Endpoint", cx))
                    .child(Input::new(&self.llm_endpoint))
                    .child(hint_row(
                        &format!(
                            "Credential source: {}",
                            snapshot.config.llm.openai.credential_source()
                        ),
                        cx,
                    )),
            )
    }

    fn render_app_prompts_pane(
        &self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let mut container = div().flex().flex_col().gap_3();

        container = container.child(
            card(cx)
                .child(section_heading("Default Prompt", cx))
                .child(hint_row(
                    "Used for all applications unless overridden below.",
                    cx,
                ))
                .child(field_label("System Prompt", cx))
                .child(Input::new(&self.default_prompt)),
        );

        let mut overrides_card = card(cx)
            .child(section_heading("Per-App Overrides", cx))
            .child(hint_row(
                "Custom prompts for specific apps. The override replaces the default when dictating in a matched app.",
                cx,
            ));

        for (index, ov) in self.app_overrides.iter().enumerate() {
            overrides_card = overrides_card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_2()
                    .pb_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(cx.theme().primary)
                                    .child(format!("#{}", index + 1)),
                            )
                            .child(
                                Button::new(SharedString::from(format!("remove-{index}")))
                                    .label("Remove")
                                    .danger()
                                    .small()
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        this.app_overrides.remove(index);
                                        this.schedule_autosave(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(field_label("Application Name", cx))
                    .child(Input::new(&ov.app_name))
                    .child(field_label("System Prompt", cx))
                    .child(Input::new(&ov.prompt)),
            );
        }

        overrides_card = overrides_card.child(
            div().mt_3().child(
                Button::new("add-override")
                    .label("+ Add App Override")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let default_prompt =
                            this.default_prompt.read(cx).value().to_string();
                        let entry = AppPromptOverride {
                            app_name: String::new(),
                            prompt: default_prompt,
                        };
                        let (inputs, subs) =
                            Self::create_override_inputs(&entry, window, cx);
                        this.app_overrides.push(inputs);
                        this._subscriptions.extend(subs);
                        this.schedule_autosave(cx);
                        cx.notify();
                    })),
            ),
        );

        container = container.child(overrides_card);
        container
    }

    fn render_general_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
        snapshot: &AppSnapshot,
    ) -> impl IntoElement {
        let current_trigger = snapshot.config.hotkey.trigger;
        let current_theme = snapshot.config.app.theme;
        let current_overlay = snapshot.config.overlay.style;

        let mut container = div().flex().flex_col().gap_4();

        // -- Recording Window -------------------------------------------
        let mut style_cards = div().flex().gap_3().flex_1();
        for style in OverlayStyle::ALL {
            let is_active = style == current_overlay;
            let icon_text = match style {
                OverlayStyle::Classic => "▁▂▃▅▃▂▁",
                OverlayStyle::Mini => "▪▪▪",
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

        container = container.child(
            section_block("Recording Window", cx)
                .child(
                    settings_card(cx)
                        .child(
                            setting_row("Style", "Overlay shown while recording", cx)
                        )
                        .child(style_cards),
                ),
        );

        // -- Keyboard Shortcut ------------------------------------------
        let hotkey_label = current_trigger.label();
        let is_recording = self.recording_hotkey;

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

        container = container.child(
            section_block("Keyboard Shortcuts", cx)
                .child(
                    settings_card(cx)
                        .child(
                            setting_row(
                                "Dictation Hotkey",
                                "Hold to record, release to transcribe",
                                cx,
                            )
                            .child(
                                div()
                                    .id("hotkey-recorder")
                                    .flex_shrink_0()
                                    .child(
                                        shortcut_button
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.recording_hotkey = true;
                                                cx.notify();
                                            })),
                                    )
                                    .when(is_recording, |this| {
                                        this.on_key_down(cx.listener(
                                            |this, event: &gpui::KeyDownEvent, window, cx| {
                                                let key = event.keystroke.key.as_str();
                                                let trigger = HotkeyTrigger::from_key_name(key);
                                                let _ = this.shared.update_config(|config| {
                                                    config.hotkey.trigger = trigger;
                                                });
                                                this.recording_hotkey = false;
                                                window.prevent_default();
                                                cx.stop_propagation();
                                                cx.notify();
                                            },
                                        ))
                                    }),
                            ),
                        ),
                ),
        );

        // -- Appearance -------------------------------------------------
        container = container.child(
            section_block("Appearance", cx)
                .child(
                    settings_card(cx)
                        .child(
                            setting_row(
                                "Theme",
                                "Switch between light and dark",
                                cx,
                            )
                            .child({
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
                                        .on_click(cx.listener(
                                            move |this, _, window, cx| {
                                                let _ =
                                                    this.shared.update_config(|config| {
                                                        config.app.theme = pref;
                                                    });
                                                apply_theme_preference(
                                                    pref,
                                                    Some(window),
                                                    cx,
                                                );
                                                cx.notify();
                                            },
                                        )),
                                    );
                                }
                                pills
                            }),
                        ),
                ),
        );

        container
    }
}

impl Render for SettingsApp {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let snapshot = self.shared.snapshot();
        let mic_name = if snapshot.config.audio.device == "default" {
            "Default Microphone".to_string()
        } else {
            snapshot.config.audio.device.clone()
        };
        let devices: Vec<String> = if snapshot.input_devices.is_empty() {
            vec!["default".to_string()]
        } else {
            snapshot.input_devices.clone()
        };

        div()
            .flex()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_sidebar(window, cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    // -- Top bar --
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_3()
                            .py_2()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                SidebarToggleButton::left()
                                    .collapsed(self.sidebar_collapsed)
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.sidebar_collapsed =
                                            !this.sidebar_collapsed;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("top-bar-mic")
                                    .label(SharedString::from(mic_name))
                                    .ghost()
                                    .small()
                                    .compact()
                                    .on_click(cx.listener(move |this, _, _window, cx| {
                                        let current = this
                                            .shared
                                            .snapshot()
                                            .config
                                            .audio
                                            .device
                                            .clone();
                                        let next_index = devices
                                            .iter()
                                            .position(|d| d == &current)
                                            .map(|i| (i + 1) % devices.len())
                                            .unwrap_or(0);
                                        let next = devices[next_index].clone();
                                        let _ = this.shared.update_config(|config| {
                                            config.audio.device = next;
                                        });
                                        this.shared.refresh_input_devices();
                                        cx.notify();
                                    })),
                            ),
                    )
                    // -- Content --
                    .child(self.render_content(window, cx)),
            )
    }
}

// -- Reusable UI helpers ---------------------------------------------------------

fn card(cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
}

fn section_heading(text: &str, cx: &App) -> gpui::Div {
    div()
        .font_weight(gpui::FontWeight::BOLD)
        .text_base()
        .text_color(cx.theme().foreground)
        .mb_2()
        .child(text.to_string())
}

fn field_label(text: &str, cx: &App) -> gpui::Div {
    div()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .mt_1()
        .child(text.to_string())
}

fn read_only_row(label: &str, value: &str, cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .mb_2()
        .child(
            div()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_color(cx.theme().foreground)
                .child(value.to_string()),
        )
}


/// A card container styled like SuperWhisper's settings rows — rounded, subtle
/// border, secondary background. Section headings go *above* these cards.
fn settings_card(cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .px_4()
        .py_1()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().secondary)
}


/// A section with a muted heading and content below (SuperWhisper style).
fn section_block(heading: &str, cx: &App) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(heading.to_string()),
        )
}

fn setting_row(label: &str, description: &str, cx: &App) -> gpui::Div {
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

fn hint_row(text: &str, cx: &App) -> gpui::Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use gpui::{AppContext, TestAppContext, VisualTestContext};

    use crate::app::SharedAppState;
    use crate::config::GlideConfig;

    fn test_shared_state() -> SharedState {
        Arc::new(SharedAppState::new(GlideConfig::default()))
    }

    fn init_and_create_view(
        cx: &mut TestAppContext,
    ) -> (Entity<SettingsApp>, VisualTestContext) {
        cx.update(|app| {
            gpui_component::init(app);
        });

        let shared = test_shared_state();
        let (view, cx) = cx.add_window_view(|window, cx| {
            SettingsApp::new(shared, window, cx)
        });
        let cx = cx.clone();
        (view, cx)
    }

    #[gpui::test]
    async fn test_settings_app_creation(cx: &mut TestAppContext) {
        let (view, cx) = init_and_create_view(cx);
        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::Models);
            assert!(!app.save_pending);
        });
    }

    #[gpui::test]
    async fn test_input_default_values(cx: &mut TestAppContext) {
        let (view, cx) = init_and_create_view(cx);
        let defaults = GlideConfig::default();

        cx.read_entity(&view, |app, cx| {
            assert_eq!(
                app.stt_model.read(cx).value().to_string(),
                defaults.stt.openai.model
            );
            assert_eq!(
                app.stt_endpoint.read(cx).value().to_string(),
                defaults.stt.openai.endpoint
            );
            assert_eq!(
                app.llm_model.read(cx).value().to_string(),
                defaults.llm.openai.model
            );
            assert_eq!(
                app.llm_endpoint.read(cx).value().to_string(),
                defaults.llm.openai.endpoint
            );
        });
    }

    #[gpui::test]
    async fn test_pane_switching(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        cx.update_entity(&view, |app, cx| {
            app.active_pane = SettingsPane::AppPrompts;
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::AppPrompts);
        });

        cx.update_entity(&view, |app, cx| {
            app.active_pane = SettingsPane::General;
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::General);
        });
    }

    #[gpui::test]
    async fn test_draft_from_inputs_matches_config(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let defaults = GlideConfig::default();

        cx.update_entity(&view, |app, cx| {
            let draft = app.draft_from_inputs(cx);
            assert_eq!(draft.stt.openai.model, defaults.stt.openai.model);
            assert_eq!(draft.stt.openai.endpoint, defaults.stt.openai.endpoint);
            assert_eq!(draft.llm.openai.model, defaults.llm.openai.model);
            assert_eq!(draft.llm.openai.endpoint, defaults.llm.openai.endpoint);
            assert_eq!(draft.llm.prompt.system, defaults.llm.prompt.system);
        });
    }

    #[gpui::test]
    async fn test_simulate_typing(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        // Focus the window and simulate typing — this is the SIGTRAP repro
        cx.simulate_input("hello");
        cx.run_until_parked();

        // If we get here without crashing, the SIGTRAP is not in the
        // InputState/subscription layer (it would be in Metal rendering)
        cx.read_entity(&view, |app, _| {
            // Just verify the view is still alive and accessible
            assert_eq!(app.active_pane, SettingsPane::Models);
        });
    }

    #[gpui::test]
    async fn test_autosave_scheduling(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        // Manually trigger schedule_autosave to set save_pending
        cx.update_entity(&view, |app, cx| {
            app.schedule_autosave(cx);
        });

        cx.read_entity(&view, |app, _| {
            assert!(app.save_pending);
        });

        // Advance the clock past the autosave delay and let the timer fire
        cx.executor().advance_clock(AUTOSAVE_DELAY + std::time::Duration::from_millis(100));
        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert!(!app.save_pending);
        });
    }

    #[gpui::test]
    async fn test_add_override(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.app_overrides.len(), 0);
        });

        // Simulate adding an override by directly calling the method
        cx.update(|window, cx| {
            view.update(cx, |app, cx| {
                let entry = AppPromptOverride {
                    app_name: "TestApp".to_string(),
                    prompt: "test prompt".to_string(),
                };
                let (inputs, subs) =
                    SettingsApp::create_override_inputs(&entry, window, cx);
                app.app_overrides.push(inputs);
                app._subscriptions.extend(subs);
                cx.notify();
            });
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.app_overrides.len(), 1);
        });
    }

    #[gpui::test]
    async fn test_remove_override(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        // Add two overrides
        cx.update(|window, cx| {
            view.update(cx, |app, cx| {
                for name in ["App1", "App2"] {
                    let entry = AppPromptOverride {
                        app_name: name.to_string(),
                        prompt: "prompt".to_string(),
                    };
                    let (inputs, subs) =
                        SettingsApp::create_override_inputs(&entry, window, cx);
                    app.app_overrides.push(inputs);
                    app._subscriptions.extend(subs);
                }
                cx.notify();
            });
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.app_overrides.len(), 2);
        });

        // Remove the first one
        cx.update_entity(&view, |app, cx| {
            app.app_overrides.remove(0);
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.app_overrides.len(), 1);
        });
    }

    #[test]
    fn test_settings_pane_labels() {
        assert_eq!(SettingsPane::Models.label(), "Models");
        assert_eq!(SettingsPane::AppPrompts.label(), "App Prompts");
        assert_eq!(SettingsPane::General.label(), "General");
    }

    // ---- Backend / text-input pipeline tests --------------------------------
    // These exercise the code paths that run when a user types into an Input:
    //   InputState::replace_text_in_range → text_wrapper.update → cx.notify()
    //   → Element::prepaint → layout_cursor/layout_selections/layout_match_range
    //   → position_for_index().unwrap()  ← potential SIGTRAP

    /// Directly call replace_text_in_range on an InputState entity
    /// (the same path macOS insert_text callback uses) and then force
    /// a full render cycle to exercise layout_cursor / layout_match_range.
    #[gpui::test]
    async fn test_input_replace_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        // Get the stt_model input entity
        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Directly call replace_text_in_range (same as macOS insert_text)
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "test-text", window, cx);
            });
        });

        // Force a render cycle — this exercises the full prepaint pipeline:
        // layout_cursor → position_for_index, layout_selections → layout_match_range
        cx.run_until_parked();

        // Verify the text was inserted
        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("test-text"));
        });
    }

    /// Test replace_text_in_range on a masked (password) input.
    /// Masked inputs remap char indices which is a distinct code path.
    #[gpui::test]
    async fn test_masked_input_replace_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_api_key.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "secret123", window, cx);
            });
        });

        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("secret123"));
        });
    }

    /// Test replace_text_in_range on a multi-line input (default prompt).
    /// Multi-line inputs use soft-wrap and multi-row layout, a different path.
    #[gpui::test]
    async fn test_multiline_input_replace_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.default_prompt.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "Line one\nLine two\nLine three", window, cx);
            });
        });

        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            let val = state.value().to_string();
            assert!(val.contains("Line one"));
            assert!(val.contains("Line three"));
        });
    }

    /// Simulate multiple rapid keystrokes to test the input handler under
    /// repeated text insertion + render cycles (stress test for SIGTRAP).
    #[gpui::test]
    async fn test_rapid_typing_stress(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Simulate rapid character-by-character insertion (like fast typing)
        for ch in "hello world, this is a longer sentence for stress testing".chars() {
            cx.update(|window, cx| {
                input_entity.update(cx, |state, cx| {
                    use gpui::EntityInputHandler;
                    state.replace_text_in_range(None, &ch.to_string(), window, cx);
                });
            });
        }

        // Force all pending renders
        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("stress testing"));
        });
    }

    /// Test the IME marked text path (set_marked_text → replace_and_mark_text_in_range).
    /// This is the path used by CJK input methods, which is most likely to trigger
    /// the position_for_index unwrap crash.
    #[gpui::test]
    async fn test_ime_marked_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Simulate IME: first mark text (uncommitted), then commit
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                // Mark text (like typing pinyin characters before committing)
                state.replace_and_mark_text_in_range(None, "abc", Some(0..3), window, cx);
            });
        });

        cx.run_until_parked();

        // Now commit the marked text by replacing it
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "committed", window, cx);
            });
        });

        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("committed"));
        });
    }

    /// Test bounds_for_range (called by macOS first_rect_for_character_range
    /// to position the IME candidate window). This is the other callback
    /// that can crash if last_layout is stale.
    #[gpui::test]
    async fn test_bounds_for_range_after_typing(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Insert some text first
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "hello", window, cx);
            });
        });

        cx.run_until_parked();

        // Now call bounds_for_range (what macOS calls for IME positioning)
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                let bounds = gpui::Bounds {
                    origin: gpui::point(gpui::px(0.), gpui::px(0.)),
                    size: gpui::size(gpui::px(400.), gpui::px(30.)),
                };
                // Should not crash — may return None if no layout yet
                let _result = state.bounds_for_range(0..5, bounds, window, cx);
            });
        });
    }

    /// Test that editing an input triggers the subscription callback
    /// (on_input_change → schedule_autosave) via the EntityInputHandler path.
    #[gpui::test]
    async fn test_input_change_subscription_fires(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Verify not pending before
        cx.read_entity(&view, |app, _| {
            assert!(!app.save_pending);
        });

        // Type via replace_text_in_range — this emits InputEvent::Change
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "x", window, cx);
            });
        });

        // The subscription should fire, setting save_pending = true
        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert!(app.save_pending, "subscription should have triggered autosave");
        });

        // Advance past the autosave delay
        cx.executor().advance_clock(AUTOSAVE_DELAY + std::time::Duration::from_millis(100));
        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert!(!app.save_pending, "autosave should have completed");
        });
    }

    /// Test rendering the full SettingsApp view after text input to exercise
    /// the complete Element::prepaint → paint pipeline.
    #[gpui::test]
    async fn test_full_render_after_edit(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let input_entity = cx.read_entity(&view, |app, _| app.stt_model.clone());

        // Edit the input
        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "gpt-4o", window, cx);
            });
        });

        // Trigger a full render of the view
        cx.update_entity(&view, |_, cx| {
            cx.notify();
        });

        // Force the render pipeline to run
        cx.run_until_parked();

        // Verify the app is still healthy
        cx.update_entity(&view, |app, cx| {
            let draft = app.draft_from_inputs(cx);
            assert!(draft.stt.openai.model.contains("gpt-4o"));
        });
    }
}

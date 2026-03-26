use std::time::Duration;

use gpui::prelude::*;
use gpui::{div, img, App, Entity, SharedString, Subscription, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::popover::Popover;
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem, SidebarToggleButton};
use gpui_component::theme::{Theme, ThemeMode};
use gpui_component::{Icon, IconName};
use gpui_component::scroll::ScrollableElement;
use gpui_component::Sizable;
use gpui_component::ActiveTheme;
use gpui_component::Side;

use crate::app::{AppSnapshot, SharedState};
use crate::config::{GlideConfig, HotkeyTrigger, ModelSelection, OverlayPosition, OverlayStyle, Provider, Style, ThemePreference};

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
    Providers,
    Styles,
    General,
}


struct ProviderInputs {
    api_key: Entity<InputState>,
    base_url: Entity<InputState>,
}

struct StyleInputs {
    name: Entity<InputState>,
    apps: Vec<String>,
    prompt: Entity<InputState>,
    search: Entity<InputState>,
    stt_model_search: Entity<InputState>,
    llm_model_search: Entity<InputState>,
}

pub struct SettingsApp {
    shared: SharedState,
    active_pane: SettingsPane,
    sidebar_collapsed: bool,
    recording_hotkey: bool,
    recording_toggle_hotkey: bool,

    openai_inputs: ProviderInputs,
    groq_inputs: ProviderInputs,
    expanded_provider: Option<usize>,
    expanded_style: Option<usize>,

    default_prompt: Entity<InputState>,
    default_stt_search: Entity<InputState>,
    default_llm_search: Entity<InputState>,
    styles: Vec<StyleInputs>,

    last_fetched_openai_key: String,
    last_fetched_groq_key: String,

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

        // Create per-provider input state
        let openai_creds = &config.providers.openai;
        let openai_api_key = cx.new(|cx| {
            InputState::new(window, cx).masked(true).default_value(&openai_creds.api_key)
        });
        let openai_base_url = cx.new(|cx| {
            InputState::new(window, cx).default_value(&openai_creds.base_url)
        });

        let groq_creds = &config.providers.groq;
        let groq_api_key = cx.new(|cx| {
            InputState::new(window, cx).masked(true).default_value(&groq_creds.api_key)
        });
        let groq_base_url = cx.new(|cx| {
            InputState::new(window, cx).default_value(&groq_creds.base_url)
        });

        let default_prompt = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 12)
                .default_value(&config.dictation.system_prompt)
        });
        let default_stt_search = cx.new(|cx| InputState::new(window, cx));
        let default_llm_search = cx.new(|cx| InputState::new(window, cx));

        let mut subs = vec![
            cx.subscribe_in(&openai_api_key, window, Self::on_input_change),
            cx.subscribe_in(&openai_base_url, window, Self::on_input_change),
            cx.subscribe_in(&groq_api_key, window, Self::on_input_change),
            cx.subscribe_in(&groq_base_url, window, Self::on_input_change),
            cx.subscribe_in(&default_prompt, window, Self::on_input_change),
        ];

        let styles: Vec<_> = config
            .dictation
            .styles
            .iter()
            .map(|entry| {
                let (inputs, entry_subs) =
                    Self::create_style_inputs(entry, window, cx);
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

        // After the background model fetch completes, apply smart defaults for first-time setup.
        let shared_for_defaults = shared.clone();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            if crate::config::any_provider_verified() {
                let _ = shared_for_defaults.update_config(|config| {
                    crate::config::apply_smart_defaults_initial(config);
                });
            }
        }).detach();

        Self {
            shared,
            active_pane: SettingsPane::General,
            sidebar_collapsed: false,
            recording_hotkey: false,
            recording_toggle_hotkey: false,
            openai_inputs: ProviderInputs {
                api_key: openai_api_key,
                base_url: openai_base_url,
            },
            groq_inputs: ProviderInputs {
                api_key: groq_api_key,
                base_url: groq_base_url,
            },
            expanded_provider: Some(0),
            expanded_style: Some(0),
            default_prompt,
            default_stt_search,
            default_llm_search,
            styles,
            last_fetched_openai_key: config.providers.openai.api_key.clone(),
            last_fetched_groq_key: config.providers.groq.api_key.clone(),
            save_pending: false,
            _subscriptions: subs,
        }
    }

    fn create_style_inputs(
        entry: &Style,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> (StyleInputs, Vec<Subscription>) {
        let name = cx.new(|cx| {
            InputState::new(window, cx).default_value(&entry.name)
        });
        let prompt = cx.new(|cx| {
            InputState::new(window, cx)
                .auto_grow(3, 12)
                .default_value(&entry.prompt)
        });
        let search = cx.new(|cx| InputState::new(window, cx));
        let stt_model_search = cx.new(|cx| InputState::new(window, cx));
        let llm_model_search = cx.new(|cx| InputState::new(window, cx));
        let subs = vec![
            cx.subscribe_in(&name, window, Self::on_input_change),
            cx.subscribe_in(&prompt, window, Self::on_input_change),
        ];
        (
            StyleInputs {
                name,
                apps: entry.apps.clone(),
                prompt,
                search,
                stt_model_search,
                llm_model_search,
            },
            subs,
        )
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
        let new_openai_key = draft.providers.openai.api_key.clone();
        let new_groq_key = draft.providers.groq.api_key.clone();
        let providers_changed = new_openai_key != self.last_fetched_openai_key
            || new_groq_key != self.last_fetched_groq_key;
        let providers = draft.providers.clone();
        let _ = self.shared.update_config(move |config| *config = draft);
        if providers_changed {
            self.last_fetched_openai_key = new_openai_key;
            self.last_fetched_groq_key = new_groq_key;
            crate::config::fetch_all_models(&providers);

            // Re-apply smart defaults once the fetch verifies providers.
            let shared = self.shared.clone();
            cx.spawn(async move |_this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                if crate::config::any_provider_verified() {
                    let _ = shared.update_config(|config| {
                        crate::config::apply_smart_defaults_initial(config);
                    });
                }
            }).detach();
        }
    }

    fn draft_from_inputs(&self, cx: &gpui::Context<Self>) -> GlideConfig {
        let mut config = self.shared.snapshot().config;

        // Save per-provider credentials
        config.providers.openai.api_key =
            self.openai_inputs.api_key.read(cx).value().to_string();
        config.providers.openai.base_url =
            self.openai_inputs.base_url.read(cx).value().to_string();
        config.providers.groq.api_key =
            self.groq_inputs.api_key.read(cx).value().to_string();
        config.providers.groq.base_url =
            self.groq_inputs.base_url.read(cx).value().to_string();

        config.dictation.system_prompt = self.default_prompt.read(cx).value().to_string();

        config.dictation.styles = self
            .styles
            .iter()
            .map(|s| Style {
                name: s.name.read(cx).value().to_string(),
                apps: s.apps.clone(),
                prompt: s.prompt.read(cx).value().to_string(),
                stt: None,
                llm: None,
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
                        SidebarMenuItem::new("General")
                            .icon(Icon::new(IconName::Settings))
                            .active(self.active_pane == SettingsPane::General)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::General;
                                cx.notify();
                            })),
                    )
                    .child(
                        SidebarMenuItem::new("Styles")
                            .icon(Icon::new(IconName::Palette))
                            .active(self.active_pane == SettingsPane::Styles)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::Styles;
                                cx.notify();
                            })),
                    )
                    .child(
                        SidebarMenuItem::new("Providers")
                            .icon(Icon::new(IconName::Globe))
                            .active(self.active_pane == SettingsPane::Providers)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.active_pane = SettingsPane::Providers;
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
            .child(match self.active_pane {
                SettingsPane::Providers => {
                    self.render_providers_pane(window, cx, &snapshot).into_any_element()
                }
                SettingsPane::Styles => {
                    self.render_styles_pane(window, cx).into_any_element()
                }
                SettingsPane::General => {
                    self.render_general_pane(window, cx, &snapshot).into_any_element()
                }
            })
    }

    fn render_providers_pane(
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

            // Accordion header
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
                    this.child(
                        img(path)
                            .w(gpui::px(24.0))
                            .h(gpui::px(24.0))
                            .rounded_md(),
                    )
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

            // Accordion body
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
                                .child(
                                    div().flex_1().child(
                                        Input::new(&inputs.api_key)
                                            .mask_toggle()
                                            .cleanable(true),
                                    ),
                                )
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

    fn render_styles_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let available_apps = crate::config::list_applications();
        let mut container = div().flex().flex_col().gap_4();

        // -- Default Prompt & Models -------------------------------------------
        let snapshot = self.shared.snapshot();
        let default_stt = snapshot.config.dictation.stt.model.clone();
        let default_llm = snapshot.config.dictation.llm
            .as_ref()
            .map(|s| s.model.clone())
            .unwrap_or_else(|| "Disabled".to_string());
        let stt_models = crate::config::cached_stt_models();
        let llm_models = crate::config::cached_llm_models();

        let shared_stt = self.shared.clone();
        let shared_llm = self.shared.clone();
        let has_provider = crate::config::any_provider_verified();

        container = container.child(
            section_block("Defaults", cx)
                .child(
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
                            let recommended_stt = crate::config::smart_stt_default()
                                .map(|s| s.model);
                            let recommended_llm = crate::config::smart_llm_default()
                                .map(|s| s.model);
                            this.child(
                                setting_row("Voice Model", "Speech-to-text", cx)
                                    .child(
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
                                    ),
                            )
                            .child(
                                setting_row("LLM Model", "Text cleanup", cx)
                                    .child(
                                        model_dropdown_button(
                                            "default-llm",
                                            &default_llm,
                                            &llm_models,
                                            recommended_llm.as_deref(),
                                            shared_llm,
                                            self.default_llm_search.clone(),
                                            |config, model, provider| {
                                                config.dictation.llm = Some(ModelSelection { provider, model });
                                            },
                                        ),
                                    ),
                            )
                        }),
                ),
        );

        // -- Styles (accordion cards) -----------------------------------------
        for (index, style) in self.styles.iter().enumerate() {
            let style_name = style.name.read(cx).value().to_string();
            let display_name = if style_name.is_empty() {
                format!("Style #{}", index + 1)
            } else {
                style_name.clone()
            };
            let is_expanded = self.expanded_style == Some(index);

            // --- Accordion header ---
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

            // --- Accordion body (only when expanded) ---
            let body = if is_expanded {
                // App icons row
                let apps = &style.apps;
                let max_shown = 5;
                let mut app_row = div().flex().items_center().gap_1().flex_wrap();
                for (ai, app) in apps.iter().take(max_shown).enumerate() {
                    let app_clone = app.clone();
                    let icon_el = if let Some(icon_path) = crate::config::app_icon_path(app) {
                        img(icon_path).w(gpui::px(20.0)).h(gpui::px(20.0)).rounded_sm().into_any_element()
                    } else {
                        let ch = app.chars().next().unwrap_or('?').to_uppercase().to_string();
                        div().flex().items_center().justify_center()
                            .w(gpui::px(20.0)).h(gpui::px(20.0)).rounded_sm()
                            .bg(cx.theme().accent.opacity(0.2)).text_xs()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(cx.theme().accent_foreground)
                            .child(ch).into_any_element()
                    };
                    app_row = app_row.child(
                        div().id(SharedString::from(format!("app-{index}-{ai}")))
                            .flex().items_center().gap_1().px_2().py_0p5()
                            .rounded_md().bg(cx.theme().secondary)
                            .border_1().border_color(cx.theme().border).text_xs()
                            .child(icon_el)
                            .child(div().text_color(cx.theme().foreground).child(app.clone()))
                            .child(
                                Button::new(SharedString::from(format!("rm-app-{index}-{ai}")))
                                    .label("×").ghost().xsmall().compact()
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
                        div().text_xs().text_color(cx.theme().muted_foreground)
                            .child(format!("+{}", apps.len() - max_shown)),
                    );
                }

                // App picker popover — exclude apps assigned to ANY style (uniqueness)
                let snap = self.shared.snapshot();
                let all_assigned: std::collections::HashSet<String> = snap.config.dictation.styles.iter()
                    .flat_map(|s| s.apps.iter().cloned())
                    .collect();
                let popover_apps: Vec<String> = available_apps.iter()
                    .filter(|a| !all_assigned.contains(*a)).cloned().collect();
                let entity_for_apps = cx.weak_entity();
                let search_entity = style.search.clone();
                let add_app_popover = Popover::new(SharedString::from(format!("app-picker-{index}")))
                    .trigger(
                        Button::new(SharedString::from(format!("add-app-{index}")))
                            .label("+ Add App").ghost().small().compact(),
                    )
                    .content(move |_state, _window, cx| {
                        let query = search_entity.read(cx).value().to_string();
                        let mut scored: Vec<(&String, i32)> = popover_apps.iter()
                            .filter_map(|a| crate::config::fuzzy_match(&query, a).map(|s| (a, s)))
                            .collect();
                        scored.sort_by(|a, b| b.1.cmp(&a.1));
                        let mut grid = div().flex().flex_wrap().gap_2().p_2();
                        for (app, _) in &scored {
                            let app_name = (*app).clone();
                            let entity = entity_for_apps.clone();
                            let tile_icon = if let Some(p) = crate::config::app_icon_path(app) {
                                img(p).w(gpui::px(40.0)).h(gpui::px(40.0)).rounded_lg().into_any_element()
                            } else {
                                let ch = app.chars().next().unwrap_or('?').to_uppercase().to_string();
                                div().flex().items_center().justify_center()
                                    .w(gpui::px(40.0)).h(gpui::px(40.0)).rounded_lg()
                                    .bg(cx.theme().accent.opacity(0.2)).text_lg()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(cx.theme().accent_foreground)
                                    .child(ch).into_any_element()
                            };
                            grid = grid.child(
                                div().id(SharedString::from(format!("pick-{app_name}")))
                                    .flex().flex_col().items_center().gap_1()
                                    .w(gpui::px(80.0)).py_2().rounded_md().cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().accent.opacity(0.15)))
                                    .child(tile_icon)
                                    .child(div().text_xs().text_color(cx.theme().foreground)
                                        .overflow_x_hidden().max_w(gpui::px(76.0))
                                        .child(app_name.clone()))
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
                        div().w(gpui::px(380.0)).max_h(gpui::px(400.0))
                            .flex().flex_col().overflow_hidden()
                            .child(div().p_2().border_b_1().border_color(cx.theme().border)
                                .child(Input::new(&search_entity)))
                            .child(div().id(SharedString::from(format!("app-grid-scroll-{index}")))
                                .flex_1().overflow_y_scrollbar().child(grid))
                            .into_any_element()
                    });

                // Style name input (editable)
                let name_input = Input::new(&style.name).appearance(false);

                // Per-style model overrides
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
                        // Style name
                        .child(name_input)
                        // Apps
                        .child(
                            div().flex().items_center().gap_2()
                                .child(app_row)
                                .child(add_app_popover),
                        )
                        // System prompt
                        .child(Input::new(&style.prompt))
                        // Model selectors side by side
                        .child(
                            div()
                                .flex()
                                .gap_3()
                                .child(
                                    style_model_dropdown(
                                        &format!("style-stt-{index}"),
                                        &stt_display,
                                        &stt_models,
                                        self.shared.clone(),
                                        style.stt_model_search.clone(),
                                        index,
                                        true,
                                    ),
                                )
                                .child(
                                    style_model_dropdown(
                                        &format!("style-llm-{index}"),
                                        &llm_display,
                                        &llm_models,
                                        self.shared.clone(),
                                        style.llm_model_search.clone(),
                                        index,
                                        false,
                                    ),
                                ),
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

        // -- Add Style button ----------------------------------------------
        container = container.child(
            div().child(
                Button::new("add-style")
                    .label("+ Add Style")
                    .primary()
                    .on_click(cx.listener(|this, _, window, cx| {
                        let default_prompt =
                            this.default_prompt.read(cx).value().to_string();
                        let style_num = this.styles.len() + 1;
                        let entry = Style {
                            name: format!("Style {style_num}"),
                            apps: Vec::new(),
                            prompt: default_prompt,
                            stt: None,
                            llm: None,
                        };
                        let (inputs, subs) =
                            Self::create_style_inputs(&entry, window, cx);
                        this.styles.push(inputs);
                        this._subscriptions.extend(subs);
                        this.schedule_autosave(cx);
                        cx.notify();
                    })),
            ),
        );

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
        let has_notch = crate::config::notch_width().is_some();
        for style in OverlayStyle::ALL {
            // Glow only makes sense on Macs with a notch
            if style == OverlayStyle::Glow && !has_notch {
                continue;
            }
            let is_active = style == current_overlay;
            let icon_text = match style {
                OverlayStyle::Classic => "▁▂▃▅▃▂▁",
                OverlayStyle::Mini => "▪▪▪",
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
        let show_position = current_overlay != OverlayStyle::Glow
            && current_overlay != OverlayStyle::None;
        container = container.child(
            section_block("Recording Window", cx)
                .child(
                    settings_card(cx)
                        .child(
                            setting_row("Style", "Overlay shown while recording", cx)
                        )
                        .child(style_cards)
                        .when(show_position, |card| card
                        .child(
                            setting_row("Position", "Where the overlay appears on screen", cx),
                        )
                        .child({
                            let mut pos_cards = div().flex().gap_3().flex_1();
                            let positions: Vec<_> = crate::config::OverlayPosition::ALL
                                .iter()
                                .filter(|p| **p != crate::config::OverlayPosition::Notch || has_notch)
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
                        })),
                ),
        );

        // -- Keyboard Shortcuts ------------------------------------------
        let hotkey_label = current_trigger
            .map(|t| t.label())
            .unwrap_or_else(|| "Not Set".to_string());
        let toggle_trigger = snapshot.config.hotkey.toggle_trigger;
        let toggle_label = toggle_trigger
            .map(|t| t.label())
            .unwrap_or_else(|| "Not Set".to_string());

        // Poll the CGEventTap for a recorded keycode (set by the background event tap thread)
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

        // Dictation hotkey button
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

        // Toggle dictation button
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

        fn start_hotkey_poll(cx: &mut gpui::Context<SettingsApp>, field: fn(&mut SettingsApp) -> &mut bool) {
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let still_recording = this
                        .update(cx, |this, _| *field(this))
                        .unwrap_or(false);
                    if !still_recording {
                        break;
                    }
                    let _ = this.update(cx, |_, cx| cx.notify());
                }
            })
            .detach();
        }

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
                                div().flex().items_center().gap_1()
                                    .child(
                                        shortcut_button
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.shared.start_hotkey_recording();
                                                this.recording_hotkey = true;
                                                cx.notify();
                                                start_hotkey_poll(cx, |s| &mut s.recording_hotkey);
                                            })),
                                    )
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
                                div().flex().items_center().gap_1()
                                    .child(
                                        toggle_button
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.shared.start_hotkey_recording();
                                                this.recording_toggle_hotkey = true;
                                                cx.notify();
                                                start_hotkey_poll(cx, |s| &mut s.recording_toggle_hotkey);
                                            })),
                                    )
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

        // -- Permissions ------------------------------------------------
        let perms = crate::permissions::check_all();
        let mut perm_card = settings_card(cx);
        for (i, perm) in perms.iter().enumerate() {
            if i > 0 {
                perm_card = perm_card.child(
                    div().h(gpui::px(1.0)).bg(cx.theme().border),
                );
            }
            // Feature-specific icon on the left
            let feature_icon = match perm.icon {
                "bell" => Icon::new(IconName::Bell).size_4(),
                "user" => Icon::new(IconName::User).size_4(),
                "eye" => Icon::new(IconName::Eye).size_4(),
                _ => Icon::new(IconName::Info).size_4(),
            };

            // Status icon + action on the right
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
                                let _ = std::process::Command::new("open")
                                    .arg(url)
                                    .spawn();
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

        container = container.child(
            section_block("Permissions", cx).child(perm_card),
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
                            .child({
                                let shared_for_menu = self.shared.clone();
                                Button::new("top-bar-mic")
                                    .label(SharedString::from(mic_name))
                                    .ghost()
                                    .small()
                                    .compact()
                                    .dropdown_menu(move |menu, _, _| {
                                        let mut m = menu;
                                        for device in &devices {
                                            let d = device.clone();
                                            let shared = shared_for_menu.clone();
                                            m = m.item(
                                                PopupMenuItem::new(SharedString::from(
                                                    d.clone(),
                                                ))
                                                .on_click(move |_, _, _cx| {
                                                    let _ =
                                                        shared.update_config(|config| {
                                                            config.audio.device =
                                                                d.clone();
                                                        });
                                                }),
                                            );
                                        }
                                        m
                                    })
                            }),
                    )
                    // -- Content --
                    .child(self.render_content(window, cx)),
            )
    }
}

// -- Reusable UI helpers ---------------------------------------------------------

#[allow(dead_code)]
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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
        .py_3()
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

fn position_button_preview(pos: OverlayPosition) -> gpui::Div {
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
                bars = bars.child(
                    div()
                        .w(gpui::px(3.0))
                        .h(gpui::px(h))
                        .bg(bar),
                );
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
                bars = bars.child(
                    div()
                        .w(gpui::px(3.0))
                        .h(gpui::px(h))
                        .bg(bar),
                );
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

fn hint_row(text: &str, cx: &App) -> gpui::Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(text.to_string())
}

/// Strip vendor prefixes (e.g. "meta-llama/") from model IDs for display.
fn display_model_name(id: &str) -> &str {
    id.rsplit_once('/').map(|(_, name)| name).unwrap_or(id)
}

/// Render a single model row inside a model picker popover.
fn model_picker_item(
    model: &crate::config::ModelInfo,
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
        .child(img(logo_path).w(gpui::px(16.0)).h(gpui::px(16.0)).rounded_sm())
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(display_model_name(&model.id).to_string()),
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

/// Popover-based model picker with fuzzy search, provider logos, and recommended star.
fn model_dropdown_button(
    id: &str,
    current_model: &str,
    models: &[crate::config::ModelInfo],
    recommended_model: Option<&str>,
    shared: SharedState,
    search_entity: Entity<InputState>,
    updater: fn(&mut GlideConfig, String, Provider),
) -> gpui::Div {
    let current_logo = models
        .iter()
        .find(|m| m.id == current_model)
        .map(|m| m.logo.clone());
    let models = models.to_vec();
    let recommended = recommended_model.map(|s| s.to_string());

    let popover_id = SharedString::from(format!("model-picker-{id}"));

    let trigger = Button::new(SharedString::from(format!("trigger-{id}")))
        .ghost()
        .small()
        .compact()
        .child(
            div()
                .truncate()
                .max_w(gpui::px(180.0))
                .child(display_model_name(current_model).to_string()),
        );

    let popover = Popover::new(popover_id)
        .trigger(trigger)
        .content(move |_state, _window, cx| {
            let popover = cx.entity().clone();
            let query = search_entity.read(cx).value().to_string();
            let filtered: Vec<_> = if query.is_empty() {
                models.iter().collect()
            } else {
                let mut scored: Vec<_> = models
                    .iter()
                    .filter_map(|m| {
                        let display = display_model_name(&m.id);
                        crate::config::fuzzy_match(&query, display).map(|s| (m, s))
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.cmp(&a.1));
                scored.into_iter().map(|(m, _)| m).collect()
            };

            let mut list = div().flex().flex_col().gap_0p5().p_1();
            for model in &filtered {
                let is_recommended = recommended.as_deref() == Some(&model.id);
                let model_id = model.id.clone();
                let model_provider_str = model.provider.clone();
                let shared = shared.clone();
                let popover = popover.clone();
                list = list.child(
                    model_picker_item(model, is_recommended, cx).on_click(
                        move |_, window, cx| {
                            let id = model_id.clone();
                            let provider = crate::config::Provider::from_model_info_provider(&model_provider_str)
                                .unwrap_or(crate::config::Provider::OpenAi);
                            let _ = shared.update_config(|config| {
                                updater(config, id, provider);
                            });
                            popover.update(cx, |state, cx| {
                                state.dismiss(window, cx);
                            });
                        },
                    ),
                );
            }

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
                        .child(Input::new(&search_entity)),
                )
                .child(
                    div()
                        .id(SharedString::from("model-scroll"))
                        .flex_1()
                        .overflow_y_scroll()
                        .child(list),
                )
        });

    let mut wrapper = div().flex().items_center().gap_1().min_w_0().overflow_hidden();
    if let Some(ref logo) = current_logo {
        wrapper = wrapper.child(
            img(crate::config::asset_path(logo))
                .w(gpui::px(16.0))
                .h(gpui::px(16.0))
                .rounded_sm()
                .flex_shrink_0(),
        );
    }
    wrapper.child(div().min_w_0().overflow_hidden().child(popover))
}

/// Popover-based model picker for per-style overrides, with "Default" option.
fn style_model_dropdown(
    id: &str,
    current_display: &str,
    models: &[crate::config::ModelInfo],
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
    let current_logo = models
        .iter()
        .find(|m| m.id == raw_model)
        .map(|m| m.logo.clone());
    let models = models.to_vec();

    let popover_id = SharedString::from(format!("model-picker-{id}"));

    let trigger = Button::new(SharedString::from(format!("trigger-{id}")))
        .ghost()
        .small()
        .compact()
        .child(
            div()
                .truncate()
                .max_w(gpui::px(180.0))
                .child(current_display.to_string()),
        );

    let popover = Popover::new(popover_id)
        .trigger(trigger)
        .content(move |_state, _window, cx| {
            let popover = cx.entity().clone();
            let query = search_entity.read(cx).value().to_string();
            let filtered: Vec<_> = if query.is_empty() {
                models.iter().collect()
            } else {
                let mut scored: Vec<_> = models
                    .iter()
                    .filter_map(|m| {
                        let display = display_model_name(&m.id);
                        crate::config::fuzzy_match(&query, display).map(|s| (m, s))
                    })
                    .collect();
                scored.sort_by(|a, b| b.1.cmp(&a.1));
                scored.into_iter().map(|(m, _)| m).collect()
            };

            // "Default" option
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
                list = list.child(
                    model_picker_item(model, false, cx).on_click(
                        move |_, window, cx| {
                            let id = model_id.clone();
                            let model_provider = model_provider_str_style.clone();
                            let _ = shared.update_config(|config| {
                                let provider = crate::config::Provider::from_model_info_provider(&model_provider)
                                    .unwrap_or(crate::config::Provider::OpenAi);
                                if let Some(style) =
                                    config.dictation.styles.get_mut(style_index)
                                {
                                    if is_stt {
                                        style.stt = Some(ModelSelection { provider, model: id });
                                    } else {
                                        style.llm = Some(ModelSelection { provider, model: id });
                                    }
                                }
                            });
                            popover.update(cx, |state, cx| {
                                state.dismiss(window, cx);
                            });
                        },
                    ),
                );
            }

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
                        .child(Input::new(&search_entity)),
                )
                .child(
                    div()
                        .id(SharedString::from("style-model-scroll"))
                        .flex_1()
                        .overflow_y_scroll()
                        .child(list),
                )
        });

    let mut wrapper = div().flex().items_center().gap_1().min_w_0().overflow_hidden();
    if let Some(ref logo) = current_logo {
        wrapper = wrapper.child(
            img(crate::config::asset_path(logo))
                .w(gpui::px(16.0))
                .h(gpui::px(16.0))
                .rounded_sm()
                .flex_shrink_0(),
        );
    }
    wrapper.child(div().min_w_0().overflow_hidden().child(popover))
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
            assert_eq!(app.active_pane, SettingsPane::General);
            assert!(!app.save_pending);
        });
    }

    #[gpui::test]
    async fn test_input_default_values(cx: &mut TestAppContext) {
        let (view, cx) = init_and_create_view(cx);
        let defaults = GlideConfig::default();

        cx.read_entity(&view, |app, cx| {
            // Check OpenAI provider base URL is loaded
            assert_eq!(
                app.openai_inputs.base_url.read(cx).value().to_string(),
                defaults.providers.openai.base_url,
            );
        });
    }

    #[gpui::test]
    async fn test_pane_switching(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        cx.update_entity(&view, |app, cx| {
            app.active_pane = SettingsPane::Styles;
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::Styles);
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
            assert_eq!(
                draft.providers.openai.base_url,
                defaults.providers.openai.base_url,
            );
            assert_eq!(draft.dictation.system_prompt, defaults.dictation.system_prompt);
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
            assert_eq!(app.active_pane, SettingsPane::General);
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
    async fn test_add_style(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let initial_count = cx.read_entity(&view, |app, _| app.styles.len());

        cx.update(|window, cx| {
            view.update(cx, |app, cx| {
                let entry = Style {
                    name: "TestApp".to_string(),
                    apps: vec![],
                    prompt: "test prompt".to_string(),
                    stt: None,
                    llm: None,
                };
                let (inputs, subs) =
                    SettingsApp::create_style_inputs(&entry, window, cx);
                app.styles.push(inputs);
                app._subscriptions.extend(subs);
                cx.notify();
            });
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.styles.len(), initial_count + 1);
        });
    }

    #[gpui::test]
    async fn test_remove_style(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        let initial_count = cx.read_entity(&view, |app, _| app.styles.len());

        // Add two more styles
        cx.update(|window, cx| {
            view.update(cx, |app, cx| {
                for name in ["Extra1", "Extra2"] {
                    let entry = Style {
                        name: name.to_string(),
                        apps: vec![],
                        prompt: "prompt".to_string(),
                        stt: None,
                        llm: None,
                    };
                    let (inputs, subs) =
                        SettingsApp::create_style_inputs(&entry, window, cx);
                    app.styles.push(inputs);
                    app._subscriptions.extend(subs);
                }
                cx.notify();
            });
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.styles.len(), initial_count + 2);
        });

        // Remove the first one
        cx.update_entity(&view, |app, cx| {
            app.styles.remove(0);
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.styles.len(), initial_count + 1);
        });
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
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.api_key.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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

        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

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
            assert!(!draft.providers.openai.base_url.is_empty());
        });
    }
}

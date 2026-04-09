mod helpers;
mod panes;

use std::time::Duration;

use gpui::prelude::*;
use gpui::{App, Entity, SharedString, Subscription, Window, div};
use gpui_component::ActiveTheme;
use gpui_component::Side;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{InputEvent, InputState};
use gpui_component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem, SidebarToggleButton};
use gpui_component::theme::{Theme, ThemeMode};
use gpui_component::{Icon, IconName};

use crate::app::SharedState;
use crate::config::{ColorAccent, GlideConfig, Style, ThemePreference};

const AUTOSAVE_DELAY: Duration = Duration::from_millis(800);

pub fn apply_theme_preference(
    pref: ThemePreference,
    accent: ColorAccent,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    match pref {
        ThemePreference::System => Theme::sync_system_appearance(window, cx),
        ThemePreference::Light => Theme::change(ThemeMode::Light, window, cx),
        ThemePreference::Dark => Theme::change(ThemeMode::Dark, window, cx),
    }

    // Apply accent color overrides on top of the base light/dark theme
    let (h, s, l, a) = accent.primary_hsla();
    let (hh, sh, lh, ah) = accent.primary_hover_hsla();
    let (ha, sa, la, aa) = accent.primary_active_hsla();
    let theme = cx.global_mut::<Theme>();
    theme.colors.primary = gpui::hsla(h, s, l, a);
    theme.colors.primary_hover = gpui::hsla(hh, sh, lh, ah);
    theme.colors.primary_active = gpui::hsla(ha, sa, la, aa);
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

pub(crate) struct StyleInputs {
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

        let openai_creds = &config.providers.openai;
        let openai_api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(&openai_creds.api_key)
        });
        let openai_base_url =
            cx.new(|cx| InputState::new(window, cx).default_value(&openai_creds.base_url));

        let groq_creds = &config.providers.groq;
        let groq_api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(&groq_creds.api_key)
        });
        let groq_base_url =
            cx.new(|cx| InputState::new(window, cx).default_value(&groq_creds.base_url));

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
                let (inputs, entry_subs) = Self::create_style_inputs(entry, window, cx);
                subs.extend(entry_subs);
                inputs
            })
            .collect();

        let theme_shared = shared.clone();
        subs.push(
            cx.observe_window_appearance(window, move |_this, window, cx| {
                let snap = theme_shared.snapshot();
                apply_theme_preference(
                    snap.config.app.theme,
                    snap.config.app.accent,
                    Some(window),
                    cx,
                );
            }),
        );

        let shared_for_defaults = shared.clone();
        cx.spawn(async move |_this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            if crate::config::any_provider_verified() {
                let _ = shared_for_defaults.update_config(|config| {
                    crate::config::apply_smart_defaults_initial(config);
                });
            }
        })
        .detach();

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
        let name = cx.new(|cx| InputState::new(window, cx).default_value(&entry.name));
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

            let shared = self.shared.clone();
            cx.spawn(async move |_this, cx| {
                cx.background_executor().timer(Duration::from_secs(3)).await;
                if crate::config::any_provider_verified() {
                    let _ = shared.update_config(|config| {
                        crate::config::apply_smart_defaults_initial(config);
                    });
                }
            })
            .detach();
        }
    }

    fn draft_from_inputs(&self, cx: &gpui::Context<Self>) -> GlideConfig {
        let mut config = self.shared.snapshot().config;

        config.providers.openai.api_key = self.openai_inputs.api_key.read(cx).value().to_string();
        config.providers.openai.base_url = self.openai_inputs.base_url.read(cx).value().to_string();
        config.providers.groq.api_key = self.groq_inputs.api_key.read(cx).value().to_string();
        config.providers.groq.base_url = self.groq_inputs.base_url.read(cx).value().to_string();

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

        Sidebar::new(Side::Left).collapsed(collapsed).child(
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
                SettingsPane::Providers => self
                    .render_providers_pane(window, cx, &snapshot)
                    .into_any_element(),
                SettingsPane::Styles => self.render_styles_pane(window, cx).into_any_element(),
                SettingsPane::General => self
                    .render_general_pane(window, cx, &snapshot)
                    .into_any_element(),
            })
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
                                        this.sidebar_collapsed = !this.sidebar_collapsed;
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
                                                PopupMenuItem::new(SharedString::from(d.clone()))
                                                    .on_click(move |_, _, _cx| {
                                                        let _ = shared.update_config(|config| {
                                                            config.audio.device = d.clone();
                                                        });
                                                    }),
                                            );
                                        }
                                        m
                                    })
                            }),
                    )
                    .child(self.render_content(window, cx)),
            )
    }
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

    fn init_and_create_view(cx: &mut TestAppContext) -> (Entity<SettingsApp>, VisualTestContext) {
        cx.update(|app| {
            gpui_component::init(app);
        });

        let shared = test_shared_state();
        let (view, cx) = cx.add_window_view(|window, cx| SettingsApp::new(shared, window, cx));
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
            assert_eq!(
                draft.dictation.system_prompt,
                defaults.dictation.system_prompt
            );
        });
    }

    #[gpui::test]
    async fn test_simulate_typing(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        cx.simulate_input("hello");
        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::General);
        });
    }

    #[gpui::test]
    async fn test_autosave_scheduling(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);

        cx.update_entity(&view, |app, cx| {
            app.schedule_autosave(cx);
        });

        cx.read_entity(&view, |app, _| {
            assert!(app.save_pending);
        });

        cx.executor()
            .advance_clock(AUTOSAVE_DELAY + std::time::Duration::from_millis(100));
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
                let (inputs, subs) = SettingsApp::create_style_inputs(&entry, window, cx);
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
                    let (inputs, subs) = SettingsApp::create_style_inputs(&entry, window, cx);
                    app.styles.push(inputs);
                    app._subscriptions.extend(subs);
                }
                cx.notify();
            });
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.styles.len(), initial_count + 2);
        });

        cx.update_entity(&view, |app, cx| {
            app.styles.remove(0);
            cx.notify();
        });

        cx.read_entity(&view, |app, _| {
            assert_eq!(app.styles.len(), initial_count + 1);
        });
    }

    #[gpui::test]
    async fn test_input_replace_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "test-text", window, cx);
            });
        });

        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("test-text"));
        });
    }

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

    #[gpui::test]
    async fn test_rapid_typing_stress(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        for ch in "hello world, this is a longer sentence for stress testing".chars() {
            cx.update(|window, cx| {
                input_entity.update(cx, |state, cx| {
                    use gpui::EntityInputHandler;
                    state.replace_text_in_range(None, &ch.to_string(), window, cx);
                });
            });
        }

        cx.run_until_parked();

        cx.read_entity(&input_entity, |state, _| {
            assert!(state.value().to_string().contains("stress testing"));
        });
    }

    #[gpui::test]
    async fn test_ime_marked_text_and_render(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_and_mark_text_in_range(None, "abc", Some(0..3), window, cx);
            });
        });

        cx.run_until_parked();

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

    #[gpui::test]
    async fn test_bounds_for_range_after_typing(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "hello", window, cx);
            });
        });

        cx.run_until_parked();

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                let bounds = gpui::Bounds {
                    origin: gpui::point(gpui::px(0.), gpui::px(0.)),
                    size: gpui::size(gpui::px(400.), gpui::px(30.)),
                };
                let _result = state.bounds_for_range(0..5, bounds, window, cx);
            });
        });
    }

    #[gpui::test]
    async fn test_input_change_subscription_fires(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        cx.read_entity(&view, |app, _| {
            assert!(!app.save_pending);
        });

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "x", window, cx);
            });
        });

        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert!(
                app.save_pending,
                "subscription should have triggered autosave"
            );
        });

        cx.executor()
            .advance_clock(AUTOSAVE_DELAY + std::time::Duration::from_millis(100));
        cx.run_until_parked();

        cx.read_entity(&view, |app, _| {
            assert!(!app.save_pending, "autosave should have completed");
        });
    }

    #[gpui::test]
    async fn test_full_render_after_edit(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| app.openai_inputs.base_url.clone());

        cx.update(|window, cx| {
            input_entity.update(cx, |state, cx| {
                use gpui::EntityInputHandler;
                state.replace_text_in_range(None, "gpt-4o", window, cx);
            });
        });

        cx.update_entity(&view, |_, cx| {
            cx.notify();
        });

        cx.run_until_parked();

        cx.update_entity(&view, |app, cx| {
            let draft = app.draft_from_inputs(cx);
            assert!(!draft.providers.openai.base_url.is_empty());
        });
    }
}

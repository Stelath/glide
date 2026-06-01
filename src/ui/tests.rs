use super::*;
use std::sync::Arc;

use gpui::{AppContext, TestAppContext, VisualTestContext};

use crate::app::state::SharedAppState;
use crate::config::{GlideConfig, ModelSelection, Provider};

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

mod settings_state {
    use super::*;

    #[test]
    fn disable_llm_rewrite_persists_disabled_default() {
        let shared = test_shared_state();
        shared
            .update_config(|config| {
                config.dictation.llm = Some(ModelSelection {
                    provider: Provider::OpenAi,
                    model: "gpt-5.4-nano".to_string(),
                });
                config.dictation.smart_defaults_applied = false;
            })
            .unwrap();

        shared.update_config(helpers::disable_llm_rewrite).unwrap();

        let config = shared.snapshot().config;
        assert!(config.dictation.llm.is_none());
        assert!(config.dictation.smart_defaults_applied);
    }

    #[gpui::test]
    async fn settings_app_creation(cx: &mut TestAppContext) {
        let (view, cx) = init_and_create_view(cx);
        cx.read_entity(&view, |app, _| {
            assert_eq!(app.active_pane, SettingsPane::General);
            assert!(!app.save_pending);
        });
    }

    #[gpui::test]
    async fn default_input_values_match_config(cx: &mut TestAppContext) {
        let (view, cx) = init_and_create_view(cx);
        let defaults = GlideConfig::default();

        cx.read_entity(&view, |app, cx| {
            assert_eq!(
                app.provider_inputs_for(Provider::OpenAi)
                    .base_url
                    .read(cx)
                    .value()
                    .to_string(),
                defaults.providers.openai.base_url,
            );
            assert_eq!(
                app.provider_inputs_for(Provider::Cerebras)
                    .base_url
                    .read(cx)
                    .value()
                    .to_string(),
                defaults.providers.cerebras.base_url,
            );
            assert_eq!(
                app.provider_inputs_for(Provider::Fireworks)
                    .base_url
                    .read(cx)
                    .value()
                    .to_string(),
                defaults.providers.fireworks.base_url,
            );
            assert_eq!(
                app.provider_inputs_for(Provider::ElevenLabs)
                    .base_url
                    .read(cx)
                    .value()
                    .to_string(),
                defaults.providers.elevenlabs.base_url,
            );
        });
    }

    #[gpui::test]
    async fn draft_from_inputs_matches_config(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let defaults = GlideConfig::default();

        cx.update_entity(&view, |app, cx| {
            let draft = app.draft_from_inputs(cx);
            assert_eq!(
                draft.providers.openai.base_url,
                defaults.providers.openai.base_url,
            );
            assert_eq!(
                draft.providers.cerebras.base_url,
                defaults.providers.cerebras.base_url,
            );
            assert_eq!(
                draft.dictation.system_prompt,
                defaults.dictation.system_prompt
            );
        });
    }
}

mod inputs_autosave {
    use super::*;

    #[gpui::test]
    async fn autosave_scheduling(cx: &mut TestAppContext) {
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
    async fn input_change_subscription_schedules_autosave(cx: &mut TestAppContext) {
        let (view, mut cx) = init_and_create_view(cx);
        let input_entity = cx.read_entity(&view, |app, _| {
            app.provider_inputs_for(Provider::OpenAi).base_url.clone()
        });

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
}

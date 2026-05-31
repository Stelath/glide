//! Builds the editable input-field entities for providers and styles, and
//! reconstructs a [`GlideConfig`] draft from the current input values.

use gpui::Subscription;
use gpui::prelude::*;
use gpui_component::input::InputState;

use super::{ProviderInputs, SettingsApp, StyleInputs};
use crate::config::{GlideConfig, Provider, Style};

impl SettingsApp {
    pub(in crate::ui) fn create_provider_inputs(
        provider: Provider,
        credentials: &crate::config::ProviderCredentials,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> (ProviderInputs, Vec<Subscription>) {
        let api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .masked(true)
                .default_value(&credentials.api_key)
        });
        let base_url =
            cx.new(|cx| InputState::new(window, cx).default_value(&credentials.base_url));
        let subs = vec![
            cx.subscribe_in(&api_key, window, Self::on_input_change),
            cx.subscribe_in(&base_url, window, Self::on_input_change),
        ];

        (
            ProviderInputs {
                provider,
                api_key,
                base_url,
            },
            subs,
        )
    }

    pub(in crate::ui) fn provider_inputs_for(&self, provider: Provider) -> &ProviderInputs {
        self.provider_inputs
            .iter()
            .find(|inputs| inputs.provider == provider)
            .expect("remote provider inputs should exist")
    }

    pub(in crate::ui) fn create_style_inputs(
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
                prompt_expanded: false,
                search,
                stt_model_search,
                llm_model_search,
            },
            subs,
        )
    }

    pub(in crate::ui) fn draft_from_inputs(&self, cx: &gpui::Context<Self>) -> GlideConfig {
        let mut config = self.shared.snapshot().config;

        for inputs in &self.provider_inputs {
            let credentials = config.providers.credentials_for_mut(inputs.provider);
            credentials.api_key = inputs.api_key.read(cx).value().to_string();
            credentials.base_url = inputs.base_url.read(cx).value().to_string();
        }

        config.dictation.system_prompt = self.default_prompt.read(cx).value().to_string();
        config.dictation.sync_system_prompt_default_flag();

        let existing_styles = config.dictation.styles.clone();
        config.dictation.styles = self
            .styles
            .iter()
            .enumerate()
            .map(|s| Style {
                name: s.1.name.read(cx).value().to_string(),
                apps: s.1.apps.clone(),
                prompt: s.1.prompt.read(cx).value().to_string(),
                stt: existing_styles.get(s.0).and_then(|style| style.stt.clone()),
                llm: existing_styles.get(s.0).and_then(|style| style.llm.clone()),
            })
            .collect();

        config
    }
}

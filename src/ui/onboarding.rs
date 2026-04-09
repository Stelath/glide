use std::time::Duration;

use gpui::prelude::*;
use gpui::{div, img, px, SharedString, Window};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Sizable;
use gpui_component::{Icon, IconName};

use crate::config::HotkeyTrigger;
use crate::permissions;

// ---------------------------------------------------------------------------
// OS-dependent hotkey presets
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn hotkey_presets() -> [(HotkeyTrigger, &'static str); 4] {
    [
        // keycode 61 = Right Option only (HotkeyTrigger::Option would match both L+R)
        (HotkeyTrigger::Custom(61), "\u{2325} Right Option (recommended)"),
        (HotkeyTrigger::CommandRight, "\u{2318} Right Command"),
        (HotkeyTrigger::Custom(63), "Fn"),
        (HotkeyTrigger::F8, "F8"),
    ]
}

#[cfg(target_os = "windows")]
fn hotkey_presets() -> [(HotkeyTrigger, &'static str); 4] {
    [
        (HotkeyTrigger::F8, "F8 (recommended)"),
        (HotkeyTrigger::F9, "F9"),
        (HotkeyTrigger::F10, "F10"),
        (HotkeyTrigger::Option, "Right Alt"),
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn hotkey_presets() -> [(HotkeyTrigger, &'static str); 4] {
    [
        (HotkeyTrigger::F8, "F8 (recommended)"),
        (HotkeyTrigger::F9, "F9"),
        (HotkeyTrigger::F10, "F10"),
        (HotkeyTrigger::Option, "Right Alt"),
    ]
}

pub(super) fn default_hotkey_preset() -> HotkeyTrigger {
    hotkey_presets()[0].0
}

#[cfg(target_os = "macos")]
fn hotkey_hint_text() -> &'static str {
    "Pick the key you\u{2019}ll hold to record. Right Option is \
     great because it\u{2019}s rarely used by other apps."
}

#[cfg(target_os = "windows")]
fn hotkey_hint_text() -> &'static str {
    "Pick the key you\u{2019}ll hold to record. F8 works well \
     because it\u{2019}s rarely used by other apps."
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn hotkey_hint_text() -> &'static str {
    "Pick the key you\u{2019}ll hold to record. F8 works well \
     because it\u{2019}s rarely used by other apps."
}

// ---------------------------------------------------------------------------
// Step enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OnboardingStep {
    Welcome,
    Permissions,
    Hotkey,
    HowItWorks,
}

impl OnboardingStep {
    const ALL: [Self; 4] = [Self::Welcome, Self::Permissions, Self::Hotkey, Self::HowItWorks];

    fn index(self) -> usize {
        match self {
            Self::Welcome => 0,
            Self::Permissions => 1,
            Self::Hotkey => 2,
            Self::HowItWorks => 3,
        }
    }

    fn next(self) -> Option<Self> {
        match self {
            Self::Welcome => Some(Self::Permissions),
            Self::Permissions => Some(Self::Hotkey),
            Self::Hotkey => Some(Self::HowItWorks),
            Self::HowItWorks => None,
        }
    }

    fn prev(self) -> Option<Self> {
        match self {
            Self::Welcome => None,
            Self::Permissions => Some(Self::Welcome),
            Self::Hotkey => Some(Self::Permissions),
            Self::HowItWorks => Some(Self::Hotkey),
        }
    }
}

// ---------------------------------------------------------------------------
// Permission state
// ---------------------------------------------------------------------------

pub(super) struct PermissionState {
    pub microphone: bool,
    pub accessibility: bool,
    pub input_monitoring: bool,
}

impl PermissionState {
    pub(super) fn check() -> Self {
        Self {
            microphone: permissions::has_microphone_access(),
            accessibility: permissions::has_accessibility_access(),
            input_monitoring: permissions::has_input_monitoring_access(),
        }
    }

    fn all_granted(&self) -> bool {
        self.microphone && self.accessibility && self.input_monitoring
    }
}

// ---------------------------------------------------------------------------
// Onboarding methods on SettingsApp
// ---------------------------------------------------------------------------

use super::SettingsApp;

impl SettingsApp {
    // -------------------------------------------------------------------
    // Navigation
    // -------------------------------------------------------------------

    fn onboarding_can_advance(&self) -> bool {
        match self.onboarding_step {
            OnboardingStep::Hotkey => self.onboarding_selected_trigger.is_some(),
            _ => true,
        }
    }

    fn onboarding_advance(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) {
        if let Some(next) = self.onboarding_step.next() {
            // Save hotkey when leaving the hotkey step
            if self.onboarding_step == OnboardingStep::Hotkey {
                if let Some(trigger) = self.onboarding_selected_trigger {
                    let _ = self.shared.update_config(|config| {
                        config.hotkey.trigger = Some(trigger);
                    });
                }
            }
            self.onboarding_step = next;
            cx.notify();
        } else {
            self.complete_onboarding(cx);
        }
    }

    fn onboarding_go_back(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(prev) = self.onboarding_step.prev() {
            self.onboarding_step = prev;
            cx.notify();
        }
    }

    fn complete_onboarding(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(trigger) = self.onboarding_selected_trigger {
            let _ = self.shared.update_config(|config| {
                config.hotkey.trigger = Some(trigger);
                config.app.onboarding_completed = true;
            });
        }
        self.show_onboarding = false;
        cx.notify();
    }

    // -------------------------------------------------------------------
    // Main overlay render
    // -------------------------------------------------------------------

    pub(super) fn render_onboarding_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .min_h_0()
                    .p(px(32.0))
                    .id("onboarding-content")
                    .overflow_y_scroll()
                    .child(match self.onboarding_step {
                        OnboardingStep::Welcome => {
                            self.render_ob_welcome(window, cx).into_any_element()
                        }
                        OnboardingStep::Permissions => {
                            self.render_ob_permissions(window, cx).into_any_element()
                        }
                        OnboardingStep::Hotkey => {
                            self.render_ob_hotkey(window, cx).into_any_element()
                        }
                        OnboardingStep::HowItWorks => {
                            self.render_ob_how_it_works(window, cx).into_any_element()
                        }
                    }),
            )
            .child(self.render_ob_footer(window, cx))
    }

    // -------------------------------------------------------------------
    // Step renderers
    // -------------------------------------------------------------------

    fn render_ob_welcome(
        &self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let accent = self.shared.snapshot().config.app.accent;
        let icon_path = crate::config::accent_icon_path(accent)
            .unwrap_or_else(|| crate::config::asset_path("assets/icons/logo.svg"));

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(20.0))
            .max_w(px(420.0))
            .child(img(icon_path).w(px(80.0)).h(px(80.0)))
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Welcome to Glide"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .child(
                        "Voice-to-text dictation for macOS. Hold a hotkey, speak, \
                         release \u{2014} cleaned text is pasted into any app.",
                    ),
            )
    }

    fn render_ob_permissions(
        &self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let perms: [(&str, &str, bool, &str, IconName); 3] = [
            (
                "Microphone",
                "Capture audio for dictation",
                self.onboarding_perm_state.microphone,
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
                IconName::Bell,
            ),
            (
                "Accessibility",
                "Paste transcribed text",
                self.onboarding_perm_state.accessibility,
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
                IconName::User,
            ),
            (
                "Input Monitoring",
                "Global hotkey detection",
                self.onboarding_perm_state.input_monitoring,
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
                IconName::Eye,
            ),
        ];

        let mut card = div()
            .flex()
            .flex_col()
            .w_full()
            .max_w(px(460.0))
            .px_4()
            .py_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted);

        for (i, (name, desc, granted, url, icon_name)) in perms.iter().enumerate() {
            if i > 0 {
                card = card.child(div().h(px(1.0)).bg(cx.theme().border));
            }

            let right_side = if *granted {
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgb(0x22C55E))
                            .child("Granted"),
                    )
                    .child(
                        Icon::new(IconName::CircleCheck)
                            .size_4()
                            .text_color(gpui::rgb(0x22C55E)),
                    )
                    .into_any_element()
            } else {
                let url_owned = url.to_string();
                Button::new(SharedString::from(format!("open-{name}")))
                    .label("Open Settings")
                    .small()
                    .compact()
                    .primary()
                    .on_click(move |_, _, _cx| {
                        let _ = std::process::Command::new("open")
                            .arg(&url_owned)
                            .spawn();
                    })
                    .into_any_element()
            };

            card = card.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .py_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(Icon::new(icon_name.clone()).size_4())
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
                                            .child(*name),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(*desc),
                                    ),
                            ),
                    )
                    .child(right_side),
            );
        }

        let mut container = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.0))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Grant Permissions"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .max_w(px(400.0))
                    .child(
                        "Glide needs these macOS permissions to work. \
                         Click each to open System Settings.",
                    ),
            )
            .child(card);

        if self.onboarding_perm_state.all_granted() {
            container = container.child(
                div()
                    .text_sm()
                    .text_color(gpui::rgb(0x22C55E))
                    .child("All permissions granted!"),
            );
        }

        container
    }

    fn render_ob_hotkey(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        // Poll for recorded keycode if recording
        if self.onboarding_recording_custom {
            if let Some(code) = self.shared.poll_recorded_keycode() {
                self.onboarding_selected_trigger = Some(HotkeyTrigger::from_keycode(code));
                self.onboarding_recording_custom = false;
            }
        }

        let presets: [(HotkeyTrigger, &str); 4] = hotkey_presets();

        let mut preset_buttons = div()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .w_full()
            .max_w(px(360.0));

        for (trigger, label) in presets {
            let is_selected = self.onboarding_selected_trigger == Some(trigger);
            preset_buttons = preset_buttons.child(
                div()
                    .id(SharedString::from(format!("preset-{}", trigger.label())))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .py_3()
                    .rounded_lg()
                    .border_2()
                    .cursor_pointer()
                    .when(is_selected, |d| {
                        d.border_color(cx.theme().primary)
                            .bg(cx.theme().primary.opacity(0.08))
                    })
                    .when(!is_selected, |d| {
                        d.border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(label),
                    )
                    .when(is_selected, |d| {
                        d.child(
                            Icon::new(IconName::Check)
                                .size_4()
                                .text_color(cx.theme().primary),
                        )
                    })
                    .on_click(cx.listener(move |this, _, _window, cx| {
                        this.onboarding_selected_trigger = Some(trigger);
                        this.onboarding_recording_custom = false;
                        cx.notify();
                    })),
            );
        }

        // Custom key recording
        let is_custom = matches!(self.onboarding_selected_trigger, Some(HotkeyTrigger::Custom(_)));
        let custom_label = if self.onboarding_recording_custom {
            "Press any key\u{2026}".to_string()
        } else if is_custom {
            self.onboarding_selected_trigger.unwrap().label()
        } else {
            "Record Custom Key".to_string()
        };

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(16.0))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("Choose Your Hotkey"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .max_w(px(400.0))
                    .child(hotkey_hint_text()),
            )
            .child(preset_buttons)
            .child(
                div()
                    .id("custom-key")
                    .flex()
                    .items_center()
                    .justify_center()
                    .px_4()
                    .py_3()
                    .w_full()
                    .max_w(px(360.0))
                    .rounded_lg()
                    .border_2()
                    .cursor_pointer()
                    .when(is_custom || self.onboarding_recording_custom, |d| {
                        d.border_color(cx.theme().primary)
                    })
                    .when(!(is_custom || self.onboarding_recording_custom), |d| {
                        d.border_color(cx.theme().border)
                    })
                    .bg(cx.theme().muted)
                    .child(
                        div()
                            .text_sm()
                            .text_color(if self.onboarding_recording_custom {
                                cx.theme().primary
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(custom_label),
                    )
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.shared.start_hotkey_recording();
                        this.onboarding_recording_custom = true;
                        cx.notify();
                        // Poll for recorded keycode
                        cx.spawn(async move |view, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(Duration::from_millis(50))
                                    .await;
                                let still = view
                                    .update(cx, |v, _| v.onboarding_recording_custom)
                                    .unwrap_or(false);
                                if !still {
                                    break;
                                }
                                let _ = view.update(cx, |_, cx| cx.notify());
                            }
                        })
                        .detach();
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .text_center()
                    .max_w(px(360.0))
                    .child(
                        "Hold this key to record audio. Release to transcribe and paste. \
                         You can also set a toggle key later in Settings.",
                    ),
            )
    }

    fn render_ob_how_it_works(
        &self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let trigger_label = self
            .onboarding_selected_trigger
            .map(|t| t.label())
            .unwrap_or_else(|| "your hotkey".to_string());

        let steps: [(&str, String, &str); 3] = [
            ("1", format!("Hold {trigger_label}"), "Start speaking naturally"),
            ("2", "Speak".to_string(), "Glide captures and transcribes your voice"),
            (
                "3",
                format!("Release {trigger_label}"),
                "Cleaned text is pasted into the active app",
            ),
        ];

        let mut flow = div().flex().flex_col().gap(px(16.0)).max_w(px(420.0));

        for (num, title, desc) in &steps {
            flow = flow.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.0))
                    .child(
                        div()
                            .w(px(32.0))
                            .h(px(32.0))
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .bg(cx.theme().primary)
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(cx.theme().primary_foreground)
                            .child(*num),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(desc.to_string()),
                            ),
                    ),
            );
        }

        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(20.0))
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child("You\u{2019}re All Set"),
            )
            .child(flow)
    }

    // -------------------------------------------------------------------
    // Footer with progress dots and navigation
    // -------------------------------------------------------------------

    fn render_ob_footer(
        &self,
        _window: &mut Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let step_idx = self.onboarding_step.index();
        let is_last = self.onboarding_step == OnboardingStep::HowItWorks;
        let can_continue = self.onboarding_can_advance();

        // Progress dots
        let mut dots = div().flex().gap(px(8.0)).items_center();
        for i in 0..OnboardingStep::ALL.len() {
            let is_current = i == step_idx;
            let is_past = i < step_idx;
            dots = dots.child(
                div()
                    .w(if is_current { px(24.0) } else { px(8.0) })
                    .h(px(8.0))
                    .rounded(px(4.0))
                    .bg(if is_current || is_past {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    }),
            );
        }

        let next_label = if is_last { "Get Started" } else { "Continue" };

        // Back button
        let back = if step_idx > 0 {
            Button::new("onboarding-back")
                .label("Back")
                .ghost()
                .on_click(cx.listener(|this, _, _window, cx| {
                    this.onboarding_go_back(cx);
                }))
                .into_any_element()
        } else {
            // Invisible spacer to keep layout balanced
            div().w(px(64.0)).into_any_element()
        };

        // Use outline style for Continue button when Slate accent would be
        // invisible in dark mode (Slate primary is near-black).
        let accent = self.shared.snapshot().config.app.accent;
        let use_outline = accent == crate::config::ColorAccent::Slate;

        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_6()
            .py_4()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(back)
            .child(dots)
            .child({
                let btn = Button::new("onboarding-next")
                    .label(next_label)
                    .when(!can_continue, |btn| btn.disabled(true))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.onboarding_advance(window, cx);
                    }));
                if use_outline {
                    btn.outline()
                } else {
                    btn.primary()
                }
            })
    }
}

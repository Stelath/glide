use std::time::Duration;

use gpui::prelude::*;
use gpui::{SharedString, div};
use gpui_component::ActiveTheme;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{Icon, IconName};

use crate::config::{ColorAccent, HotkeyTrigger, OverlayStyle, ThemePreference};
use crate::state::AppSnapshot;

use super::super::helpers::*;
use super::super::{SettingsApp, apply_theme_preference};

impl SettingsApp {
    pub(in crate::ui) fn render_general_pane(
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
        let is_macos = cfg!(target_os = "macos");
        let has_notch = crate::platform::notch_width().is_some();
        for style in OverlayStyle::ALL {
            if style == OverlayStyle::Glow && !is_macos {
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

        if self.recording_hotkey
            && let Some(code) = self.shared.poll_recorded_keycode()
        {
            let trigger = HotkeyTrigger::from_keycode(code);
            let _ = self.shared.update_config(|config| {
                config.hotkey.trigger = Some(trigger);
            });
            self.recording_hotkey = false;
            self.schedule_autosave(cx);
            cx.notify();
        }
        if self.recording_toggle_hotkey
            && let Some(code) = self.shared.poll_recorded_keycode()
        {
            let trigger = HotkeyTrigger::from_keycode(code);
            let _ = self.shared.update_config(|config| {
                config.hotkey.toggle_trigger = Some(trigger);
            });
            self.recording_toggle_hotkey = false;
            self.schedule_autosave(cx);
            cx.notify();
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
        let perms = &self.permission_statuses;
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

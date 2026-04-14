//! Welcome overlay and Account settings pane.

use gpui::prelude::*;
use gpui::{AnyElement, Div, SharedString, div, px};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Sizable;
use gpui_component::button::{Button, ButtonVariants};

use crate::account::{self, Account, AccountState};

use super::SettingsApp;

impl SettingsApp {
    pub(super) fn render_welcome_overlay(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let google_available = account::google::config::is_configured();
        let apple_available = account::apple::IS_AVAILABLE;
        let pending = self.account_sign_in_pending;

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(cx.theme().background)
            .p_8()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .mb_8()
                    .child(
                        div()
                            .text_3xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Welcome to Glide"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .max_w(px(420.0))
                            .child(
                                "Sign in to sync your account and unlock a plan without \
                                 managing your own API keys.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .w(px(320.0))
                    .child(welcome_button(
                        "welcome-apple",
                        "Sign in with Apple",
                        true,
                        !apple_available || pending,
                        cx.listener(|this, _, _window, cx| {
                            this.start_apple_sign_in(cx);
                        }),
                    ))
                    .child(welcome_button(
                        "welcome-google",
                        "Continue with Google",
                        false,
                        !google_available || pending,
                        cx.listener(|this, _, _window, cx| {
                            this.start_google_sign_in(cx);
                        }),
                    ))
                    .child(
                        Button::new("welcome-guest")
                            .ghost()
                            .small()
                            .label(SharedString::from("Continue as Guest"))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.continue_as_guest(cx);
                            })),
                    )
                    .child(if !google_available || !apple_available {
                        let mut msgs = Vec::new();
                        if !apple_available {
                            msgs.push("Apple sign in is coming soon on macOS.");
                        }
                        if !google_available {
                            msgs.push("Google sign in is not configured in this build.");
                        }
                        div()
                            .mt_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(msgs.join(" "))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(if let Some(err) = self.account_error.as_deref() {
                        div()
                            .mt_2()
                            .p_3()
                            .rounded_md()
                            .bg(cx.theme().danger.opacity(0.1))
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(err.to_string())
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
    }
}

fn welcome_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    disabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Button {
    let base = Button::new(id).label(SharedString::from(label)).w_full();
    let mut button = if primary { base.primary() } else { base.outline() };
    if disabled {
        button = button.disabled(true);
    } else {
        button = button.on_click(on_click);
    }
    button
}

impl SettingsApp {
    pub(super) fn render_account_pane(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let snapshot = self.shared.snapshot();
        let account_state = snapshot.config.account.clone();

        let mut container = div().flex().flex_col().gap_4();

        container = container.child(
            super::helpers::section_block("Account", cx).child(
                super::helpers::settings_card(cx)
                    .child(self.render_account_body(&account_state, cx)),
            ),
        );

        if let Some(error) = self.account_error.as_deref() {
            container = container.child(
                div()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .bg(cx.theme().danger.opacity(0.08))
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(error.to_string()),
            );
        }

        container
    }

    fn render_account_body(
        &self,
        state: &AccountState,
        cx: &mut gpui::Context<Self>,
    ) -> AnyElement {
        match &state.current {
            Some(account) => self.render_signed_in_body(account, cx).into_any_element(),
            None => self.render_guest_body(cx).into_any_element(),
        }
    }

    fn render_signed_in_body(
        &self,
        account: &Account,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let name = account.short_display_name();
        let email = account.email.clone().unwrap_or_default();
        let provider_label = format!("Signed in with {}", account.provider.display_name());
        let initials = account.initials();

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(avatar_circle(&initials, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(email),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(provider_label),
                            ),
                    ),
            )
            .child(
                Button::new("account-sign-out")
                    .danger()
                    .small()
                    .label(SharedString::from("Sign Out"))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.sign_out(cx);
                    })),
            )
    }

    fn render_guest_body(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let google_available = account::google::config::is_configured();
        let apple_available = account::apple::IS_AVAILABLE;
        let pending = self.account_sign_in_pending;

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "You're using Glide as a guest. Sign in to unlock a plan (coming soon).",
                    ),
            )
            .child({
                let mut apple = Button::new("account-apple")
                    .primary()
                    .small()
                    .label(SharedString::from("Sign in with Apple"));
                if apple_available && !pending {
                    apple = apple.on_click(cx.listener(|this, _, _window, cx| {
                        this.start_apple_sign_in(cx);
                    }));
                } else {
                    apple = apple.disabled(true);
                }
                apple
            })
            .child({
                let mut google = Button::new("account-google")
                    .outline()
                    .small()
                    .label(SharedString::from("Continue with Google"));
                if google_available && !pending {
                    google = google.on_click(cx.listener(|this, _, _window, cx| {
                        this.start_google_sign_in(cx);
                    }));
                } else {
                    google = google.disabled(true);
                }
                google
            })
    }
}

fn avatar_circle(initials: &str, cx: &gpui::App) -> Div {
    div()
        .w(px(48.0))
        .h(px(48.0))
        .rounded_full()
        .bg(cx.theme().primary)
        .flex()
        .items_center()
        .justify_center()
        .text_color(cx.theme().primary_foreground)
        .text_lg()
        .font_weight(gpui::FontWeight::BOLD)
        .child(initials.to_string())
}

impl SettingsApp {
    pub(super) fn continue_as_guest(&mut self, cx: &mut gpui::Context<Self>) {
        let _ = self.shared.update_config(|config| {
            config.account.has_seen_welcome = true;
        });
        self.show_welcome = false;
        self.account_error = None;
        cx.notify();
    }

    pub(super) fn sign_out(&mut self, cx: &mut gpui::Context<Self>) {
        if let Some(account) = self.shared.config().account.current.clone() {
            let _ = account::clear_tokens(&account);
        }
        let _ = self.shared.update_config(|config| {
            config.account.current = None;
        });
        self.account_error = None;
        cx.notify();
    }

    pub(super) fn start_google_sign_in(&mut self, cx: &mut gpui::Context<Self>) {
        if self.account_sign_in_pending {
            return;
        }
        self.account_sign_in_pending = true;
        self.account_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let outcome = account::google::sign_in().await;
            let _ = this.update(cx, |view, cx| {
                view.account_sign_in_pending = false;
                match outcome {
                    Ok((account, tokens)) => view.apply_sign_in(account, tokens, cx),
                    Err(e) => {
                        view.account_error = Some(format!("{e}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(super) fn start_apple_sign_in(&mut self, cx: &mut gpui::Context<Self>) {
        if self.account_sign_in_pending {
            return;
        }
        if !account::apple::IS_AVAILABLE {
            self.account_error = Some(
                "Apple sign in is coming soon on macOS. Use Google or continue as guest."
                    .to_string(),
            );
            cx.notify();
            return;
        }
        self.account_sign_in_pending = true;
        self.account_error = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let outcome = account::apple::sign_in().await;
            let _ = this.update(cx, |view, cx| {
                view.account_sign_in_pending = false;
                match outcome {
                    Ok((account, tokens)) => view.apply_sign_in(account, tokens, cx),
                    Err(e) => {
                        view.account_error = Some(format!("{e}"));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn apply_sign_in(
        &mut self,
        account: Account,
        tokens: account::AccountTokens,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Err(e) = account::save_tokens(&account, &tokens) {
            self.account_error = Some(format!("Failed to store tokens: {e}"));
            cx.notify();
            return;
        }
        let _ = self.shared.update_config(|config| {
            config.account.current = Some(account.clone());
            config.account.has_seen_welcome = true;
        });
        self.show_welcome = false;
        self.active_pane = super::SettingsPane::Account;
        cx.notify();
    }
}

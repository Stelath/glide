use gpui::prelude::*;
use gpui::{Window, div, img, px};
use gpui_component::ActiveTheme;

use crate::state::SharedState;

pub(crate) struct AboutView {
    shared: SharedState,
}

impl AboutView {
    pub(crate) fn new(shared: SharedState) -> Self {
        Self { shared }
    }
}

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let accent = self.shared.config().app.accent;
        let icon_path = crate::platform::accent_icon_path(accent)
            .unwrap_or_else(|| crate::config::asset_path("assets/icons/logo.svg"));

        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(cx.theme().background)
            .gap(px(12.0))
            .child(img(icon_path).w(px(80.0)).h(px(80.0)))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child("Glide"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("Version {}", env!("CARGO_PKG_VERSION"))),
            )
    }
}

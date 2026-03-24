mod app;
mod audio;
mod config;
mod hotkey;
mod llm;
mod paste;
mod permissions;
mod pipeline;
mod stt;
mod ui;

use std::sync::Arc;

use gpui::{size, px, AppContext, Application, WindowBounds, WindowOptions};
use gpui_component::Root;
use tokio::runtime::Runtime;

use crate::{app::SharedAppState, config::GlideConfig};

fn main() {
    let config = GlideConfig::load_or_create().expect("failed to load config");
    let shared = Arc::new(SharedAppState::new(config));
    shared.refresh_input_devices();
    shared.set_permission_hint(permissions::macos_permission_hint());

    let runtime = Arc::new(Runtime::new().expect("failed to start async runtime"));

    // Pre-extract app icons in background so the picker popover opens instantly
    crate::config::preload_app_icons();

    // Fetch available models from provider APIs in background
    crate::config::fetch_all_models(&shared.snapshot().config.providers);

    let app = Application::new().with_assets(gpui_component_assets::Assets);
    app.run(move |cx| {
        gpui_component::init(cx);

        // Apply saved theme preference at startup
        ui::apply_theme_preference(shared.snapshot().config.app.theme, None, cx);

        hotkey::start_listener(shared.clone(), runtime);

        let shared_for_window = shared.clone();
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(
                    size(px(1000.0), px(650.0)),
                    cx,
                )),
                window_min_size: Some(size(px(700.0), px(450.0))),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title("Glide");
                let view = cx.new(|cx| ui::SettingsApp::new(shared_for_window, window, cx));
                let any_view: gpui::AnyView = view.into();
                cx.new(|cx| Root::new(any_view, window, cx))
            },
        )
        .unwrap();

        let _quit_sub = cx.on_window_closed(|cx| {
            cx.quit();
        });
    });
}

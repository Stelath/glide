use std::sync::Arc;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Application, Bounds, Global, WindowBounds,
    WindowOptions, point, px, size,
};
use gpui_component::Root;
use tokio::runtime::Runtime;

use crate::{
    actions::{self, ShellActionHandlers},
    config::GlideConfig,
    hotkey, menu, overlay, permissions,
    state::SharedAppState,
    ui,
};

/// Tracks the settings window so we can reopen/close it specifically.
struct SettingsWindowState {
    handle: Option<AnyWindowHandle>,
    shared: Arc<SharedAppState>,
}
impl Global for SettingsWindowState {}

fn ensure_settings_window(cx: &mut App) {
    let state = cx.global_mut::<SettingsWindowState>();
    // If the handle is still valid, just activate it
    if let Some(handle) = state.handle
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    // Otherwise create a new one
    let shared = cx.global::<SettingsWindowState>().shared.clone();
    let shared_for_view = shared.clone();
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1000.0), px(650.0)), cx)),
            window_min_size: Some(size(px(700.0), px(450.0))),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title("Glide");
            let view = cx.new(|cx| ui::SettingsApp::new(shared_for_view, window, cx));
            let any_view: gpui::AnyView = view.into();
            cx.new(|cx| Root::new(any_view, window, cx))
        },
    );
    if let Ok(h) = handle {
        cx.global_mut::<SettingsWindowState>().handle = Some(h.into());
    }
}

fn open_about_window(cx: &mut App) {
    let shared = cx.global::<SettingsWindowState>().shared.clone();
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(240.0), px(200.0)),
            ))),
            is_resizable: false,
            ..Default::default()
        },
        move |window, cx| {
            window.set_window_title("About Glide");
            cx.new(move |cx| {
                let view = cx.new(move |_| ui::AboutView::new(shared));
                let any_view: gpui::AnyView = view.into();
                Root::new(any_view, window, cx)
            })
        },
    );
}

fn close_settings_window(cx: &mut App) {
    let handle = cx.global::<SettingsWindowState>().handle;
    if let Some(handle) = handle {
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            cx.global_mut::<SettingsWindowState>().handle = None;
        });
    }
}

pub fn run() {
    permissions::request_accessibility_access_once();

    let config = GlideConfig::load_or_create().expect("failed to load config");
    let shared = Arc::new(SharedAppState::new(config));
    shared.refresh_input_devices();
    shared.set_permission_hint(permissions::macos_permission_hint());

    let runtime = Arc::new(Runtime::new().expect("failed to start async runtime"));
    crate::prewarm::start_app_prewarm(shared.clone(), runtime.clone());

    crate::platform::preload_app_icons();
    crate::model_catalog::fetch_all_models(&shared.snapshot().config.providers);

    let app = Application::new().with_assets(gpui_component_assets::Assets);

    // Reopen settings window when dock icon clicked with no windows visible
    app.on_reopen(move |cx| {
        ensure_settings_window(cx);
    });

    app.run(move |cx| {
        gpui_component::init(cx);
        cx.activate(true);

        // Store the shared state globally so ensure_settings_window can access it
        cx.set_global(SettingsWindowState {
            handle: None,
            shared: shared.clone(),
        });
        // Apply saved theme preference at startup
        let snap = shared.snapshot();
        ui::apply_theme_preference(snap.config.app.theme, snap.config.app.accent, None, cx);

        // --- Actions, key bindings, and menu bar ---
        actions::register(
            cx,
            ShellActionHandlers {
                close_window: close_settings_window,
                show_settings: ensure_settings_window,
                show_about: open_about_window,
            },
        );
        actions::bind_keybindings(cx);
        menu::install(cx);

        // --- Background services ---
        hotkey::start_listener(shared.clone(), runtime);

        let overlay_shared = shared.clone();
        let overlay_entity = cx.new(|cx| {
            let controller = overlay::OverlayController::new(overlay_shared);
            controller.start_polling(cx);
            controller
        });
        cx.set_global(overlay::OverlayHandle(overlay_entity));

        // --- Open initial settings window ---
        ensure_settings_window(cx);
    });
}

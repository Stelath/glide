mod app;
mod audio;
mod config;
mod hotkey;
mod llm;
mod overlay;
mod paste;
mod permissions;
mod pipeline;
mod stt;
mod ui;

use std::sync::Arc;

use gpui::{
    actions, div, img, size, px, AnyWindowHandle, App, AppContext, Application, Bounds, Context,
    Global, IntoElement, KeyBinding, Menu, MenuItem, OsAction, ParentElement, Render,
    Styled, SystemMenuType, Window, WindowBounds, WindowOptions,
    point,
};
use gpui_component::{ActiveTheme, Root};
use tokio::runtime::Runtime;

use crate::{app::SharedAppState, config::GlideConfig};

actions!(
    glide,
    [Quit, CloseWindow, ShowSettings, ShowAbout, Minimize, Zoom, Undo, Redo, Cut, Copy, Paste, SelectAll]
);

/// Tracks the settings window so we can reopen/close it specifically.
struct SettingsWindowState {
    handle: Option<AnyWindowHandle>,
    shared: Arc<SharedAppState>,
}
impl Global for SettingsWindowState {}

fn ensure_settings_window(cx: &mut App) {
    let state = cx.global_mut::<SettingsWindowState>();
    // If the handle is still valid, just activate it
    if let Some(handle) = state.handle {
        if handle.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
    }
    // Otherwise create a new one
    let shared = cx.global::<SettingsWindowState>().shared.clone();
    let shared_for_view = shared.clone();
    let handle = cx.open_window(
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
            let view = cx.new(|cx| ui::SettingsApp::new(shared_for_view, window, cx));
            let any_view: gpui::AnyView = view.into();
            cx.new(|cx| Root::new(any_view, window, cx))
        },
    );
    if let Ok(h) = handle {
        cx.global_mut::<SettingsWindowState>().handle = Some(h.into());
    }
}

struct AboutView;

impl Render for AboutView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .bg(cx.theme().background)
            .gap(px(12.0))
            .child(
                img(crate::config::asset_path("assets/icons/logo.svg"))
                    .w(px(80.0))
                    .h(px(80.0)),
            )
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

fn open_about_window(cx: &mut App) {
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(0.0), px(0.0)),
                size(px(240.0), px(200.0)),
            ))),
            is_resizable: false,
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title("About Glide");
            cx.new(|cx| {
                let view = cx.new(|_| AboutView);
                let any_view: gpui::AnyView = view.into();
                Root::new(any_view, window, cx)
            })
        },
    );
}

fn main() {
    let config = GlideConfig::load_or_create().expect("failed to load config");
    let shared = Arc::new(SharedAppState::new(config));
    shared.refresh_input_devices();
    shared.set_permission_hint(permissions::macos_permission_hint());

    let runtime = Arc::new(Runtime::new().expect("failed to start async runtime"));

    crate::config::preload_app_icons();
    crate::config::fetch_all_models(&shared.snapshot().config.providers);

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

        // Apply saved theme preference and dock icon at startup
        let snap = shared.snapshot();
        ui::apply_theme_preference(snap.config.app.theme, snap.config.app.accent, None, cx);
        crate::config::set_dock_icon(snap.config.app.accent);

        // --- Actions ---
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &CloseWindow, cx| {
            let handle = cx.global::<SettingsWindowState>().handle;
            if let Some(handle) = handle {
                // Defer so the removal happens after the current dispatch cycle.
                // Don't clear the handle yet — ensure_settings_window checks
                // validity via handle.update(), which will fail once the
                // deferred removal is processed.
                cx.defer(move |cx| {
                    let _ = handle.update(cx, |_, window, _| window.remove_window());
                    cx.global_mut::<SettingsWindowState>().handle = None;
                });
            }
        });
        cx.on_action(|_: &ShowSettings, cx| ensure_settings_window(cx));
        cx.on_action(|_: &ShowAbout, cx| open_about_window(cx));
        cx.on_action(|_: &Minimize, cx| {
            if let Some(stack) = cx.window_stack() {
                if let Some(window) = stack.into_iter().next() {
                    let _ = window.update(cx, |_, window, _| window.minimize_window());
                }
            }
        });
        cx.on_action(|_: &Zoom, cx| {
            if let Some(stack) = cx.window_stack() {
                if let Some(window) = stack.into_iter().next() {
                    let _ = window.update(cx, |_, window, _| window.zoom_window());
                }
            }
        });

        // --- Key bindings ---
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-w", CloseWindow, None),
            KeyBinding::new("cmd-m", Minimize, None),
        ]);

        // --- Menu bar ---
        cx.set_menus(vec![
            Menu {
                name: "Glide".into(),
                items: vec![
                    MenuItem::action("About Glide", ShowAbout),
                    MenuItem::separator(),
                    MenuItem::os_submenu("Services", SystemMenuType::Services),
                    MenuItem::separator(),
                    MenuItem::action("Quit Glide", Quit),
                ],
            },
            Menu {
                name: "File".into(),
                items: vec![
                    MenuItem::action("Close Window", CloseWindow),
                ],
            },
            Menu {
                name: "Edit".into(),
                items: vec![
                    MenuItem::os_action("Undo", Undo, OsAction::Undo),
                    MenuItem::os_action("Redo", Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::os_action("Cut", Cut, OsAction::Cut),
                    MenuItem::os_action("Copy", Copy, OsAction::Copy),
                    MenuItem::os_action("Paste", Paste, OsAction::Paste),
                    MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                ],
            },
            Menu {
                name: "Window".into(),
                items: vec![
                    MenuItem::action("Minimize", Minimize),
                    MenuItem::action("Zoom", Zoom),
                ],
            },
        ]);

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

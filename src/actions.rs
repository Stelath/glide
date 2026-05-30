use std::sync::Arc;

use gpui::{
    AnyWindowHandle, App, AppContext as _, Bounds, Global, KeyBinding, Window, WindowBounds,
    WindowOptions, actions, point, px, size,
};
use gpui_component::Root;

use crate::{state::SharedAppState, ui};

actions!(
    glide,
    [
        Quit,
        CloseWindow,
        ShowSettings,
        ShowAbout,
        Minimize,
        Zoom,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll
    ]
);

/// Tracks the settings window so we can reopen/close it specifically.
// GPUI/macOS does not give this window a stable app-level identity for us.
struct SettingsWindowState {
    handle: Option<AnyWindowHandle>,
    shared: Arc<SharedAppState>,
}

impl Global for SettingsWindowState {}

pub(crate) fn init(cx: &mut App, shared: Arc<SharedAppState>) {
    cx.set_global(SettingsWindowState {
        handle: None,
        shared,
    });
}

pub(crate) fn register(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &CloseWindow, cx| close_settings_window(cx));
    cx.on_action(|_: &ShowSettings, cx| ensure_settings_window(cx));
    cx.on_action(|_: &ShowAbout, cx| open_about_window(cx));
    cx.on_action(|_: &Minimize, cx| minimize_active_window(cx));
    cx.on_action(|_: &Zoom, cx| zoom_active_window(cx));
}

pub(crate) fn bind_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
    ]);
}

fn minimize_active_window(cx: &mut App) {
    update_active_window(cx, Window::minimize_window);
}

fn zoom_active_window(cx: &mut App) {
    update_active_window(cx, Window::zoom_window);
}

pub(crate) fn ensure_settings_window(cx: &mut App) {
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

fn update_active_window(cx: &mut App, update: impl FnOnce(&Window)) {
    if let Some(stack) = cx.window_stack()
        && let Some(window) = stack.into_iter().next()
    {
        let _ = window.update(cx, |_, window, _| update(window));
    }
}

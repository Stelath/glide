use gpui::{App, KeyBinding, actions};

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

pub(crate) struct ShellActionHandlers {
    pub close_window: fn(&mut App),
    pub show_settings: fn(&mut App),
    pub show_about: fn(&mut App),
}

pub(crate) fn register(cx: &mut App, handlers: ShellActionHandlers) {
    let close_window = handlers.close_window;
    let show_settings = handlers.show_settings;
    let show_about = handlers.show_about;

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(move |_: &CloseWindow, cx| close_window(cx));
    cx.on_action(move |_: &ShowSettings, cx| show_settings(cx));
    cx.on_action(move |_: &ShowAbout, cx| show_about(cx));
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
    if let Some(stack) = cx.window_stack()
        && let Some(window) = stack.into_iter().next()
    {
        let _ = window.update(cx, |_, window, _| window.minimize_window());
    }
}

fn zoom_active_window(cx: &mut App) {
    if let Some(stack) = cx.window_stack()
        && let Some(window) = stack.into_iter().next()
    {
        let _ = window.update(cx, |_, window, _| window.zoom_window());
    }
}

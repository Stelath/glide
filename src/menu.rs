use gpui::{App, Menu, MenuItem, OsAction, SystemMenuType};

use crate::actions::{
    CloseWindow, Copy, Cut, Minimize, Paste, Quit, Redo, SelectAll, ShowAbout, Undo, Zoom,
};

pub(crate) fn install(cx: &mut App) {
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
            items: vec![MenuItem::action("Close Window", CloseWindow)],
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
}

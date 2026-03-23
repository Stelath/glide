use std::{thread, time::Duration};

use anyhow::{Context, Result};
use arboard::Clipboard;
use enigo::{
    Direction::{Click, Press, Release},
    Enigo, Key, Keyboard, Settings,
};

use crate::config::PasteConfig;

pub fn paste_text(text: &str, config: &PasteConfig) -> Result<()> {
    let mut clipboard = Clipboard::new().context("failed to access clipboard")?;
    let previous_text = if config.restore_clipboard {
        clipboard.get_text().ok()
    } else {
        None
    };

    clipboard
        .set_text(text.to_string())
        .context("failed to update clipboard")?;

    let mut enigo =
        Enigo::new(&Settings::default()).context("failed to initialize keyboard driver")?;
    enigo
        .key(Key::Meta, Press)
        .context("failed to press command key")?;
    enigo
        .key(Key::Unicode('v'), Click)
        .context("failed to send paste shortcut")?;
    enigo
        .key(Key::Meta, Release)
        .context("failed to release command key")?;

    if let Some(previous_text) = previous_text {
        thread::sleep(Duration::from_millis(config.restore_delay_ms));
        let mut clipboard = Clipboard::new().context("failed to re-open clipboard")?;
        clipboard
            .set_text(previous_text)
            .context("failed to restore clipboard contents")?;
    }

    Ok(())
}

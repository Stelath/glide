use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use arboard::Clipboard;

use crate::config::PasteConfig;

// --- CoreGraphics FFI for keyboard event simulation ---

const CLIPBOARD_SETTLE_DELAY_MS: u64 = 50;
const MIN_RESTORE_DELAY_MS: u64 = 750;

type CGEventRef = *mut std::ffi::c_void;
type CGEventSourceRef = *mut std::ffi::c_void;

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_EVENT_FLAG_MASK_COMMAND: u64 = 0x00100000;
// macOS virtual keycode for 'V'
const KVK_V: u16 = 9;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(
        source: CGEventSourceRef,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const std::ffi::c_void);
}

/// Simulate Cmd+V using CoreGraphics keyboard events.
fn simulate_paste() {
    unsafe {
        // Key down: Cmd+V
        let key_down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KVK_V, true);
        CGEventSetFlags(key_down, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_down);
        CFRelease(key_down);

        // Key up: release V
        let key_up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KVK_V, false);
        CGEventSetFlags(key_up, K_CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(K_CG_HID_EVENT_TAP, key_up);
        CFRelease(key_up);
    }
}

fn restore_delay(config: &PasteConfig) -> Duration {
    Duration::from_millis(config.restore_delay_ms.max(MIN_RESTORE_DELAY_MS))
}

fn spawn_delayed_restore<F>(delay: Duration, restore: F) -> thread::JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(move || {
        thread::sleep(delay);
        restore();
    })
}

fn restore_clipboard_async(previous_text: String, delay: Duration) {
    let scheduled = Instant::now();
    spawn_delayed_restore(delay, move || {
        let result = (|| -> Result<()> {
            let mut clipboard = Clipboard::new().context("failed to re-open clipboard")?;
            clipboard
                .set_text(previous_text)
                .context("failed to restore clipboard contents")?;
            Ok(())
        })();

        match result {
            Ok(()) => eprintln!(
                "[glide] Paste: restored clipboard after {} ms",
                scheduled.elapsed().as_millis()
            ),
            Err(error) => eprintln!("[glide] Paste: failed to restore clipboard: {error:#}"),
        }
    });
}

pub fn paste_text(text: &str, config: &PasteConfig) -> Result<()> {
    anyhow::ensure!(
        crate::permissions::has_accessibility_access(),
        "Accessibility permission required for paste. Grant access in \
         System Settings > Privacy & Security > Accessibility and relaunch."
    );

    let mut clipboard = Clipboard::new().context("failed to access clipboard")?;
    let previous_text = if config.restore_clipboard {
        clipboard.get_text().ok()
    } else {
        None
    };

    clipboard
        .set_text(text.to_string())
        .context("failed to update clipboard")?;

    thread::sleep(Duration::from_millis(CLIPBOARD_SETTLE_DELAY_MS));
    simulate_paste();

    if let Some(previous_text) = previous_text {
        restore_clipboard_async(previous_text, restore_delay(config));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restore_delay_preserves_safe_values_and_clamps_unsafe_values() {
        let cases = [
            (1_000, Duration::from_millis(1_000)),
            (100, Duration::from_millis(MIN_RESTORE_DELAY_MS)),
        ];

        for (restore_delay_ms, expected) in cases {
            let config = PasteConfig {
                restore_clipboard: true,
                restore_delay_ms,
            };
            assert_eq!(restore_delay(&config), expected);
        }
    }

    #[test]
    fn delayed_restore_returns_without_waiting_for_delay() {
        let delay = Duration::from_millis(100);
        let started = Instant::now();
        let handle = spawn_delayed_restore(delay, || {});
        assert!(started.elapsed() < delay / 2);
        handle.join().unwrap();
    }
}

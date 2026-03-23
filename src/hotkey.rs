use std::{sync::Arc, thread};

use tokio::runtime::Runtime;

use crate::{
    app::{RuntimeStatus, SharedState},
    audio::AudioRecorder,
    config::HotkeyTrigger,
    pipeline,
};

// macOS virtual keycodes for the trigger keys we care about.
mod keycode {
    pub const OPTION_LEFT: u16 = 58;
    pub const OPTION_RIGHT: u16 = 61;
    pub const COMMAND_RIGHT: u16 = 54;
    pub const F8: u16 = 100;
    pub const F9: u16 = 101;
    pub const F10: u16 = 109;
}

// CGEvent flag masks for detecting modifier press vs release.
mod flags {
    pub const ALTERNATE: u64 = 0x00080000; // NSEventModifierFlagOption
    pub const COMMAND: u64 = 0x00100000; // NSEventModifierFlagCommand
}

// --- Minimal CoreGraphics / CoreFoundation FFI -----------------------------------

#[allow(non_upper_case_globals)]
mod ffi {
    use std::ffi::c_void;

    pub type CFMachPortRef = *mut c_void;
    pub type CFRunLoopSourceRef = *mut c_void;
    pub type CFRunLoopRef = *mut c_void;
    pub type CGEventTapProxy = *mut c_void;
    pub type CGEventRef = *mut c_void;
    pub type CFAllocatorRef = *const c_void;
    pub type CFIndex = isize;
    pub type CFStringRef = *const c_void;

    pub const kCFAllocatorDefault: CFAllocatorRef = std::ptr::null();
    // CGEventTapLocation
    pub const kCGSessionEventTap: u32 = 1;

    // CGEventTapPlacement
    pub const kCGHeadInsertEventTap: u32 = 0;

    // CGEventTapOptions
    pub const kCGEventTapOptionListenOnly: u32 = 1;

    // CGEventType
    pub const kCGEventKeyDown: u32 = 10;
    pub const kCGEventKeyUp: u32 = 11;
    pub const kCGEventFlagsChanged: u32 = 12;

    // CGEventField
    pub const kCGKeyboardEventKeycode: u32 = 9;

    // CGEventMask helpers
    pub const fn cg_event_mask_bit(event_type: u32) -> u64 {
        1u64 << event_type
    }

    pub type CGEventTapCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: u32,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    unsafe extern "C" {
        // CoreGraphics
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;

        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventGetFlags(event: CGEventRef) -> u64;

        // CoreFoundation
        pub fn CFMachPortCreateRunLoopSource(
            allocator: CFAllocatorRef,
            port: CFMachPortRef,
            order: CFIndex,
        ) -> CFRunLoopSourceRef;

        pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
        pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
        pub fn CFRunLoopRun();
        pub fn CFRelease(cf: *const c_void);

        // We need the real symbol for kCFRunLoopCommonModes.
        #[link_name = "kCFRunLoopCommonModes"]
        pub static kCFRunLoopCommonModes_real: CFStringRef;
    }
}

// State passed through CGEventTap user_info pointer.
struct TapContext {
    shared: SharedState,
    runtime: Arc<Runtime>,
    recorder: AudioRecorder,
    pressed: bool,
}

pub fn start_listener(shared: SharedState, runtime: Arc<Runtime>) {
    thread::spawn(move || {
        shared.set_status(RuntimeStatus::Idle, "Ready for dictation");

        let ctx = Box::new(TapContext {
            shared: shared.clone(),
            runtime,
            recorder: AudioRecorder::new(),
            pressed: false,
        });
        let ctx_ptr = Box::into_raw(ctx) as *mut std::ffi::c_void;

        let event_mask = ffi::cg_event_mask_bit(ffi::kCGEventKeyDown)
            | ffi::cg_event_mask_bit(ffi::kCGEventKeyUp)
            | ffi::cg_event_mask_bit(ffi::kCGEventFlagsChanged);

        unsafe {
            let tap = ffi::CGEventTapCreate(
                ffi::kCGSessionEventTap,
                ffi::kCGHeadInsertEventTap,
                ffi::kCGEventTapOptionListenOnly,
                event_mask,
                event_tap_callback,
                ctx_ptr,
            );

            if tap.is_null() {
                let ctx = Box::from_raw(ctx_ptr as *mut TapContext);
                ctx.shared.set_error(
                    "Failed to create event tap. Grant Accessibility permission in \
                     System Settings > Privacy & Security > Accessibility and relaunch.",
                );
                return;
            }

            let source =
                ffi::CFMachPortCreateRunLoopSource(ffi::kCFAllocatorDefault, tap, 0);
            let run_loop = ffi::CFRunLoopGetCurrent();
            ffi::CFRunLoopAddSource(run_loop, source, ffi::kCFRunLoopCommonModes_real);

            // This blocks forever (like rdev::listen did).
            ffi::CFRunLoopRun();

            ffi::CFRelease(source);
            ffi::CFRelease(tap);
            let _ = Box::from_raw(ctx_ptr as *mut TapContext);
        }
    });
}

unsafe extern "C" fn event_tap_callback(
    _proxy: ffi::CGEventTapProxy,
    event_type: u32,
    event: ffi::CGEventRef,
    user_info: *mut std::ffi::c_void,
) -> ffi::CGEventRef {
    let ctx = unsafe { &mut *(user_info as *mut TapContext) };
    let keycode = unsafe { ffi::CGEventGetIntegerValueField(event, ffi::kCGKeyboardEventKeycode) } as u16;
    let trigger = ctx.shared.config().hotkey.trigger;

    match event_type {
        ffi::kCGEventKeyDown => {
            if is_trigger_keycode(trigger, keycode) && !ctx.pressed {
                handle_press(ctx);
            }
        }
        ffi::kCGEventKeyUp => {
            if is_trigger_keycode(trigger, keycode) && ctx.pressed {
                handle_release(ctx);
            }
        }
        ffi::kCGEventFlagsChanged => {
            if is_trigger_keycode(trigger, keycode) {
                let event_flags = unsafe { ffi::CGEventGetFlags(event) };
                let is_down = modifier_is_pressed(trigger, event_flags);
                if is_down && !ctx.pressed {
                    handle_press(ctx);
                } else if !is_down && ctx.pressed {
                    handle_release(ctx);
                }
            }
        }
        _ => {}
    }

    event
}

fn handle_press(ctx: &mut TapContext) {
    ctx.pressed = true;
    if matches!(ctx.shared.snapshot().status, RuntimeStatus::Processing) {
        return;
    }

    let config = ctx.shared.config();
    match ctx.recorder.start(&config.audio) {
        Ok(()) => ctx.shared.set_status(
            RuntimeStatus::Recording,
            "Listening... release hotkey to transcribe",
        ),
        Err(error) => {
            ctx.pressed = false;
            ctx.shared
                .set_error(format!("failed to start recording: {error:#}"));
        }
    }
}

fn handle_release(ctx: &mut TapContext) {
    ctx.pressed = false;

    match ctx.recorder.stop() {
        Ok(audio) => {
            ctx.shared
                .set_status(RuntimeStatus::Processing, "Uploading audio");
            let shared_clone = ctx.shared.clone();
            ctx.runtime.spawn(async move {
                if let Err(error) =
                    pipeline::process_recording(shared_clone.clone(), audio).await
                {
                    shared_clone.set_error(error.to_string());
                }
            });
        }
        Err(error) => {
            ctx.shared
                .set_error(format!("failed to finish recording: {error:#}"));
        }
    }
}

fn is_trigger_keycode(trigger: HotkeyTrigger, keycode: u16) -> bool {
    match trigger {
        HotkeyTrigger::Option => {
            matches!(keycode, keycode::OPTION_LEFT | keycode::OPTION_RIGHT)
        }
        HotkeyTrigger::CommandRight => keycode == keycode::COMMAND_RIGHT,
        HotkeyTrigger::F8 => keycode == keycode::F8,
        HotkeyTrigger::F9 => keycode == keycode::F9,
        HotkeyTrigger::F10 => keycode == keycode::F10,
        HotkeyTrigger::Custom(code) => keycode == code,
    }
}

fn modifier_is_pressed(trigger: HotkeyTrigger, event_flags: u64) -> bool {
    match trigger {
        HotkeyTrigger::Option => event_flags & flags::ALTERNATE != 0,
        HotkeyTrigger::CommandRight => event_flags & flags::COMMAND != 0,
        // F-keys and custom keys are not modifiers; they use keyDown/keyUp, not flagsChanged.
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_option_maps_to_alt_keycodes() {
        assert!(is_trigger_keycode(HotkeyTrigger::Option, keycode::OPTION_LEFT));
        assert!(is_trigger_keycode(HotkeyTrigger::Option, keycode::OPTION_RIGHT));
    }

    #[test]
    fn test_command_right_maps() {
        assert!(is_trigger_keycode(HotkeyTrigger::CommandRight, keycode::COMMAND_RIGHT));
    }

    #[test]
    fn test_f8_maps() {
        assert!(is_trigger_keycode(HotkeyTrigger::F8, keycode::F8));
    }

    #[test]
    fn test_f9_maps() {
        assert!(is_trigger_keycode(HotkeyTrigger::F9, keycode::F9));
    }

    #[test]
    fn test_f10_maps() {
        assert!(is_trigger_keycode(HotkeyTrigger::F10, keycode::F10));
    }

    #[test]
    fn test_wrong_key_returns_false() {
        assert!(!is_trigger_keycode(HotkeyTrigger::F8, keycode::F9));
        assert!(!is_trigger_keycode(HotkeyTrigger::Option, keycode::COMMAND_RIGHT));
        assert!(!is_trigger_keycode(HotkeyTrigger::CommandRight, keycode::OPTION_LEFT));
    }

    #[test]
    fn test_modifier_pressed_option() {
        assert!(modifier_is_pressed(HotkeyTrigger::Option, flags::ALTERNATE));
        assert!(!modifier_is_pressed(HotkeyTrigger::Option, 0));
        assert!(!modifier_is_pressed(HotkeyTrigger::Option, flags::COMMAND));
    }

    #[test]
    fn test_modifier_pressed_command_right() {
        assert!(modifier_is_pressed(HotkeyTrigger::CommandRight, flags::COMMAND));
        assert!(!modifier_is_pressed(HotkeyTrigger::CommandRight, 0));
    }

    #[test]
    fn test_modifier_pressed_fkeys_always_false() {
        assert!(!modifier_is_pressed(HotkeyTrigger::F8, flags::ALTERNATE));
        assert!(!modifier_is_pressed(HotkeyTrigger::F9, flags::COMMAND));
        assert!(!modifier_is_pressed(HotkeyTrigger::F10, 0xFFFFFFFF));
    }
}

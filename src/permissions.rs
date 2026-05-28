// --- macOS permission checking via FFI ---
use std::ffi::{c_char, c_void};
use std::sync::Once;

// Accessibility: ApplicationServices framework
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *const c_void;
    fn CFRelease(cf: *const c_void);
}

static ACCESSIBILITY_PROMPT: Once = Once::new();

// Input Monitoring: CoreGraphics framework (macOS 10.15+)
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
}

#[cfg(target_os = "macos")]
mod microphone {
    use std::{sync::mpsc, time::Duration};

    use block2::RcBlock;
    use objc2::runtime::Bool;

    use super::*;

    #[link(name = "AVFoundation", kind = "framework")]
    unsafe extern "C" {
        static AVMediaTypeAudio: *const c_void;
    }

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void;
    }

    pub(super) fn authorization_status() -> MicrophoneAuthorizationStatus {
        type MsgSendStatus = unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void) -> isize;

        unsafe {
            let class = objc_getClass(c"AVCaptureDevice".as_ptr());
            if class.is_null() {
                return MicrophoneAuthorizationStatus::Unknown(-1);
            }

            let selector = sel_registerName(c"authorizationStatusForMediaType:".as_ptr());
            let msg: MsgSendStatus = std::mem::transmute(objc_msgSend as *const ());
            MicrophoneAuthorizationStatus::from_raw(msg(class, selector, AVMediaTypeAudio))
        }
    }

    pub(super) fn request_access() -> MicrophoneAuthorizationStatus {
        let status = authorization_status();
        if status != MicrophoneAuthorizationStatus::NotDetermined {
            return status;
        }

        type MsgSendRequest =
            unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_void, *mut c_void);

        let (tx, rx) = mpsc::channel();
        let completion: RcBlock<dyn Fn(Bool)> = RcBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });

        unsafe {
            let class = objc_getClass(c"AVCaptureDevice".as_ptr());
            if class.is_null() {
                return MicrophoneAuthorizationStatus::Unknown(-1);
            }

            let selector =
                sel_registerName(c"requestAccessForMediaType:completionHandler:".as_ptr());
            let msg: MsgSendRequest = std::mem::transmute(objc_msgSend as *const ());
            msg(
                class,
                selector,
                AVMediaTypeAudio,
                RcBlock::as_ptr(&completion).cast(),
            );
        }

        match rx.recv_timeout(Duration::from_secs(60)) {
            Ok(true) => MicrophoneAuthorizationStatus::Authorized,
            Ok(false) => authorization_status(),
            Err(_) => authorization_status(),
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod microphone {
    use super::*;

    pub(super) fn authorization_status() -> MicrophoneAuthorizationStatus {
        if cpal_microphone_access() {
            MicrophoneAuthorizationStatus::Authorized
        } else {
            MicrophoneAuthorizationStatus::Unknown(-1)
        }
    }

    pub(super) fn request_access() -> MicrophoneAuthorizationStatus {
        authorization_status()
    }
}

pub const MICROPHONE_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicrophoneAuthorizationStatus {
    NotDetermined,
    Restricted,
    Denied,
    Authorized,
    Unknown(isize),
}

impl MicrophoneAuthorizationStatus {
    fn from_raw(value: isize) -> Self {
        match value {
            0 => Self::NotDetermined,
            1 => Self::Restricted,
            2 => Self::Denied,
            3 => Self::Authorized,
            other => Self::Unknown(other),
        }
    }

    pub fn is_authorized(self) -> bool {
        matches!(self, Self::Authorized)
    }

    pub fn is_denied_or_restricted(self) -> bool {
        matches!(self, Self::Denied | Self::Restricted)
    }

    pub fn can_capture(self) -> bool {
        matches!(self, Self::Authorized)
    }
}

/// Check if the app has Accessibility permission (needed for simulated paste via enigo).
pub fn has_accessibility_access() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Ask macOS to prompt/register this app in Privacy & Security > Accessibility.
pub fn request_accessibility_access() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            std::ptr::null(),
            std::ptr::null(),
        );
        if options.is_null() {
            return AXIsProcessTrusted();
        }
        let trusted = AXIsProcessTrustedWithOptions(options);
        CFRelease(options);
        trusted
    }
}

pub fn request_accessibility_access_once() {
    ACCESSIBILITY_PROMPT.call_once(|| {
        let _ = request_accessibility_access();
    });
}

/// Check if the app has Input Monitoring permission (needed for global hotkey via CGEventTap).
pub fn has_input_monitoring_access() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}

/// Check if the app has Microphone permission (needed for audio capture via cpal).
pub fn has_microphone_access() -> bool {
    microphone_authorization_status().is_authorized()
}

pub fn microphone_authorization_status() -> MicrophoneAuthorizationStatus {
    microphone::authorization_status()
}

pub fn request_microphone_access() -> MicrophoneAuthorizationStatus {
    microphone::request_access()
}

pub fn microphone_access_error(status: MicrophoneAuthorizationStatus) -> Option<String> {
    match status {
        MicrophoneAuthorizationStatus::NotDetermined => Some(
            "Microphone access has not been granted. Choose Allow when macOS asks for microphone access, then try dictation again.".to_string(),
        ),
        MicrophoneAuthorizationStatus::Denied => Some(
            "Microphone access is denied. Enable Glide in System Settings > Privacy & Security > Microphone, then try again.".to_string(),
        ),
        MicrophoneAuthorizationStatus::Restricted => Some(
            "Microphone access is restricted by macOS policy. Enable microphone access for Glide in System Settings if available.".to_string(),
        ),
        MicrophoneAuthorizationStatus::Unknown(_) => Some(
            "Glide could not determine microphone permission. Enable microphone access in System Settings > Privacy & Security > Microphone, then try again.".to_string(),
        ),
        _ => None,
    }
}

pub fn open_microphone_settings() {
    let _ = std::process::Command::new("open")
        .arg(MICROPHONE_SETTINGS_URL)
        .spawn();
}

#[cfg(not(target_os = "macos"))]
fn cpal_microphone_access() -> bool {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(device) => device.default_input_config().is_ok(),
        None => false,
    }
}

/// Permission status for display in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionStatus {
    pub name: &'static str,
    pub description: &'static str,
    pub granted: bool,
    pub settings_url: &'static str,
    /// Icon name string matching gpui_component::IconName variants.
    pub icon: &'static str,
}

/// Check all required permissions and return their statuses.
pub fn check_all() -> Vec<PermissionStatus> {
    vec![
        PermissionStatus {
            name: "Microphone",
            description: "Capture audio for dictation",
            granted: has_microphone_access(),
            settings_url: MICROPHONE_SETTINGS_URL,
            icon: "bell", // closest to audio/mic in the icon pack
        },
        PermissionStatus {
            name: "Accessibility",
            description: "Paste transcribed text",
            granted: has_accessibility_access(),
            settings_url: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            icon: "user",
        },
        PermissionStatus {
            name: "Input Monitoring",
            description: "Global hotkey detection",
            granted: has_input_monitoring_access(),
            settings_url: "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
            icon: "eye",
        },
    ]
}

pub fn macos_permission_hint() -> String {
    "macOS needs Microphone, Accessibility, and Input Monitoring access for Glide to capture audio, listen globally, and paste text. If dictation does nothing, grant those permissions in System Settings > Privacy & Security and relaunch the app.".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hint_mentions_required_permissions() {
        let hint = macos_permission_hint();
        assert!(hint.contains("Microphone"));
        assert!(hint.contains("Accessibility"));
        assert!(hint.contains("Input Monitoring"));
    }

    #[test]
    fn test_check_all_returns_three_permissions() {
        let statuses = check_all();
        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].name, "Microphone");
        assert_eq!(statuses[1].name, "Accessibility");
        assert_eq!(statuses[2].name, "Input Monitoring");
    }

    #[test]
    fn test_microphone_authorization_status_mapping() {
        assert_eq!(
            MicrophoneAuthorizationStatus::from_raw(0),
            MicrophoneAuthorizationStatus::NotDetermined
        );
        assert_eq!(
            MicrophoneAuthorizationStatus::from_raw(1),
            MicrophoneAuthorizationStatus::Restricted
        );
        assert_eq!(
            MicrophoneAuthorizationStatus::from_raw(2),
            MicrophoneAuthorizationStatus::Denied
        );
        assert_eq!(
            MicrophoneAuthorizationStatus::from_raw(3),
            MicrophoneAuthorizationStatus::Authorized
        );
        assert_eq!(
            MicrophoneAuthorizationStatus::from_raw(99),
            MicrophoneAuthorizationStatus::Unknown(99)
        );
    }

    #[test]
    fn test_microphone_denied_status_has_actionable_error() {
        let message = microphone_access_error(MicrophoneAuthorizationStatus::Denied)
            .expect("denied status should produce a message");
        assert!(message.contains("Microphone access is denied"));
        assert!(message.contains("System Settings"));
    }

    #[test]
    fn test_microphone_not_determined_has_prompt_error() {
        let message = microphone_access_error(MicrophoneAuthorizationStatus::NotDetermined)
            .expect("not determined status should produce a message");
        assert!(message.contains("Choose Allow"));
    }
}

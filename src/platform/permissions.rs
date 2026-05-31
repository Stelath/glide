use std::ffi::{c_char, c_void};

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
pub const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const INPUT_MONITORING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";

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

/// Check if the app has Accessibility permission (needed for simulated paste via CoreGraphics).
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
    open_settings_url(MICROPHONE_SETTINGS_URL);
}

pub fn open_accessibility_settings() {
    open_settings_url(ACCESSIBILITY_SETTINGS_URL);
}

pub fn request_accessibility_access_or_open_settings() -> bool {
    if has_accessibility_access() || request_accessibility_access() {
        return true;
    }

    open_accessibility_settings();
    false
}

fn open_settings_url(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
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
        permission_status(
            "Microphone",
            "Capture audio for dictation",
            has_microphone_access(),
            MICROPHONE_SETTINGS_URL,
            "bell",
        ),
        permission_status(
            "Accessibility",
            "Paste transcribed text",
            has_accessibility_access(),
            ACCESSIBILITY_SETTINGS_URL,
            "user",
        ),
        permission_status(
            "Input Monitoring",
            "Global hotkey detection",
            has_input_monitoring_access(),
            INPUT_MONITORING_SETTINGS_URL,
            "eye",
        ),
    ]
}

fn permission_status(
    name: &'static str,
    description: &'static str,
    granted: bool,
    settings_url: &'static str,
    icon: &'static str,
) -> PermissionStatus {
    PermissionStatus {
        name,
        description,
        granted,
        settings_url,
        icon,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_all_returns_expected_permissions() {
        let statuses = check_all();
        let names = statuses
            .iter()
            .map(|status| status.name)
            .collect::<Vec<_>>();
        assert_eq!(names, ["Microphone", "Accessibility", "Input Monitoring"]);
    }

    #[test]
    fn test_microphone_authorization_status_mapping() {
        let cases = [
            (0, MicrophoneAuthorizationStatus::NotDetermined),
            (1, MicrophoneAuthorizationStatus::Restricted),
            (2, MicrophoneAuthorizationStatus::Denied),
            (3, MicrophoneAuthorizationStatus::Authorized),
            (99, MicrophoneAuthorizationStatus::Unknown(99)),
        ];

        for (raw, status) in cases {
            assert_eq!(MicrophoneAuthorizationStatus::from_raw(raw), status);
        }
    }

    #[test]
    fn test_microphone_access_errors_are_actionable() {
        let cases = [
            (
                MicrophoneAuthorizationStatus::Denied,
                ["Microphone access is denied", "System Settings"],
            ),
            (
                MicrophoneAuthorizationStatus::NotDetermined,
                ["Microphone access has not been granted", "Choose Allow"],
            ),
        ];

        for (status, snippets) in cases {
            let message = microphone_access_error(status).expect("status should produce a message");
            for snippet in snippets {
                assert!(message.contains(snippet), "{message}");
            }
        }
    }
}

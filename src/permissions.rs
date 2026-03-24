// --- macOS permission checking via FFI ---

// Accessibility: ApplicationServices framework
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

// Input Monitoring: CoreGraphics framework (macOS 10.15+)
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightListenEventAccess() -> bool;
}

/// Check if the app has Accessibility permission (needed for simulated paste via enigo).
pub fn has_accessibility_access() -> bool {
    unsafe { AXIsProcessTrusted() }
}

/// Check if the app has Input Monitoring permission (needed for global hotkey via CGEventTap).
pub fn has_input_monitoring_access() -> bool {
    unsafe { CGPreflightListenEventAccess() }
}

/// Check if the app has Microphone permission (needed for audio capture via cpal).
/// Uses a practical check: tries to list audio input devices via cpal.
/// If listing succeeds and returns devices, we likely have access.
pub fn has_microphone_access() -> bool {
    // Use cpal to check — if we can enumerate input devices, microphone access is granted.
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    match host.default_input_device() {
        Some(device) => device.default_input_config().is_ok(),
        None => false,
    }
}

/// Permission status for display in the UI.
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
            settings_url: "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone",
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
}

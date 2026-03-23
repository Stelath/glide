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
}

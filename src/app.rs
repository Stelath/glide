use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::{audio, config::GlideConfig};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppSnapshot {
    pub config: GlideConfig,
    pub status: RuntimeStatus,
    pub status_detail: String,
    pub last_transcript: String,
    pub last_error: Option<String>,
    pub input_devices: Vec<String>,
    pub permission_hint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    Starting,
    Idle,
    Recording,
    Processing,
    Error,
}

impl RuntimeStatus {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Starting => "Starting",
            Self::Idle => "Idle",
            Self::Recording => "Recording",
            Self::Processing => "Processing",
            Self::Error => "Error",
        }
    }
}

#[derive(Debug)]
pub struct SharedAppState {
    inner: Mutex<AppState>,
    /// When true, the CGEventTap will capture the next keycode for hotkey recording.
    pub hotkey_recording: AtomicBool,
    /// Raw keycode captured by the event tap during recording.
    recorded_keycode: AtomicU16,
    /// Set to true once a keycode has been recorded.
    keycode_ready: AtomicBool,
}

#[derive(Debug)]
struct AppState {
    config: GlideConfig,
    status: RuntimeStatus,
    status_detail: String,
    last_transcript: String,
    last_error: Option<String>,
    input_devices: Vec<String>,
    permission_hint: String,
}

impl SharedAppState {
    pub fn new(config: GlideConfig) -> Self {
        Self {
            inner: Mutex::new(AppState {
                config,
                status: RuntimeStatus::Starting,
                status_detail: "Booting background services".to_string(),
                last_transcript: String::new(),
                last_error: None,
                input_devices: Vec::new(),
                permission_hint: String::new(),
            }),
            hotkey_recording: AtomicBool::new(false),
            recorded_keycode: AtomicU16::new(0),
            keycode_ready: AtomicBool::new(false),
        }
    }

    /// Start hotkey recording — the CGEventTap will capture the next key press.
    pub fn start_hotkey_recording(&self) {
        self.keycode_ready.store(false, Ordering::SeqCst);
        self.recorded_keycode.store(0, Ordering::SeqCst);
        self.hotkey_recording.store(true, Ordering::SeqCst);
    }

    /// Called by the event tap when a key is pressed during recording.
    pub fn record_keycode(&self, code: u16) {
        self.recorded_keycode.store(code, Ordering::SeqCst);
        self.hotkey_recording.store(false, Ordering::SeqCst);
        self.keycode_ready.store(true, Ordering::SeqCst);
    }

    /// Poll for a recorded keycode. Returns Some(code) once, then resets.
    pub fn poll_recorded_keycode(&self) -> Option<u16> {
        if self.keycode_ready.swap(false, Ordering::SeqCst) {
            Some(self.recorded_keycode.load(Ordering::SeqCst))
        } else {
            None
        }
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let state = self.inner.lock().expect("state poisoned");
        AppSnapshot {
            config: state.config.clone(),
            status: state.status,
            status_detail: state.status_detail.clone(),
            last_transcript: state.last_transcript.clone(),
            last_error: state.last_error.clone(),
            input_devices: state.input_devices.clone(),
            permission_hint: state.permission_hint.clone(),
        }
    }

    pub fn config(&self) -> GlideConfig {
        self.inner.lock().expect("state poisoned").config.clone()
    }

    pub fn update_config(&self, update: impl FnOnce(&mut GlideConfig)) -> Result<()> {
        let mut state = self.inner.lock().expect("state poisoned");
        update(&mut state.config);
        state.config.save()?;
        Ok(())
    }

    pub fn refresh_input_devices(&self) {
        let devices = audio::list_input_devices().unwrap_or_else(|_| vec!["default".to_string()]);
        let mut state = self.inner.lock().expect("state poisoned");
        state.input_devices = if devices.is_empty() {
            vec!["default".to_string()]
        } else {
            devices
        };

        if state.config.audio.device != "default"
            && !state
                .input_devices
                .iter()
                .any(|device| device == &state.config.audio.device)
        {
            state.config.audio.device = "default".to_string();
            let _ = state.config.save();
        }
    }

    pub fn set_permission_hint(&self, hint: String) {
        let mut state = self.inner.lock().expect("state poisoned");
        state.permission_hint = hint;
    }

    pub fn set_status(&self, status: RuntimeStatus, detail: impl Into<String>) {
        let mut state = self.inner.lock().expect("state poisoned");
        state.status = status;
        state.status_detail = detail.into();
        if status != RuntimeStatus::Error {
            state.last_error = None;
        }
    }

    pub fn set_error(&self, message: impl Into<String>) {
        let mut state = self.inner.lock().expect("state poisoned");
        let message = message.into();
        state.status = RuntimeStatus::Error;
        state.status_detail = message.clone();
        state.last_error = Some(message);
    }

    pub fn set_last_transcript(&self, transcript: String) {
        let mut state = self.inner.lock().expect("state poisoned");
        state.last_transcript = transcript;
    }
}

pub type SharedState = Arc<SharedAppState>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GlideConfig;

    fn make_state() -> SharedAppState {
        SharedAppState::new(GlideConfig::default())
    }

    #[test]
    fn test_new_state_defaults() {
        let state = make_state();
        let snap = state.snapshot();
        assert_eq!(snap.status, RuntimeStatus::Starting);
        assert!(snap.last_transcript.is_empty());
        assert!(snap.last_error.is_none());
        assert!(snap.permission_hint.is_empty());
    }

    #[test]
    fn test_set_status_updates_snapshot() {
        let state = make_state();
        state.set_status(RuntimeStatus::Idle, "Ready");
        let snap = state.snapshot();
        assert_eq!(snap.status, RuntimeStatus::Idle);
        assert_eq!(snap.status_detail, "Ready");
    }

    #[test]
    fn test_set_error_sets_status_and_last_error() {
        let state = make_state();
        state.set_error("boom");
        let snap = state.snapshot();
        assert_eq!(snap.status, RuntimeStatus::Error);
        assert_eq!(snap.last_error.as_deref(), Some("boom"));
        assert_eq!(snap.status_detail, "boom");
    }

    #[test]
    fn test_non_error_status_clears_last_error() {
        let state = make_state();
        state.set_error("problem");
        assert!(state.snapshot().last_error.is_some());

        state.set_status(RuntimeStatus::Idle, "OK");
        assert!(state.snapshot().last_error.is_none());
    }

    #[test]
    fn test_set_last_transcript() {
        let state = make_state();
        state.set_last_transcript("hello world".to_string());
        assert_eq!(state.snapshot().last_transcript, "hello world");
    }

    #[test]
    fn test_set_permission_hint() {
        let state = make_state();
        state.set_permission_hint("grant access".to_string());
        assert_eq!(state.snapshot().permission_hint, "grant access");
    }

    #[test]
    fn test_config_readable() {
        let state = make_state();
        let config = state.config();
        // Default config should have default values
        assert_eq!(config.audio.sample_rate, 16_000);
    }

    #[test]
    fn test_runtime_status_labels() {
        let statuses = [
            RuntimeStatus::Starting,
            RuntimeStatus::Idle,
            RuntimeStatus::Recording,
            RuntimeStatus::Processing,
            RuntimeStatus::Error,
        ];
        for status in statuses {
            assert!(!status.label().is_empty());
        }
    }
}

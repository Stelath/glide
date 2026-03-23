use std::{fmt, fs, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlideConfig {
    pub app: AppConfig,
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub stt: SttConfig,
    pub llm: LlmConfig,
    pub overlay: OverlayConfig,
    pub paste: PasteConfig,
}

impl Default for GlideConfig {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            hotkey: HotkeyConfig::default(),
            audio: AudioConfig::default(),
            stt: SttConfig::default(),
            llm: LlmConfig::default(),
            overlay: OverlayConfig::default(),
            paste: PasteConfig::default(),
        }
    }
}

impl GlideConfig {
    pub fn load_or_create() -> Result<Self> {
        let path = config_path()?;

        // Migrate from old ~/.config/glide/ location if needed
        if !path.exists() {
            if let Some(old_path) = legacy_config_path() {
                if old_path.exists() {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("failed to create {}", parent.display()))?;
                        set_dir_permissions(parent);
                    }
                    fs::copy(&old_path, &path).with_context(|| {
                        format!(
                            "failed to migrate config from {} to {}",
                            old_path.display(),
                            path.display()
                        )
                    })?;
                    set_file_permissions(&path);
                }
            }
        }

        if path.exists() {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config at {}", path.display()))?;
            let config: Self = toml::from_str(&raw)
                .with_context(|| format!("failed to parse config at {}", path.display()))?;
            config.validate()?;
            return Ok(config);
        }

        let config = Self::default();
        config.save()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = config_path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
            set_dir_permissions(parent);
        }

        let raw = toml::to_string_pretty(self).context("failed to serialize config")?;
        fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))?;
        set_file_permissions(&path);
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.audio.sample_rate > 0,
            "audio.sample_rate must be positive"
        );
        anyhow::ensure!(self.audio.channels > 0, "audio.channels must be positive");
        anyhow::ensure!(self.overlay.width > 0, "overlay.width must be positive");
        anyhow::ensure!(self.overlay.height > 0, "overlay.height must be positive");
        anyhow::ensure!(
            (0.0..=1.0).contains(&self.overlay.opacity),
            "overlay.opacity must be between 0 and 1"
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub launch_at_login: bool,
    pub menu_bar_icon: MenuBarIcon,
    pub theme: ThemePreference,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            menu_bar_icon: MenuBarIcon::Default,
            theme: ThemePreference::System,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarIcon {
    Default,
    Monochrome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    pub trigger: HotkeyTrigger,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            trigger: HotkeyTrigger::F8,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyTrigger {
    Option,
    CommandRight,
    F8,
    F9,
    F10,
    Custom(u16),
}

impl HotkeyTrigger {
    pub fn label(self) -> String {
        match self {
            Self::Option => "⌥ Option".to_string(),
            Self::CommandRight => "⌘ Right Cmd".to_string(),
            Self::F8 => "F8".to_string(),
            Self::F9 => "F9".to_string(),
            Self::F10 => "F10".to_string(),
            Self::Custom(code) => keycode_label(code),
        }
    }

    /// Map a macOS virtual keycode to the best matching named trigger,
    /// falling back to `Custom(code)`.
    #[allow(dead_code)]
    pub fn from_keycode(code: u16) -> Self {
        match code {
            58 | 61 => Self::Option,
            54 => Self::CommandRight,
            100 => Self::F8,
            101 => Self::F9,
            109 => Self::F10,
            _ => Self::Custom(code),
        }
    }

    /// Map a GPUI keystroke key string to a trigger.
    pub fn from_key_name(key: &str) -> Self {
        match key {
            "alt" => Self::Option,
            "f8" => Self::F8,
            "f9" => Self::F9,
            "f10" => Self::F10,
            _ => {
                // Map common key names to macOS virtual keycodes
                let code = key_name_to_keycode(key);
                Self::Custom(code)
            }
        }
    }
}

/// Human-readable label for a macOS virtual keycode.
fn keycode_label(code: u16) -> String {
    match code {
        0 => "A".to_string(),
        1 => "S".to_string(),
        2 => "D".to_string(),
        3 => "F".to_string(),
        4 => "H".to_string(),
        5 => "G".to_string(),
        6 => "Z".to_string(),
        7 => "X".to_string(),
        8 => "C".to_string(),
        9 => "V".to_string(),
        11 => "B".to_string(),
        12 => "Q".to_string(),
        13 => "W".to_string(),
        14 => "E".to_string(),
        15 => "R".to_string(),
        16 => "Y".to_string(),
        17 => "T".to_string(),
        31 => "O".to_string(),
        32 => "U".to_string(),
        34 => "I".to_string(),
        35 => "P".to_string(),
        36 => "Return".to_string(),
        37 => "L".to_string(),
        38 => "J".to_string(),
        40 => "K".to_string(),
        45 => "N".to_string(),
        46 => "M".to_string(),
        49 => "Space".to_string(),
        50 => "`".to_string(),
        51 => "Delete".to_string(),
        53 => "Escape".to_string(),
        54 => "⌘ Right Cmd".to_string(),
        55 => "⌘ Left Cmd".to_string(),
        56 => "⇧ Left Shift".to_string(),
        57 => "⇪ Caps Lock".to_string(),
        58 => "⌥ Left Option".to_string(),
        59 => "⌃ Left Ctrl".to_string(),
        60 => "⇧ Right Shift".to_string(),
        61 => "⌥ Right Option".to_string(),
        62 => "⌃ Right Ctrl".to_string(),
        96 => "F5".to_string(),
        97 => "F6".to_string(),
        98 => "F7".to_string(),
        99 => "F3".to_string(),
        100 => "F8".to_string(),
        101 => "F9".to_string(),
        103 => "F11".to_string(),
        109 => "F10".to_string(),
        111 => "F12".to_string(),
        118 => "F4".to_string(),
        120 => "F2".to_string(),
        122 => "F1".to_string(),
        _ => format!("Key {code}"),
    }
}

/// Map a GPUI key name string to a macOS virtual keycode.
fn key_name_to_keycode(name: &str) -> u16 {
    match name {
        "a" => 0, "s" => 1, "d" => 2, "f" => 3, "h" => 4, "g" => 5,
        "z" => 6, "x" => 7, "c" => 8, "v" => 9, "b" => 11, "q" => 12,
        "w" => 13, "e" => 14, "r" => 15, "y" => 16, "t" => 17,
        "o" => 31, "u" => 32, "i" => 34, "p" => 35, "l" => 37,
        "j" => 38, "k" => 40, "n" => 45, "m" => 46,
        "enter" => 36, "space" => 49, "`" => 50, "backspace" => 51,
        "escape" => 53, "tab" => 48,
        "f1" => 122, "f2" => 120, "f3" => 99, "f4" => 118,
        "f5" => 96, "f6" => 97, "f7" => 98, "f8" => 100,
        "f9" => 101, "f10" => 109, "f11" => 103, "f12" => 111,
        _ => 0,
    }
}

impl fmt::Display for HotkeyTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub device: String,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            channels: 1,
            device: "default".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SttConfig {
    pub provider: SttProviderKind,
    pub openai: OpenAiSttConfig,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: SttProviderKind::OpenAi,
            openai: OpenAiSttConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderKind {
    OpenAi,
}

impl SttProviderKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI Whisper",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAiSttConfig {
    pub api_key: String,
    pub api_key_env: String,
    pub model: String,
    pub endpoint: String,
}

impl Default for OpenAiSttConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            model: "whisper-1".to_string(),
            endpoint: "https://api.openai.com/v1/audio/transcriptions".to_string(),
        }
    }
}

impl OpenAiSttConfig {
    pub fn resolve_api_key(&self) -> Result<String> {
        if !self.api_key.trim().is_empty() {
            return Ok(self.api_key.trim().to_string());
        }

        std::env::var(&self.api_key_env).with_context(|| {
            format!(
                "missing speech-to-text API key; set it in Glide settings or via {}",
                self.api_key_env
            )
        })
    }

    pub fn credential_source(&self) -> &'static str {
        if self.api_key.trim().is_empty() {
            "environment variable"
        } else {
            "saved in config"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmConfig {
    pub provider: LlmProviderKind,
    pub openai: OpenAiLlmConfig,
    pub prompt: PromptConfig,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            provider: LlmProviderKind::None,
            openai: OpenAiLlmConfig::default(),
            prompt: PromptConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderKind {
    None,
    OpenAi,
}

impl LlmProviderKind {
    pub const ALL: [Self; 2] = [Self::None, Self::OpenAi];

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OpenAiLlmConfig {
    pub api_key: String,
    pub api_key_env: String,
    pub model: String,
    pub endpoint: String,
}

impl Default for OpenAiLlmConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            model: "gpt-4o-mini".to_string(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
        }
    }
}

impl OpenAiLlmConfig {
    pub fn resolve_api_key(&self) -> Result<String> {
        if !self.api_key.trim().is_empty() {
            return Ok(self.api_key.trim().to_string());
        }

        std::env::var(&self.api_key_env).with_context(|| {
            format!(
                "missing cleanup API key; set it in Glide settings or via {}",
                self.api_key_env
            )
        })
    }

    pub fn credential_source(&self) -> &'static str {
        if self.api_key.trim().is_empty() {
            "environment variable"
        } else {
            "saved in config"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PromptConfig {
    pub system: String,
    pub app_overrides: Vec<AppPromptOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPromptOverride {
    pub app_name: String,
    pub prompt: String,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            system: "You are a dictation assistant. Clean up the following raw speech transcript into well-formatted text. Fix grammar, punctuation, and filler words. Preserve the speaker's intent exactly.".to_string(),
            app_overrides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayStyle {
    Classic,
    Mini,
    None,
}

impl OverlayStyle {
    pub const ALL: [Self; 3] = [Self::Classic, Self::Mini, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Mini => "Mini",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    pub style: OverlayStyle,
    pub width: u32,
    pub height: u32,
    pub position: String,
    pub opacity: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            style: OverlayStyle::Classic,
            width: 300,
            height: 80,
            position: "bottom-center".to_string(),
            opacity: 0.85,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PasteConfig {
    pub restore_clipboard: bool,
    pub restore_delay_ms: u64,
}

impl Default for PasteConfig {
    fn default() -> Self {
        Self {
            restore_clipboard: true,
            restore_delay_ms: 100,
        }
    }
}

pub fn config_dir_path() -> Result<PathBuf> {
    let root =
        dirs::data_local_dir().context("failed to resolve local application data directory")?;
    Ok(root.join("glide"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir_path()?.join("config.toml"))
}

/// Returns the old ~/.config/glide/config.toml path for migration purposes.
fn legacy_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|root| root.join("glide").join("config.toml"))
}

#[cfg(unix)]
fn set_file_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_file_permissions(_path: &std::path::Path) {}

#[cfg(unix)]
fn set_dir_permissions(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn set_dir_permissions(_path: &std::path::Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_is_valid() {
        GlideConfig::default().validate().unwrap();
    }

    #[test]
    fn test_validation_rejects_zero_sample_rate() {
        let mut config = GlideConfig::default();
        config.audio.sample_rate = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_rejects_zero_channels() {
        let mut config = GlideConfig::default();
        config.audio.channels = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_rejects_zero_overlay_dimensions() {
        let mut config = GlideConfig::default();
        config.overlay.width = 0;
        assert!(config.validate().is_err());

        let mut config = GlideConfig::default();
        config.overlay.height = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validation_rejects_bad_opacity() {
        let mut config = GlideConfig::default();
        config.overlay.opacity = 2.0;
        assert!(config.validate().is_err());

        config.overlay.opacity = -0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = GlideConfig::default();
        let serialized = toml::to_string_pretty(&config).unwrap();
        let parsed: GlideConfig = toml::from_str(&serialized).unwrap();
        // Compare by re-serializing (GlideConfig doesn't derive PartialEq)
        let reserialized = toml::to_string_pretty(&parsed).unwrap();
        assert_eq!(serialized, reserialized);
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let config = GlideConfig::default();
        let raw = toml::to_string_pretty(&config).unwrap();
        fs::write(&path, &raw).unwrap();

        let loaded: GlideConfig = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        loaded.validate().unwrap();

        let loaded_raw = toml::to_string_pretty(&loaded).unwrap();
        assert_eq!(raw, loaded_raw);
    }

    #[test]
    fn test_hotkey_trigger_labels() {
        assert!(!HotkeyTrigger::Option.label().is_empty());
        assert!(!HotkeyTrigger::CommandRight.label().is_empty());
        assert!(!HotkeyTrigger::F8.label().is_empty());
        assert!(!HotkeyTrigger::F9.label().is_empty());
        assert!(!HotkeyTrigger::F10.label().is_empty());
        assert!(!HotkeyTrigger::Custom(49).label().is_empty());
    }

    #[test]
    fn test_hotkey_from_key_name() {
        assert_eq!(HotkeyTrigger::from_key_name("f8"), HotkeyTrigger::F8);
        assert_eq!(HotkeyTrigger::from_key_name("alt"), HotkeyTrigger::Option);
        assert_eq!(HotkeyTrigger::from_key_name("space"), HotkeyTrigger::Custom(49));
    }

    #[test]
    fn test_overlay_style_labels() {
        assert_eq!(OverlayStyle::Classic.label(), "Classic");
        assert_eq!(OverlayStyle::Mini.label(), "Mini");
        assert_eq!(OverlayStyle::None.label(), "None");
    }

    #[test]
    fn test_theme_preference_labels() {
        for pref in ThemePreference::ALL {
            assert!(!pref.label().is_empty());
        }
    }

    #[test]
    fn test_llm_provider_cycling() {
        assert_eq!(LlmProviderKind::None.next(), LlmProviderKind::OpenAi);
        assert_eq!(LlmProviderKind::OpenAi.next(), LlmProviderKind::None);
    }

    #[test]
    fn test_credential_source_env_when_empty() {
        let stt = OpenAiSttConfig::default();
        assert_eq!(stt.credential_source(), "environment variable");

        let llm = OpenAiLlmConfig::default();
        assert_eq!(llm.credential_source(), "environment variable");
    }

    #[test]
    fn test_credential_source_config_when_set() {
        let mut stt = OpenAiSttConfig::default();
        stt.api_key = "sk-test".to_string();
        assert_eq!(stt.credential_source(), "saved in config");

        let mut llm = OpenAiLlmConfig::default();
        llm.api_key = "sk-test".to_string();
        assert_eq!(llm.credential_source(), "saved in config");
    }

    #[test]
    fn test_resolve_api_key_from_env() {
        let unique_var = "GLIDE_TEST_API_KEY_RESOLVE";
        // Safety: single-threaded test, no other threads reading this var
        unsafe { std::env::set_var(unique_var, "test-key-123") };

        let mut stt = OpenAiSttConfig::default();
        stt.api_key_env = unique_var.to_string();
        stt.api_key = String::new();

        let resolved = stt.resolve_api_key().unwrap();
        assert_eq!(resolved, "test-key-123");

        unsafe { std::env::remove_var(unique_var) };
    }

    #[test]
    fn test_resolve_api_key_prefers_config() {
        let mut stt = OpenAiSttConfig::default();
        stt.api_key = "direct-key".to_string();

        let resolved = stt.resolve_api_key().unwrap();
        assert_eq!(resolved, "direct-key");
    }

    #[test]
    fn test_resolve_api_key_fails_when_missing() {
        let mut stt = OpenAiSttConfig::default();
        stt.api_key = String::new();
        stt.api_key_env = "GLIDE_NONEXISTENT_VAR_12345".to_string();

        assert!(stt.resolve_api_key().is_err());
    }
}

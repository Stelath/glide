pub mod models;
pub mod providers;

pub use models::{DictationConfig, DictionaryConfig, ModelSelection, ReplacementRule, Style};
pub use providers::{Provider, ProvidersConfig};

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub fn asset_path(relative: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    let bundle_resources = exe
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("Resources").join(relative));
    if let Some(ref p) = bundle_resources
        && p.exists()
    {
        return p.clone();
    }
    std::env::current_dir().unwrap_or_default().join(relative)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GlideConfig {
    pub app: AppConfig,
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub providers: ProvidersConfig,
    pub dictation: DictationConfig,
    pub dictionary: DictionaryConfig,
    pub overlay: OverlayConfig,
    pub paste: PasteConfig,
}

impl GlideConfig {
    pub fn load_or_create() -> Result<Self> {
        let mut config: Self = confy::load("glide", "config").unwrap_or_default();
        let api_keys = load_provider_keys_from_keyring();
        config.providers.openai.api_key = api_keys.get("openai").cloned().unwrap_or_default();
        config.providers.groq.api_key = api_keys.get("groq").cloned().unwrap_or_default();
        config.providers.cerebras.api_key = api_keys.get("cerebras").cloned().unwrap_or_default();
        config.providers.fireworks.api_key = api_keys.get("fireworks").cloned().unwrap_or_default();
        config.providers.elevenlabs.api_key =
            api_keys.get("elevenlabs").cloned().unwrap_or_default();
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        #[cfg(not(test))]
        {
            confy::store("glide", "config", self)
                .map_err(|e| anyhow::anyhow!("failed to save config: {e}"))?;
            save_provider_keys_to_keyring(&provider_keys_from_config(self));
        }
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

// ---------------------------------------------------------------------------
// App config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub launch_at_login: bool,
    pub menu_bar_icon: MenuBarIcon,
    pub theme: ThemePreference,
    pub accent: ColorAccent,
    #[serde(default)]
    pub onboarding_completed: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            launch_at_login: false,
            menu_bar_icon: MenuBarIcon::Default,
            theme: ThemePreference::System,
            accent: ColorAccent::Slate,
            onboarding_completed: false,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ColorAccent {
    Purple,
    Blue,
    Orange,
    #[default]
    Slate,
}

impl ColorAccent {
    pub const ALL: [Self; 4] = [Self::Purple, Self::Blue, Self::Orange, Self::Slate];

    pub fn label(self) -> &'static str {
        match self {
            Self::Purple => "Purple",
            Self::Blue => "Blue",
            Self::Orange => "Orange",
            Self::Slate => "Slate",
        }
    }

    /// Primary accent color in HSLA for the GPUI theme system.
    pub fn primary_hsla(self) -> (f32, f32, f32, f32) {
        match self {
            // #7E6CC4 → hsl(252, 38%, 60%)
            Self::Purple => (0.70, 0.38, 0.60, 1.0),
            // #4A8FD4 → hsl(211, 58%, 56%)
            Self::Blue => (0.586, 0.58, 0.56, 1.0),
            // #F0603A → hsl(13, 85%, 58%)
            Self::Orange => (0.035, 0.85, 0.58, 1.0),
            // Near-black for dark selected pill appearance
            Self::Slate => (0.0, 0.0, 0.15, 1.0),
        }
    }

    /// Slightly lighter variant for hover state.
    pub fn primary_hover_hsla(self) -> (f32, f32, f32, f32) {
        let (h, s, l, a) = self.primary_hsla();
        (h, s, (l + 0.08).min(1.0), a)
    }

    /// Slightly darker variant for active/pressed state.
    pub fn primary_active_hsla(self) -> (f32, f32, f32, f32) {
        let (h, s, l, a) = self.primary_hsla();
        (h, s, (l - 0.08).max(0.0), a)
    }

    /// HSLA color for overlay EQ bars and loading dots.
    /// Slate uses the original neutral gray; others use tinted bars.
    pub fn bar_hsla(self) -> (f32, f32, f32, f32) {
        match self {
            // Original neutral gray bars
            Self::Slate => (0.0, 0.0, 0.78, 0.9),
            // Tinted bars matching the accent
            Self::Purple => (0.70, 0.35, 0.75, 0.9),
            Self::Blue => (0.586, 0.45, 0.75, 0.9),
            Self::Orange => (0.035, 0.65, 0.72, 0.9),
        }
    }

    /// RGBA color for notch overlay bars and dots (used in ObjC FFI).
    pub fn bar_rgba(self) -> (f64, f64, f64, f64) {
        match self {
            // Original neutral white bars
            Self::Slate => (0.78, 0.78, 0.78, 0.9),
            // Tinted bars matching the accent
            Self::Purple => (0.65, 0.55, 0.85, 0.9),
            Self::Blue => (0.45, 0.65, 0.88, 0.9),
            Self::Orange => (0.92, 0.50, 0.32, 0.9),
        }
    }

    /// Path to the .icns file for this accent (relative to assets/).
    pub fn icns_asset(self) -> &'static str {
        match self {
            Self::Purple => "assets/icons/AppIcon-Purple.icns",
            Self::Blue => "assets/icons/AppIcon-Blue.icns",
            Self::Orange => "assets/icons/AppIcon-Orange.icns",
            Self::Slate => "assets/icons/AppIcon-Slate.icns",
        }
    }

    /// RGB values for the notch glow overlay effect.
    /// Returns `None` for Slate (rainbow hue-cycling glow).
    pub fn glow_rgb(self) -> Option<(f64, f64, f64)> {
        match self {
            Self::Purple => Some((0.49, 0.42, 0.77)),
            Self::Blue => Some((0.29, 0.56, 0.83)),
            Self::Orange => Some((0.94, 0.38, 0.23)),
            Self::Slate => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Hotkey config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HotkeyConfig {
    #[serde(default = "default_hold_trigger")]
    pub trigger: Option<HotkeyTrigger>,
    #[serde(default)]
    pub toggle_trigger: Option<HotkeyTrigger>,
}

fn default_hold_trigger() -> Option<HotkeyTrigger> {
    Some(HotkeyTrigger::F8)
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            trigger: default_hold_trigger(),
            toggle_trigger: None,
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

    pub fn from_keycode(code: u16) -> Self {
        match code {
            100 => Self::F8,
            101 => Self::F9,
            109 => Self::F10,
            _ => Self::Custom(code),
        }
    }
}

impl fmt::Display for HotkeyTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label())
    }
}

fn keycode_label(code: u16) -> String {
    match code {
        0 => "A",
        1 => "S",
        2 => "D",
        3 => "F",
        4 => "H",
        5 => "G",
        6 => "Z",
        7 => "X",
        8 => "C",
        9 => "V",
        11 => "B",
        12 => "Q",
        13 => "W",
        14 => "E",
        15 => "R",
        16 => "Y",
        17 => "T",
        31 => "O",
        32 => "U",
        34 => "I",
        35 => "P",
        36 => "Return",
        37 => "L",
        38 => "J",
        40 => "K",
        45 => "N",
        46 => "M",
        49 => "Space",
        50 => "`",
        51 => "Delete",
        53 => "Escape",
        54 => "⌘ Right Cmd",
        55 => "⌘ Left Cmd",
        56 => "⇧ Left Shift",
        57 => "⇪ Caps Lock",
        58 => "⌥ Left Option",
        59 => "⌃ Left Ctrl",
        60 => "⇧ Right Shift",
        61 => "⌥ Right Option",
        62 => "⌃ Right Ctrl",
        63 => "Fn",
        96 => "F5",
        97 => "F6",
        98 => "F7",
        99 => "F3",
        100 => "F8",
        101 => "F9",
        103 => "F11",
        109 => "F10",
        111 => "F12",
        118 => "F4",
        120 => "F2",
        122 => "F1",
        _ => return format!("Key {code}"),
    }
    .to_string()
}

/// Return the CGEvent flag mask for a modifier keycode.
pub fn modifier_flag_for_keycode(code: u16) -> u64 {
    match code {
        54 | 55 => 0x00100000, // Command
        56 | 60 => 0x00020000, // Shift
        58 | 61 => 0x00080000, // Option
        59 | 62 => 0x00040000, // Control
        57 => 0x00010000,      // CapsLock
        63 => 0x00800000,      // Function
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Audio config
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Overlay config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayStyle {
    Classic,
    Glow,
    None,
}

impl OverlayStyle {
    pub const ALL: [Self; 3] = [Self::Classic, Self::Glow, Self::None];

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Glow => "Glow",
            Self::None => "None",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    Notch,
    Floating,
}

impl OverlayPosition {
    pub const ALL: [Self; 2] = [Self::Notch, Self::Floating];

    pub fn label(self) -> &'static str {
        match self {
            Self::Notch => "Notch",
            Self::Floating => "Floating",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    pub style: OverlayStyle,
    pub width: u32,
    pub height: u32,
    pub position: OverlayPosition,
    pub opacity: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        Self {
            style: OverlayStyle::Classic,
            width: 300,
            height: 80,
            position: OverlayPosition::Floating,
            opacity: 0.85,
        }
    }
}

// ---------------------------------------------------------------------------
// Paste config
// ---------------------------------------------------------------------------

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
            restore_delay_ms: 750,
        }
    }
}

// ---------------------------------------------------------------------------
// Keyring helpers
// ---------------------------------------------------------------------------

const KEYRING_SERVICE: &str = "glide";
const KEYRING_ACCOUNT: &str = "provider-api-keys";
const REMOTE_PROVIDER_KEY_IDS: [&str; 5] =
    ["openai", "groq", "cerebras", "fireworks", "elevenlabs"];

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProviderKeyringPayload {
    version: u8,
    api_keys: BTreeMap<String, String>,
}

fn provider_keys_from_config(config: &GlideConfig) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    insert_provider_key(&mut keys, "openai", &config.providers.openai.api_key);
    insert_provider_key(&mut keys, "groq", &config.providers.groq.api_key);
    insert_provider_key(&mut keys, "cerebras", &config.providers.cerebras.api_key);
    insert_provider_key(&mut keys, "fireworks", &config.providers.fireworks.api_key);
    insert_provider_key(
        &mut keys,
        "elevenlabs",
        &config.providers.elevenlabs.api_key,
    );
    keys
}

fn insert_provider_key(keys: &mut BTreeMap<String, String>, provider: &str, key: &str) {
    if !key.trim().is_empty() {
        keys.insert(provider.to_string(), key.to_string());
    }
}

fn load_provider_keys_from_keyring() -> BTreeMap<String, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
        .and_then(|e| e.get_password())
        .ok()
        .map(|raw| decode_provider_keys(&raw))
        .unwrap_or_default()
}

#[cfg_attr(test, allow(dead_code))]
fn save_provider_keys_to_keyring(keys: &BTreeMap<String, String>) {
    let Some(payload) = encode_provider_keys(keys) else {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
            let _ = entry.delete_credential();
        }
        return;
    };

    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT) {
        let current = entry.get_password().unwrap_or_default();
        if current != payload {
            let _ = entry.set_password(&payload);
        }
    }
}

fn encode_provider_keys(keys: &BTreeMap<String, String>) -> Option<String> {
    let api_keys = keys
        .iter()
        .filter(|(_, key)| !key.trim().is_empty())
        .map(|(provider, key)| (provider.clone(), key.clone()))
        .collect::<BTreeMap<_, _>>();

    if api_keys.is_empty() {
        return None;
    }

    serde_json::to_string(&ProviderKeyringPayload {
        version: 1,
        api_keys,
    })
    .ok()
}

fn decode_provider_keys(raw: &str) -> BTreeMap<String, String> {
    serde_json::from_str::<ProviderKeyringPayload>(raw)
        .map(|payload| payload.api_keys)
        .or_else(|_| serde_json::from_str::<BTreeMap<String, String>>(raw))
        .unwrap_or_default()
        .into_iter()
        .filter(|(provider, key)| {
            REMOTE_PROVIDER_KEY_IDS.contains(&provider.as_str()) && !key.trim().is_empty()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

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
    fn test_provider_key_payload_roundtrip() {
        let mut config = GlideConfig::default();
        config.providers.openai.api_key = "openai-key".to_string();
        config.providers.groq.api_key = "groq-key".to_string();
        config.providers.cerebras.api_key = "cerebras-key".to_string();
        config.providers.fireworks.api_key = "fireworks-key".to_string();
        config.providers.elevenlabs.api_key = "elevenlabs-key".to_string();

        let keys = provider_keys_from_config(&config);
        let payload = encode_provider_keys(&keys).unwrap();
        let decoded = decode_provider_keys(&payload);

        assert_eq!(decoded.get("openai").unwrap(), "openai-key");
        assert_eq!(decoded.get("groq").unwrap(), "groq-key");
        assert_eq!(decoded.get("cerebras").unwrap(), "cerebras-key");
        assert_eq!(decoded.get("fireworks").unwrap(), "fireworks-key");
        assert_eq!(decoded.get("elevenlabs").unwrap(), "elevenlabs-key");
    }

    #[test]
    fn test_provider_key_payload_omits_empty_and_unknown_keys() {
        let mut keys = BTreeMap::new();
        keys.insert("openai".to_string(), "openai-key".to_string());
        keys.insert("groq".to_string(), "  ".to_string());
        keys.insert("cerebras".to_string(), "cerebras-key".to_string());
        keys.insert("fireworks".to_string(), "fireworks-key".to_string());
        keys.insert("elevenlabs".to_string(), "elevenlabs-key".to_string());
        keys.insert("other".to_string(), "other-key".to_string());

        let payload = encode_provider_keys(&keys).unwrap();
        let decoded = decode_provider_keys(&payload);

        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded.get("openai").unwrap(), "openai-key");
        assert_eq!(decoded.get("cerebras").unwrap(), "cerebras-key");
        assert_eq!(decoded.get("fireworks").unwrap(), "fireworks-key");
        assert_eq!(decoded.get("elevenlabs").unwrap(), "elevenlabs-key");
    }

    #[test]
    fn test_provider_key_payload_accepts_plain_map_shape() {
        let decoded = decode_provider_keys(
            r#"{"openai":"openai-key","cerebras":"cerebras-key","fireworks":"fireworks-key","elevenlabs":"elevenlabs-key","other":"ignored"}"#,
        );

        assert_eq!(decoded.len(), 4);
        assert_eq!(decoded.get("openai").unwrap(), "openai-key");
        assert_eq!(decoded.get("cerebras").unwrap(), "cerebras-key");
        assert_eq!(decoded.get("fireworks").unwrap(), "fireworks-key");
        assert_eq!(decoded.get("elevenlabs").unwrap(), "elevenlabs-key");
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
    fn test_hotkey_from_keycode() {
        assert_eq!(HotkeyTrigger::from_keycode(100), HotkeyTrigger::F8);
        assert_eq!(HotkeyTrigger::from_keycode(58), HotkeyTrigger::Custom(58));
        assert_eq!(HotkeyTrigger::from_keycode(61), HotkeyTrigger::Custom(61));
        assert_eq!(HotkeyTrigger::from_keycode(55), HotkeyTrigger::Custom(55));
        assert_eq!(HotkeyTrigger::from_keycode(54), HotkeyTrigger::Custom(54));
        assert_eq!(HotkeyTrigger::from_keycode(63), HotkeyTrigger::Custom(63));
        assert_eq!(HotkeyTrigger::from_keycode(49), HotkeyTrigger::Custom(49));
    }

    #[test]
    fn test_overlay_style_labels() {
        assert_eq!(OverlayStyle::Classic.label(), "Classic");
        assert_eq!(OverlayStyle::Glow.label(), "Glow");
        assert_eq!(OverlayStyle::None.label(), "None");
    }

    #[test]
    fn test_theme_preference_labels() {
        for pref in ThemePreference::ALL {
            assert!(!pref.label().is_empty());
        }
    }
}

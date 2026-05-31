pub mod models;
pub mod providers;

pub use models::{
    DictationConfig, DictionaryConfig, ModelSelection, ReplacementRule, STYLE_PROMPT_PLACEHOLDER,
    Style,
};
pub use providers::{Provider, ProviderCredentials, ProvidersConfig};

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use strum::EnumMessage as _;

const CONFIG_APP_NAME: &str = "glide";
const CONFIG_NAME: &str = "config";

// --- Config loading ---

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
        let mut config: Self =
            confy::load(CONFIG_APP_NAME, CONFIG_NAME).context("failed to load Glide config")?;
        config.dictation.refresh_builtin_prompt_defaults();
        let api_keys = load_provider_keys_from_keyring();
        for provider in Provider::REMOTE {
            let Some(key_id) = provider.key_id() else {
                continue;
            };
            config.providers.credentials_for_mut(provider).api_key =
                api_keys.get(key_id).cloned().unwrap_or_default();
        }
        config.validate()?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        #[cfg(not(test))]
        {
            confy::store(CONFIG_APP_NAME, CONFIG_NAME, self)
                .map_err(|e| anyhow::anyhow!("failed to save config: {e}"))?;
            save_provider_keys_to_keyring(&provider_keys_from_config(self));
        }
        Ok(())
    }

    pub fn config_file_path() -> Result<PathBuf> {
        confy::get_configuration_file_path(CONFIG_APP_NAME, CONFIG_NAME)
            .context("failed to locate Glide config file")
    }

    pub fn reset_to_default() -> Result<Option<PathBuf>> {
        let path = Self::config_file_path()?;
        let backup_path = backup_config_file(&path)?;

        #[cfg(not(test))]
        {
            let config = Self::default();
            confy::store(CONFIG_APP_NAME, CONFIG_NAME, &config)
                .map_err(|e| anyhow::anyhow!("failed to reset config: {e}"))?;
            save_provider_keys_to_keyring(&provider_keys_from_config(&config));
        }

        Ok(backup_path)
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

fn backup_config_file(path: &std::path::Path) -> Result<Option<PathBuf>> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.toml");
    let backup_path = path.with_file_name(format!("{file_name}.corrupt-{timestamp}.bak"));

    match std::fs::rename(path, &backup_path) {
        Ok(()) => Ok(Some(backup_path)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("failed to back up corrupt config at {}", path.display())),
    }
}

// --- App config ---

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

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum::EnumMessage,
    strum::VariantArray,
)]
#[serde(rename_all = "snake_case")]
pub enum ThemePreference {
    #[strum(message = "System")]
    System,
    #[strum(message = "Light")]
    Light,
    #[strum(message = "Dark")]
    Dark,
}

impl ThemePreference {
    pub fn label(self) -> &'static str {
        self.get_message().expect("theme label")
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MenuBarIcon {
    Default,
    Monochrome,
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    Default,
    strum::EnumMessage,
    strum::VariantArray,
)]
#[serde(rename_all = "snake_case")]
pub enum ColorAccent {
    #[strum(message = "Purple")]
    Purple,
    #[strum(message = "Blue")]
    Blue,
    #[strum(message = "Orange")]
    Orange,
    #[default]
    #[strum(message = "Slate")]
    Slate,
}

impl ColorAccent {
    pub fn label(self) -> &'static str {
        self.get_message().expect("accent label")
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

// --- Hotkey config ---

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

// --- Audio config ---

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

// --- Overlay config ---

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum::EnumMessage,
    strum::VariantArray,
)]
#[serde(rename_all = "snake_case")]
pub enum OverlayStyle {
    #[strum(message = "Classic")]
    Classic,
    #[strum(message = "Glow")]
    Glow,
    #[strum(message = "None")]
    None,
}

impl OverlayStyle {
    pub fn label(self) -> &'static str {
        self.get_message().expect("overlay style label")
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    strum::EnumMessage,
    strum::VariantArray,
)]
#[serde(rename_all = "snake_case")]
pub enum OverlayPosition {
    #[strum(message = "Notch")]
    Notch,
    #[strum(message = "Floating")]
    Floating,
}

impl OverlayPosition {
    pub fn label(self) -> &'static str {
        self.get_message().expect("overlay position label")
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

// --- Paste config ---

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

// --- Keyring helpers ---

const KEYRING_SERVICE: &str = "glide";
const KEYRING_ACCOUNT: &str = "provider-api-keys";

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProviderKeyringPayload {
    version: u8,
    api_keys: BTreeMap<String, String>,
}

fn provider_keys_from_config(config: &GlideConfig) -> BTreeMap<String, String> {
    let mut keys = BTreeMap::new();
    for (provider, credentials) in config.providers.remote_credentials() {
        let Some(key_id) = provider.key_id() else {
            continue;
        };
        insert_provider_key(&mut keys, key_id, &credentials.api_key);
    }
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

#[cfg(not(test))]
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
        .filter_map(|(provider, key)| {
            let provider = Provider::from_key_id(&provider)?;
            let key_id = provider.key_id()?;
            (!key.trim().is_empty()).then(|| (key_id.to_string(), key))
        })
        .collect()
}

#[cfg(test)]
mod tests;

use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;
use std::{fmt, fs, path::PathBuf};

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlideConfig {
    pub app: AppConfig,
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub providers: ProvidersConfig,
    pub dictation: DictationConfig,
    pub overlay: OverlayConfig,
    pub paste: PasteConfig,
}

impl Default for GlideConfig {
    fn default() -> Self {
        Self {
            app: AppConfig::default(),
            hotkey: HotkeyConfig::default(),
            audio: AudioConfig::default(),
            providers: ProvidersConfig::default(),
            dictation: DictationConfig::default(),
            overlay: OverlayConfig::default(),
            paste: PasteConfig::default(),
        }
    }
}

impl GlideConfig {
    /// Load config from confy + API keys from system keyring.
    pub fn load_or_create() -> Result<Self> {
        let mut config: Self =
            confy::load("glide", "config").unwrap_or_default();
        // Load API keys from the OS credential store (macOS Keychain)
        config.providers.openai.api_key = load_key_from_keyring("openai");
        config.providers.groq.api_key = load_key_from_keyring("groq");
        config.validate()?;
        Ok(config)
    }

    /// Save config via confy + API keys to system keyring.
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        #[cfg(not(test))]
        {
            confy::store("glide", "config", self)
                .map_err(|e| anyhow::anyhow!("failed to save config: {e}"))?;
            // Store API keys in the OS credential store
            save_key_to_keyring("openai", &self.providers.openai.api_key);
            save_key_to_keyring("groq", &self.providers.groq.api_key);
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

    /// Map a macOS virtual keycode to the best matching named trigger,
    /// falling back to `Custom(code)`.
    #[allow(dead_code)]
    pub fn from_keycode(code: u16) -> Self {
        match code {
            100 => Self::F8,
            101 => Self::F9,
            109 => Self::F10,
            _ => Self::Custom(code),
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
        63 => "Fn".to_string(),
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

/// Whether a macOS virtual keycode is a modifier key (fires flagsChanged, not keyDown/keyUp).
#[allow(dead_code)]
pub fn is_modifier_keycode(code: u16) -> bool {
    // 54=RCmd, 55=LCmd, 56=LShift, 57=CapsLock, 58=LOption, 59=LCtrl, 60=RShift, 61=ROption, 62=RCtrl, 63=Fn
    matches!(code, 54 | 55 | 56 | 57 | 58 | 59 | 60 | 61 | 62 | 63)
}

/// Return the CGEvent flag mask for a modifier keycode, used by the CGEventTap backend.
pub fn modifier_flag_for_keycode(code: u16) -> u64 {
    match code {
        54 | 55 => 0x00100000, // NSEventModifierFlagCommand
        56 | 60 => 0x00020000, // NSEventModifierFlagShift
        58 | 61 => 0x00080000, // NSEventModifierFlagOption
        59 | 62 => 0x00040000, // NSEventModifierFlagControl
        57 => 0x00010000,      // NSEventModifierFlagCapsLock
        63 => 0x00800000,      // NSEventModifierFlagFunction
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

// --- Unified provider system ---

/// A provider that can serve STT, LLM, or both.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    OpenAi,
    Groq,
}

impl Provider {
    #[allow(dead_code)]
    pub const ALL: [Self; 2] = [Self::OpenAi, Self::Groq];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI",
            Self::Groq => "Groq",
        }
    }

    pub fn logo(self) -> &'static str {
        match self {
            Self::OpenAi => "assets/icons/openai.png",
            Self::Groq => "assets/icons/groq.png",
        }
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::OpenAi => "https://api.openai.com/v1",
            Self::Groq => "https://api.groq.com/openai/v1",
        }
    }

    pub fn stt_endpoint(self, base: &str) -> String {
        format!("{}/audio/transcriptions", base.trim_end_matches('/'))
    }

    pub fn llm_endpoint(self, base: &str) -> String {
        format!("{}/chat/completions", base.trim_end_matches('/'))
    }

    pub fn from_model_info_provider(s: &str) -> Option<Self> {
        match s {
            "OpenAI" => Some(Self::OpenAi),
            "Groq" => Some(Self::Groq),
            _ => None,
        }
    }
}

/// Unified provider credentials — one entry per provider, shared across STT and LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    pub openai: ProviderCredentials,
    pub groq: ProviderCredentials,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            openai: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::OpenAi.default_base_url().to_string(),
            },
            groq: ProviderCredentials {
                api_key: String::new(),
                base_url: Provider::Groq.default_base_url().to_string(),
            },
        }
    }
}

impl ProvidersConfig {
    pub fn credentials_for(&self, provider: Provider) -> &ProviderCredentials {
        match provider {
            Provider::OpenAi => &self.openai,
            Provider::Groq => &self.groq,
        }
    }

    #[allow(dead_code)]
    pub fn credentials_for_mut(&mut self, provider: Provider) -> &mut ProviderCredentials {
        match provider {
            Provider::OpenAi => &mut self.openai,
            Provider::Groq => &mut self.groq,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderCredentials {
    #[serde(skip)] // API keys stored in OS keyring, not in config file
    pub api_key: String,
    pub base_url: String,
}

impl Default for ProviderCredentials {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: String::new(),
        }
    }
}

impl ProviderCredentials {
    pub fn resolve_api_key(&self, label: &str) -> Result<String> {
        if !self.api_key.trim().is_empty() {
            return Ok(self.api_key.trim().to_string());
        }
        anyhow::bail!("missing {label} API key; set it in Glide settings")
    }
}

// --- Unified dictation config ---

/// A provider + model pair. The fundamental unit of model selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: Provider,
    pub model: String,
}

/// All dictation-related settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DictationConfig {
    pub stt: ModelSelection,
    pub llm: Option<ModelSelection>,
    pub system_prompt: String,
    pub styles: Vec<Style>,
}

impl Default for DictationConfig {
    fn default() -> Self {
        Self {
            stt: ModelSelection {
                provider: Provider::OpenAi,
                model: "whisper-1".to_string(),
            },
            llm: None,
            system_prompt: "You are a dictation post-processor. You receive raw speech-to-text output and return clean text ready to be typed into an application.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning.\n- Fix spelling, grammar, and punctuation errors.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context or custom vocabulary, correct the spelling. Never insert names or terms from context that the speaker did not say.\n- Preserve the speaker's intent, tone, and meaning exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription. The context is only for correcting spelling of words already spoken.\n- Do not change the meaning of what was said.".to_string(),
            styles: vec![
                Style {
                    name: "Professional".to_string(),
                    apps: vec![],
                    prompt: "You are a dictation post-processor for professional communication. You receive raw speech-to-text output and return clean, formal text ready to be typed into a work application.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning.\n- Fix spelling, grammar, and punctuation errors.\n- Elevate the language to a professional, clear, and well-structured tone.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.\n- Preserve the speaker's intent and meaning exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription.\n- Do not change the meaning of what was said.".to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Messaging".to_string(),
                    apps: vec![],
                    prompt: "You are a dictation post-processor for casual messaging. You receive raw speech-to-text output and return clean, conversational text ready to be sent in a chat or text message.\n\nYour job:\n- Remove filler words (um, uh, you know, like) unless they carry meaning or add personality.\n- Fix obvious spelling and grammar errors, but keep the tone informal and natural.\n- Use casual punctuation\u{2014}lowercase is fine, fragments are OK.\n- When the transcript already contains a word that is a close misspelling of a name or term from the context, correct the spelling. Never insert names or terms the speaker did not say.\n- Preserve the speaker's voice and conversational style exactly.\n\nOutput rules:\n- Return ONLY the cleaned transcript text, nothing else.\n- If the transcription is empty, return exactly: EMPTY\n- Do not add words, names, or content that are not in the transcription.\n- Do not change the meaning of what was said.".to_string(),
                    stt: None,
                    llm: None,
                },
            ],
        }
    }
}

// --- Model info for dropdowns ---

// --- Model fetching and caching ---

use std::sync::Mutex;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub logo: String,
}

static CACHED_STT_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
static CACHED_LLM_MODELS: OnceLock<Mutex<Vec<ModelInfo>>> = OnceLock::new();
static PROVIDER_VERIFIED: OnceLock<Mutex<[bool; 2]>> = OnceLock::new();

/// Check whether a provider's API key has been verified via a successful /models fetch.
pub fn provider_verified(provider: Provider) -> bool {
    let cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
    let locked = cache.lock().unwrap();
    match provider {
        Provider::OpenAi => locked[0],
        Provider::Groq => locked[1],
    }
}

fn fallback_stt_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo { id: "whisper-1".into(), provider: "OpenAI".into(), logo: "assets/icons/openai.png".into() },
        ModelInfo { id: "whisper-large-v3".into(), provider: "Groq".into(), logo: "assets/icons/groq.png".into() },
        ModelInfo { id: "whisper-large-v3-turbo".into(), provider: "Groq".into(), logo: "assets/icons/groq.png".into() },
    ]
}

fn fallback_llm_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo { id: "gpt-4o-mini".into(), provider: "OpenAI".into(), logo: "assets/icons/openai.png".into() },
        ModelInfo { id: "gpt-4o".into(), provider: "OpenAI".into(), logo: "assets/icons/openai.png".into() },
        ModelInfo { id: "gpt-4-turbo".into(), provider: "OpenAI".into(), logo: "assets/icons/openai.png".into() },
        ModelInfo { id: "llama-3.3-70b-versatile".into(), provider: "Groq".into(), logo: "assets/icons/groq.png".into() },
        ModelInfo { id: "llama-3.1-8b-instant".into(), provider: "Groq".into(), logo: "assets/icons/groq.png".into() },
        ModelInfo { id: "mixtral-8x7b-32768".into(), provider: "Groq".into(), logo: "assets/icons/groq.png".into() },
    ]
}

/// Get cached STT models. Falls back to hardcoded list if API fetch hasn't completed.
pub fn cached_stt_models() -> Vec<ModelInfo> {
    let cache = CACHED_STT_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_stt_models()
    } else {
        locked.clone()
    }
}

/// Get cached LLM models. Falls back to hardcoded list if API fetch hasn't completed.
pub fn cached_llm_models() -> Vec<ModelInfo> {
    let cache = CACHED_LLM_MODELS.get_or_init(|| Mutex::new(Vec::new()));
    let locked = cache.lock().unwrap();
    if locked.is_empty() {
        fallback_llm_models()
    } else {
        locked.clone()
    }
}

#[derive(serde::Deserialize)]
struct ModelsResponse {
    data: Vec<ModelsResponseEntry>,
}

#[derive(serde::Deserialize)]
struct ModelsResponseEntry {
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    owned_by: String,
    #[serde(default)]
    active: Option<bool>,
}

/// Fetch model lists from all configured providers in a background thread.
pub fn fetch_all_models(providers: &ProvidersConfig) {
    let openai = providers.openai.clone();
    let groq = providers.groq.clone();

    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        let mut stt = Vec::new();
        let mut llm = Vec::new();

        // Fetch from each provider
        for (provider, creds) in [
            (Provider::OpenAi, &openai),
            (Provider::Groq, &groq),
        ] {
            let verified_cache_skip = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
            let skip_idx = match provider { Provider::OpenAi => 0, Provider::Groq => 1 };
            if creds.api_key.trim().is_empty() || creds.base_url.trim().is_empty() {
                verified_cache_skip.lock().unwrap()[skip_idx] = false;
                continue;
            }
            let url = format!("{}/models", creds.base_url.trim_end_matches('/'));
            let resp = client
                .get(&url)
                .bearer_auth(&creds.api_key)
                .send()
                .and_then(|r| r.json::<ModelsResponse>());

            let verified_cache = PROVIDER_VERIFIED.get_or_init(|| Mutex::new([false; 2]));
            let provider_idx = match provider { Provider::OpenAi => 0, Provider::Groq => 1 };

            if let Ok(resp) = resp {
                verified_cache.lock().unwrap()[provider_idx] = true;
                let logo = provider.logo().to_string();
                let label = provider.label().to_string();
                for entry in resp.data {
                    // Skip inactive models (Groq provides this field)
                    if entry.active == Some(false) {
                        continue;
                    }

                    let id = &entry.id;
                    let id_lower = id.to_lowercase();

                    let is_stt = id_lower.contains("whisper")
                        || id_lower.contains("distil-whisper");

                    let info = ModelInfo {
                        id: id.clone(),
                        provider: label.clone(),
                        logo: logo.clone(),
                    };

                    if is_stt {
                        stt.push(info);
                    } else {
                        // Exclude non-chat model types
                        let excluded = id_lower.contains("embedding")
                            || id_lower.contains("tts")
                            || id_lower.contains("dall-e")
                            || id_lower.contains("moderation")
                            || id_lower.starts_with("ft:")
                            || id_lower.contains("realtime")
                            || id_lower.contains("-audio-")
                            || id_lower.contains("davinci")
                            || id_lower.contains("babbage")
                            || id_lower.contains("canary")
                            || id_lower.contains("search")
                            || id_lower.contains("similarity")
                            || id_lower.starts_with("text-")
                            || id_lower.starts_with("code-")
                            || id_lower.contains("omni-")
                            || id_lower.contains("orpheus");
                        if !excluded {
                            llm.push(info);
                        }
                    }
                }
            } else {
                verified_cache.lock().unwrap()[provider_idx] = false;
            }
        }

        // Sort by provider then model id
        stt.sort_by(|a, b| (&a.provider, &a.id).cmp(&(&b.provider, &b.id)));
        llm.sort_by(|a, b| (&a.provider, &a.id).cmp(&(&b.provider, &b.id)));

        if !stt.is_empty() {
            let cache = CACHED_STT_MODELS.get_or_init(|| Mutex::new(Vec::new()));
            *cache.lock().unwrap() = stt;
        }
        if !llm.is_empty() {
            let cache = CACHED_LLM_MODELS.get_or_init(|| Mutex::new(Vec::new()));
            *cache.lock().unwrap() = llm;
        }
    });
}

/// A dictation style with a name, assigned apps, a custom system prompt,
/// and optional model overrides.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub name: String,
    #[serde(default)]
    pub apps: Vec<String>,
    pub prompt: String,
    #[serde(default)]
    pub stt: Option<ModelSelection>,
    #[serde(default)]
    pub llm: Option<ModelSelection>,
}

/// List application names from /Applications (macOS).
pub fn list_applications() -> Vec<String> {
    let mut apps: Vec<String> = std::fs::read_dir("/Applications")
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().map(|e| e == "app").unwrap_or(false) {
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();
    apps.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    apps
}

// --- App icon extraction via NSWorkspace ---

#[link(name = "AppKit", kind = "framework")]
#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

// Objective-C runtime FFI — objc_msgSend is NOT variadic on ARM64.
// We declare the base 2-arg form and transmute to typed function pointers
// for calls with extra arguments to get the correct register-based ABI.
unsafe extern "C" {
    fn objc_getClass(name: *const u8) -> *mut c_void;
    fn sel_registerName(name: *const u8) -> *mut c_void;
    fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void) -> *mut c_void;
}

// Typed function pointer aliases for objc_msgSend with extra args
type MsgSendPtr = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void;
type MsgSendUsize = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut c_void) -> *mut c_void;
type MsgSendLen = unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize;

static ICON_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();

fn icon_cache_dir() -> &'static Path {
    ICON_CACHE_DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join("glide-icons");
        let _ = fs::create_dir_all(&dir);
        dir
    })
}

/// Get the cached PNG icon path for an app. Returns None if not yet cached.
/// Icons are extracted in the background by `preload_app_icons()`.
pub fn app_icon_path(app_name: &str) -> Option<PathBuf> {
    let png_path = icon_cache_dir().join(format!("{app_name}.png"));
    if png_path.exists() {
        Some(png_path)
    } else {
        None
    }
}

/// Pre-extract all app icons on a background thread so the UI never blocks.
pub fn preload_app_icons() {
    std::thread::spawn(|| {
        let apps = list_applications();
        for app in &apps {
            let png_path = icon_cache_dir().join(format!("{app}.png"));
            if png_path.exists() {
                continue;
            }
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = extract_icon_to_png(app, &png_path);
            }));
        }
    });
}

/// Use NSWorkspace to extract an app's icon and save as PNG.
fn extract_icon_to_png(app_name: &str, dest: &Path) -> Result<()> {
    // All objc_msgSend calls with extra args must use typed function pointers
    // (not variadic) to get the correct ARM64 register-based calling convention.
    let msg1: MsgSendPtr = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let msg_usize: MsgSendUsize =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let msg_len: MsgSendLen =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };

    unsafe {
        let workspace_class = objc_getClass(b"NSWorkspace\0".as_ptr());
        if workspace_class.is_null() {
            anyhow::bail!("NSWorkspace class not found");
        }
        let shared_sel = sel_registerName(b"sharedWorkspace\0".as_ptr());
        let workspace = objc_msgSend(workspace_class, shared_sel);
        if workspace.is_null() {
            anyhow::bail!("failed to get NSWorkspace");
        }

        // Build NSString for the app path using CString for proper null termination
        let app_path =
            std::ffi::CString::new(format!("/Applications/{app_name}.app"))
                .context("invalid app name")?;
        let nsstring_class = objc_getClass(b"NSString\0".as_ptr());
        let string_sel =
            sel_registerName(b"stringWithUTF8String:\0".as_ptr());
        let ns_path =
            msg1(nsstring_class, string_sel, app_path.as_ptr() as *mut c_void);
        if ns_path.is_null() {
            anyhow::bail!("failed to create NSString");
        }

        // [workspace iconForFile:path]
        let icon_sel = sel_registerName(b"iconForFile:\0".as_ptr());
        let icon = msg1(workspace, icon_sel, ns_path);
        if icon.is_null() {
            anyhow::bail!("failed to get icon");
        }

        // [icon TIFFRepresentation]
        let tiff_sel = sel_registerName(b"TIFFRepresentation\0".as_ptr());
        let tiff_data = objc_msgSend(icon, tiff_sel);
        if tiff_data.is_null() {
            anyhow::bail!("failed to get TIFF data");
        }

        // [NSBitmapImageRep imageRepWithData:tiff]
        let rep_class = objc_getClass(b"NSBitmapImageRep\0".as_ptr());
        if rep_class.is_null() {
            anyhow::bail!("NSBitmapImageRep class not found");
        }
        let rep_sel = sel_registerName(b"imageRepWithData:\0".as_ptr());
        let rep = msg1(rep_class, rep_sel, tiff_data);
        if rep.is_null() {
            anyhow::bail!("failed to create bitmap rep");
        }

        // [rep representationUsingType:NSBitmapImageFileTypePNG properties:@{}]
        let png_sel = sel_registerName(
            b"representationUsingType:properties:\0".as_ptr(),
        );
        let dict_class = objc_getClass(b"NSDictionary\0".as_ptr());
        let empty_dict_sel = sel_registerName(b"dictionary\0".as_ptr());
        let empty_dict = objc_msgSend(dict_class, empty_dict_sel);
        let png_data = msg_usize(rep, png_sel, 4 /* PNG */, empty_dict);
        if png_data.is_null() {
            anyhow::bail!("failed to create PNG data");
        }

        // Read bytes from NSData
        let bytes_sel = sel_registerName(b"bytes\0".as_ptr());
        let length_sel = sel_registerName(b"length\0".as_ptr());
        let bytes_ptr = objc_msgSend(png_data, bytes_sel) as *const u8;
        let length = msg_len(png_data, length_sel);

        if bytes_ptr.is_null() || length == 0 {
            anyhow::bail!("empty PNG data");
        }

        let bytes = std::slice::from_raw_parts(bytes_ptr, length);
        fs::write(dest, bytes)
            .with_context(|| format!("failed to write icon to {}", dest.display()))?;

        Ok(())
    }
}

/// Fuzzy subsequence match. Returns a score if all query chars appear in order
/// in the candidate (case-insensitive). Higher score = better match.
pub fn fuzzy_match(query: &str, candidate: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    let query_lower = query.to_lowercase();
    let candidate_lower = candidate.to_lowercase();
    let mut score = 0i32;
    let mut qi = query_lower.chars().peekable();
    for (i, c) in candidate_lower.chars().enumerate() {
        if qi.peek() == Some(&c) {
            qi.next();
            score += 100 - i as i32;
        }
    }
    if qi.peek().is_none() {
        Some(score)
    } else {
        None
    }
}

// --- Frontmost app detection via NSWorkspace ---

/// Get the name of the currently frontmost (active) application.
pub fn frontmost_app_name() -> Option<String> {
    let msg1: MsgSendPtr = unsafe { std::mem::transmute(objc_msgSend as *const ()) };

    unsafe {
        let workspace_class = objc_getClass(b"NSWorkspace\0".as_ptr());
        if workspace_class.is_null() {
            return None;
        }
        let shared_sel = sel_registerName(b"sharedWorkspace\0".as_ptr());
        let workspace = objc_msgSend(workspace_class, shared_sel);
        if workspace.is_null() {
            return None;
        }

        let frontmost_sel = sel_registerName(b"frontmostApplication\0".as_ptr());
        let app = objc_msgSend(workspace, frontmost_sel);
        if app.is_null() {
            return None;
        }

        let name_sel = sel_registerName(b"localizedName\0".as_ptr());
        let ns_name = objc_msgSend(app, name_sel);
        if ns_name.is_null() {
            return None;
        }

        let utf8_sel = sel_registerName(b"UTF8String\0".as_ptr());
        let cstr_ptr = msg1(ns_name, utf8_sel, std::ptr::null_mut()) as *const i8;
        if cstr_ptr.is_null() {
            return None;
        }

        let name = std::ffi::CStr::from_ptr(cstr_ptr).to_string_lossy().into_owned();
        Some(name)
    }
}

// --- Screen size FFI for overlay positioning ---

unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
}

/// Get the main display resolution in pixels.
pub fn main_display_size() -> (usize, usize) {
    unsafe {
        let display = CGMainDisplayID();
        (CGDisplayPixelsWide(display), CGDisplayPixelsHigh(display))
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

// --- Keyring helpers for secure API key storage ---

fn load_key_from_keyring(provider: &str) -> String {
    keyring::Entry::new(&format!("glide-{provider}"), "api-key")
        .and_then(|e| e.get_password())
        .unwrap_or_default()
}

fn save_key_to_keyring(provider: &str, key: &str) {
    if let Ok(entry) = keyring::Entry::new(&format!("glide-{provider}"), "api-key") {
        // Only write to keyring if the value actually changed — avoids macOS password prompts
        let current = entry.get_password().unwrap_or_default();
        if current == key {
            return;
        }
        if key.trim().is_empty() {
            let _ = entry.delete_credential();
        } else {
            let _ = entry.set_password(key);
        }
    }
}

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
    fn test_hotkey_from_keycode() {
        assert_eq!(HotkeyTrigger::from_keycode(100), HotkeyTrigger::F8);
        assert_eq!(HotkeyTrigger::from_keycode(58), HotkeyTrigger::Custom(58)); // Left Option
        assert_eq!(HotkeyTrigger::from_keycode(61), HotkeyTrigger::Custom(61)); // Right Option
        assert_eq!(HotkeyTrigger::from_keycode(55), HotkeyTrigger::Custom(55)); // Left Cmd
        assert_eq!(HotkeyTrigger::from_keycode(54), HotkeyTrigger::Custom(54)); // Right Cmd
        assert_eq!(HotkeyTrigger::from_keycode(63), HotkeyTrigger::Custom(63)); // Fn
        assert_eq!(HotkeyTrigger::from_keycode(49), HotkeyTrigger::Custom(49)); // Space
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
    fn test_provider_variants() {
        assert_eq!(Provider::ALL.len(), 2);
        assert_eq!(Provider::OpenAi.label(), "OpenAI");
        assert_eq!(Provider::Groq.label(), "Groq");
        assert!(!Provider::OpenAi.default_base_url().is_empty());
    }

    #[test]
    fn test_resolve_api_key_from_credentials() {
        let mut creds = ProviderCredentials::default();
        creds.api_key = "direct-key".to_string();

        let resolved = creds.resolve_api_key("test").unwrap();
        assert_eq!(resolved, "direct-key");
    }

    #[test]
    fn test_resolve_api_key_fails_when_missing() {
        let creds = ProviderCredentials::default();
        assert!(creds.resolve_api_key("test").is_err());
    }
}

use super::*;
use std::{collections::BTreeMap, fs};

fn assert_invalid_config(name: &str, mutate: impl FnOnce(&mut GlideConfig)) {
    let mut config = GlideConfig::default();
    mutate(&mut config);
    assert!(config.validate().is_err(), "{name} should be invalid");
}

#[test]
fn default_config_is_valid() {
    GlideConfig::default().validate().unwrap();
}

#[test]
fn validation_rejects_invalid_numeric_fields() {
    assert_invalid_config("zero sample rate", |config| config.audio.sample_rate = 0);
    assert_invalid_config("zero channels", |config| config.audio.channels = 0);
    assert_invalid_config("zero overlay width", |config| config.overlay.width = 0);
    assert_invalid_config("zero overlay height", |config| config.overlay.height = 0);
    assert_invalid_config("opacity above range", |config| config.overlay.opacity = 2.0);
    assert_invalid_config("opacity below range", |config| {
        config.overlay.opacity = -0.1
    });
}

#[test]
fn serde_roundtrip_is_stable() {
    let config = GlideConfig::default();
    let serialized = toml::to_string_pretty(&config).unwrap();
    let parsed: GlideConfig = toml::from_str(&serialized).unwrap();
    let reserialized = toml::to_string_pretty(&parsed).unwrap();
    assert_eq!(serialized, reserialized);
}

#[test]
fn provider_key_payload_roundtrips_known_remote_keys() {
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
fn provider_key_payload_filters_empty_and_unknown_keys() {
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
fn provider_key_payload_accepts_plain_map_shape() {
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
fn hotkey_trigger_labels_are_exact() {
    let cases = [
        (HotkeyTrigger::Option, "⌥ Option"),
        (HotkeyTrigger::CommandRight, "⌘ Right Cmd"),
        (HotkeyTrigger::F8, "F8"),
        (HotkeyTrigger::F9, "F9"),
        (HotkeyTrigger::F10, "F10"),
        (HotkeyTrigger::Custom(49), "Space"),
        (HotkeyTrigger::Custom(61), "⌥ Right Option"),
    ];

    for (trigger, label) in cases {
        assert_eq!(trigger.label(), label);
    }
}

#[test]
fn hotkey_from_keycode_only_special_cases_function_keys() {
    let cases = [
        (100, HotkeyTrigger::F8),
        (101, HotkeyTrigger::F9),
        (109, HotkeyTrigger::F10),
        (58, HotkeyTrigger::Custom(58)),
        (61, HotkeyTrigger::Custom(61)),
        (55, HotkeyTrigger::Custom(55)),
        (54, HotkeyTrigger::Custom(54)),
        (63, HotkeyTrigger::Custom(63)),
        (49, HotkeyTrigger::Custom(49)),
    ];

    for (keycode, trigger) in cases {
        assert_eq!(HotkeyTrigger::from_keycode(keycode), trigger);
    }
}

#[test]
fn overlay_and_theme_labels_are_exact() {
    assert_eq!(OverlayStyle::Classic.label(), "Classic");
    assert_eq!(OverlayStyle::Glow.label(), "Glow");
    assert_eq!(OverlayStyle::None.label(), "None");

    let themes = [
        (ThemePreference::System, "System"),
        (ThemePreference::Light, "Light"),
        (ThemePreference::Dark, "Dark"),
    ];
    for (theme, label) in themes {
        assert_eq!(theme.label(), label);
    }
}

#[test]
fn config_toml_file_shape_matches_serialized_defaults() {
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

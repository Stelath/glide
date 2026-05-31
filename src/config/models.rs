use serde::{Deserialize, Serialize};

use super::providers::Provider;

const DEFAULT_PROMPT: &str = include_str!("prompts/default.md");
const PROFESSIONAL_PROMPT: &str = include_str!("prompts/professional.md");
const MESSAGING_PROMPT: &str = include_str!("prompts/messaging.md");
const CODING_PROMPT: &str = include_str!("prompts/coding.md");
pub const STYLE_PROMPT_PLACEHOLDER: &str = "{{STYLE}}";

const LEGACY_DEFAULT_PROMPT_HASHES: &[u64] = &[0xc209_be5b_8876_64a4];
const LEGACY_PROFESSIONAL_PROMPT_HASHES: &[u64] = &[0x7778_211a_d268_2d40];
const LEGACY_MESSAGING_PROMPT_HASHES: &[u64] = &[0xc156_ddf1_366f_599f];
const LEGACY_CODING_PROMPT_HASHES: &[u64] = &[0x81ba_1e2c_9520_f6f3];
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSelection {
    pub provider: Provider,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DictationConfig {
    pub stt: ModelSelection,
    pub llm: Option<ModelSelection>,
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub system_prompt_uses_default: bool,
    pub styles: Vec<Style>,
    #[serde(default)]
    pub smart_defaults_applied: bool,
}

impl Default for DictationConfig {
    fn default() -> Self {
        Self {
            stt: ModelSelection {
                provider: Provider::OpenAi,
                model: "whisper-1".to_string(),
            },
            llm: None,
            smart_defaults_applied: false,
            system_prompt: DEFAULT_PROMPT.trim_end().to_string(),
            system_prompt_uses_default: true,
            styles: vec![
                Style {
                    name: "Professional".to_string(),
                    apps: vec![],
                    prompt: PROFESSIONAL_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Messaging".to_string(),
                    apps: vec![],
                    prompt: MESSAGING_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
                Style {
                    name: "Coding".to_string(),
                    apps: vec![],
                    prompt: CODING_PROMPT.trim_end().to_string(),
                    stt: None,
                    llm: None,
                },
            ],
        }
    }
}

impl DictationConfig {
    pub fn default_system_prompt() -> &'static str {
        DEFAULT_PROMPT.trim_end()
    }

    pub fn sync_system_prompt_default_flag(&mut self) {
        self.system_prompt_uses_default =
            normalized_prompt(&self.system_prompt) == Self::default_system_prompt();
    }

    pub fn refresh_builtin_prompt_defaults(&mut self) {
        let prompt_matches_known_default = prompt_matches_current_or_legacy(
            &self.system_prompt,
            Self::default_system_prompt(),
            LEGACY_DEFAULT_PROMPT_HASHES,
        );

        if self.system_prompt_uses_default || prompt_matches_known_default {
            if prompt_matches_known_default {
                self.system_prompt = Self::default_system_prompt().to_string();
                self.system_prompt_uses_default = true;
            } else {
                self.system_prompt_uses_default = false;
            }
        }

        for style in &mut self.styles {
            if let Some(default_prompt) =
                default_style_prompt_if_unedited(&style.name, &style.prompt)
            {
                style.prompt = default_prompt.to_string();
            }
        }
    }
}

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

// --- Dictionary config ---

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct DictionaryConfig {
    pub vocabulary: Vec<String>,
    pub replacements: Vec<ReplacementRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplacementRule {
    pub find: String,
    pub replace: String,
    #[serde(default)]
    pub case_sensitive: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn normalized_prompt(prompt: &str) -> &str {
    prompt.trim_end()
}

fn prompt_matches_current_or_legacy(
    prompt: &str,
    current: &'static str,
    legacy_hashes: &[u64],
) -> bool {
    let prompt = normalized_prompt(prompt);
    prompt == normalized_prompt(current) || legacy_hashes.contains(&prompt_hash(prompt))
}

fn prompt_hash(prompt: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in normalized_prompt(prompt).bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn default_style_prompt_if_unedited(name: &str, prompt: &str) -> Option<&'static str> {
    let (current, legacy_hashes) = match name {
        "Professional" => (
            PROFESSIONAL_PROMPT.trim_end(),
            LEGACY_PROFESSIONAL_PROMPT_HASHES,
        ),
        "Messaging" => (MESSAGING_PROMPT.trim_end(), LEGACY_MESSAGING_PROMPT_HASHES),
        "Coding" => (CODING_PROMPT.trim_end(), LEGACY_CODING_PROMPT_HASHES),
        _ => return None,
    };

    prompt_matches_current_or_legacy(prompt, current, legacy_hashes).then_some(current)
}

#[cfg(test)]
mod tests {
    use super::{DictationConfig, STYLE_PROMPT_PLACEHOLDER};

    #[test]
    fn default_prompt_contains_cleanup_contract_and_style_placeholder() {
        let config = DictationConfig::default();
        assert!(config.system_prompt_uses_default);
        assert!(config.system_prompt.contains("CORE TASK:"));
        assert!(config.system_prompt.contains(STYLE_PROMPT_PLACEHOLDER));
        assert!(
            config
                .system_prompt
                .contains("Preserve spoken questions as questions")
        );

        for style in &config.styles {
            assert!(!style.prompt.contains("CORE TASK:"), "{} style", style.name);
            assert!(
                !style.prompt.contains("raw transcript"),
                "{} style",
                style.name
            );
            assert!(
                style.prompt.len() < 400,
                "{} style should be short",
                style.name
            );
        }
    }

    #[test]
    fn refresh_builtin_prompt_defaults_preserves_custom_prompt() {
        let mut config = DictationConfig::default();
        config.system_prompt = "custom prompt".to_string();
        config.system_prompt_uses_default = true;

        config.refresh_builtin_prompt_defaults();

        assert_eq!(config.system_prompt, "custom prompt");
        assert!(!config.system_prompt_uses_default);
    }

    #[test]
    fn sync_system_prompt_default_flag_tracks_current_default() {
        let mut config = DictationConfig::default();
        config.system_prompt = "custom prompt".to_string();
        config.sync_system_prompt_default_flag();
        assert!(!config.system_prompt_uses_default);

        config.system_prompt = DictationConfig::default_system_prompt().to_string();
        config.sync_system_prompt_default_flag();
        assert!(config.system_prompt_uses_default);
    }
}

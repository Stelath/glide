use serde::{Deserialize, Serialize};

use super::providers::Provider;

const DEFAULT_PROMPT: &str = include_str!("prompts/default.md");
const PROFESSIONAL_PROMPT: &str = include_str!("prompts/professional.md");
const MESSAGING_PROMPT: &str = include_str!("prompts/messaging.md");
const CODING_PROMPT: &str = include_str!("prompts/coding.md");

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

// ---------------------------------------------------------------------------
// Dictionary config
// ---------------------------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::DictationConfig;

    #[test]
    fn default_style_prompts_preserve_dictated_questions() {
        let config = DictationConfig::default();
        assert!(
            config
                .system_prompt
                .contains("Preserve questions the speaker dictated")
        );
        assert!(
            !config
                .system_prompt
                .contains("- No questions. No suggestions. No added content.")
        );

        for style in &config.styles {
            assert!(
                style
                    .prompt
                    .contains("Preserve questions the speaker dictated"),
                "{} style should preserve dictated questions",
                style.name
            );
            assert!(
                !style
                    .prompt
                    .contains("- No questions. No suggestions. No added content."),
                "{} style should not broadly forbid questions",
                style.name
            );
        }
    }
}

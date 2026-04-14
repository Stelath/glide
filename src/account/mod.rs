//! User accounts and sign-in for the macOS app.
//!
//! Profile non-secrets live on `GlideConfig.account` (confy → config.toml);
//! token secrets live in the macOS Keychain via the `keyring` crate.

pub mod google;
pub mod oauth;
pub mod router;
pub mod apple;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthProvider {
    Apple,
    Google,
}

impl AuthProvider {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Apple => "Apple",
            Self::Google => "Google",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SubscriptionTier {
    #[default]
    None,
    Free,
    Pro,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub transcription_seconds: u64,
    pub llm_tokens_in: u64,
    pub llm_tokens_out: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub provider: AuthProvider,
    pub stable_id: String,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub issued_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub subscription_tier: SubscriptionTier,
    #[serde(default)]
    pub token_usage: TokenUsage,
}

impl Account {
    pub fn keyring_account(&self) -> String {
        format!("{}:{}", provider_slug(self.provider), self.stable_id)
    }

    pub fn short_display_name(&self) -> String {
        if let Some(name) = self.display_name.as_deref().filter(|s| !s.is_empty()) {
            return name.to_string();
        }
        if let Some(email) = self.email.as_deref() {
            if let Some(at) = email.find('@') {
                return email[..at].to_string();
            }
            return email.to_string();
        }
        "Account".to_string()
    }

    pub fn initials(&self) -> String {
        let source = self
            .display_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(self.email.as_deref())
            .unwrap_or("?");

        let letters: String = source
            .split(|c: char| !c.is_alphabetic())
            .filter(|s| !s.is_empty())
            .take(2)
            .filter_map(|s| s.chars().next())
            .collect::<String>()
            .to_uppercase();

        if letters.is_empty() { "?".to_string() } else { letters }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AccountState {
    pub current: Option<Account>,
    pub has_seen_welcome: bool,
}

#[derive(Debug, Clone)]
pub struct AccountTokens {
    pub id_token: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

const ID_TOKEN_SERVICE: &str = "glide-account-idtoken";
const ACCESS_TOKEN_SERVICE: &str = "glide-account-accesstoken";
const REFRESH_TOKEN_SERVICE: &str = "glide-account-refreshtoken";

fn provider_slug(provider: AuthProvider) -> &'static str {
    match provider {
        AuthProvider::Apple => "apple",
        AuthProvider::Google => "google",
    }
}

pub fn save_tokens(account: &Account, tokens: &AccountTokens) -> Result<()> {
    let keyring_account = account.keyring_account();
    write_secret(ID_TOKEN_SERVICE, &keyring_account, &tokens.id_token)?;
    let _ = write_secret(
        ACCESS_TOKEN_SERVICE,
        &keyring_account,
        tokens.access_token.as_deref().unwrap_or(""),
    );
    let _ = write_secret(
        REFRESH_TOKEN_SERVICE,
        &keyring_account,
        tokens.refresh_token.as_deref().unwrap_or(""),
    );
    Ok(())
}

#[allow(dead_code)]
pub fn load_tokens(account: &Account) -> Result<Option<AccountTokens>> {
    let keyring_account = account.keyring_account();
    let id_token = read_secret(ID_TOKEN_SERVICE, &keyring_account)?;
    if id_token.is_empty() {
        return Ok(None);
    }
    let access_token = read_secret(ACCESS_TOKEN_SERVICE, &keyring_account)
        .ok()
        .filter(|s| !s.is_empty());
    let refresh_token = read_secret(REFRESH_TOKEN_SERVICE, &keyring_account)
        .ok()
        .filter(|s| !s.is_empty());
    Ok(Some(AccountTokens {
        id_token,
        access_token,
        refresh_token,
    }))
}

pub fn clear_tokens(account: &Account) -> Result<()> {
    let keyring_account = account.keyring_account();
    let _ = delete_secret(ID_TOKEN_SERVICE, &keyring_account);
    let _ = delete_secret(ACCESS_TOKEN_SERVICE, &keyring_account);
    let _ = delete_secret(REFRESH_TOKEN_SERVICE, &keyring_account);
    Ok(())
}

fn write_secret(service: &str, account: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, account).context("failed to create keyring entry")?;
    if value.is_empty() {
        let _ = entry.delete_credential();
        return Ok(());
    }
    if let Ok(current) = entry.get_password() {
        if current == value {
            return Ok(());
        }
    }
    entry
        .set_password(value)
        .context("failed to write keyring secret")?;
    Ok(())
}

#[allow(dead_code)]
fn read_secret(service: &str, account: &str) -> Result<String> {
    let entry = keyring::Entry::new(service, account).context("failed to create keyring entry")?;
    match entry.get_password() {
        Ok(value) => Ok(value),
        Err(keyring::Error::NoEntry) => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

fn delete_secret(service: &str, account: &str) -> Result<()> {
    let entry = keyring::Entry::new(service, account).context("failed to create keyring entry")?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_account() -> Account {
        Account {
            provider: AuthProvider::Google,
            stable_id: "1234567890".to_string(),
            email: Some("ada@example.com".to_string()),
            display_name: Some("Ada Lovelace".to_string()),
            avatar_url: None,
            issued_at: Utc::now(),
            expires_at: None,
            subscription_tier: SubscriptionTier::None,
            token_usage: TokenUsage::default(),
        }
    }

    #[test]
    fn test_account_serde_roundtrip() {
        let account = sample_account();
        let json = serde_json::to_string(&account).unwrap();
        let parsed: Account = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.stable_id, account.stable_id);
        assert_eq!(parsed.email, account.email);
        assert_eq!(parsed.subscription_tier, SubscriptionTier::None);
    }

    #[test]
    fn test_short_display_name_prefers_display_name() {
        let account = sample_account();
        assert_eq!(account.short_display_name(), "Ada Lovelace");
    }

    #[test]
    fn test_short_display_name_falls_back_to_email_local_part() {
        let mut account = sample_account();
        account.display_name = None;
        assert_eq!(account.short_display_name(), "ada");
    }

    #[test]
    fn test_initials_from_display_name() {
        let account = sample_account();
        assert_eq!(account.initials(), "AL");
    }

    #[test]
    fn test_initials_single_name() {
        let mut account = sample_account();
        account.display_name = Some("Ada".to_string());
        assert_eq!(account.initials(), "A");
    }

    #[test]
    fn test_initials_handles_missing_everything() {
        let mut account = sample_account();
        account.display_name = None;
        account.email = None;
        assert_eq!(account.initials(), "?");
    }

    #[test]
    fn test_keyring_account_is_stable_and_includes_provider() {
        let account = sample_account();
        assert_eq!(account.keyring_account(), "google:1234567890");
    }

    #[test]
    fn test_account_state_default_is_guest() {
        let state = AccountState::default();
        assert!(state.current.is_none());
        assert!(!state.has_seen_welcome);
    }

    #[test]
    fn test_subscription_tier_default_is_none() {
        assert_eq!(SubscriptionTier::default(), SubscriptionTier::None);
    }
}

//! Google sign in for the macOS app — OAuth 2.0 + PKCE via the system browser.
//!
//! Uses Google's **desktop** OAuth client type which accepts `http://127.0.0.1:<port>`
//! redirect URIs. Desktop clients do have a `client_secret`, but per Google's
//! documentation it is not actually secret in this flow; the real security
//! comes from PKCE and the loopback redirect binding to an ephemeral port
//! that only the real user process can open.
//!
//! No Google SDK, no service account keys, no proxying through our backend.

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::Serialize;

use super::oauth::{
    CallbackParams, LoopbackServer, PkcePair, TokenResponse, decode_id_token,
    generate_pkce, generate_state, open_browser,
};
use super::{Account, AccountTokens, AuthProvider, SubscriptionTier, TokenUsage};

/// Configuration for the Google desktop OAuth client.
///
/// Provision both values in Google Cloud Console under
/// "APIs & Services → Credentials → Create OAuth client ID → Desktop app".
///
/// While both are empty strings the Google button is disabled in the UI —
/// Apple sign in (when available) and Guest still work without any config.
pub mod config {
    pub const CLIENT_ID: &str = "";
    pub const CLIENT_SECRET: &str = "";
    pub const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
    pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
    pub const SCOPES: &[&str] = &["openid", "email", "profile"];

    pub fn is_configured() -> bool {
        !CLIENT_ID.is_empty()
    }
}

pub async fn sign_in() -> Result<(Account, AccountTokens)> {
    if !config::is_configured() {
        return Err(anyhow!(
            "Google sign in is not configured in this build — set CLIENT_ID in src/account/google.rs"
        ));
    }

    // Step 1: Bind the loopback listener so we know the port *before* building
    // the authorize URL (the redirect_uri must match byte-for-byte).
    let server = LoopbackServer::start()?;
    let redirect_uri = server.redirect_uri();

    // Step 2: Generate PKCE + state.
    let pkce = generate_pkce();
    let state = generate_state();

    // Step 3: Build + open the authorize URL.
    let authorize_url = build_authorize_url(&redirect_uri, &pkce, &state);
    open_browser(&authorize_url)?;

    // Step 4: Wait for the redirect on a blocking thread.
    // 2-minute ceiling leaves plenty of time for the user to sign in but
    // won't hang forever.
    let callback = tokio::task::spawn_blocking(move || {
        server.wait_for_callback(Duration::from_secs(120))
    })
    .await
    .context("loopback task panicked")??;

    verify_state(&callback, &state)?;

    // Step 5: Exchange the code for tokens.
    let tokens = exchange_code(&callback.code, &pkce, &redirect_uri).await?;

    // Step 6: Decode the id_token JWT to pull profile claims.
    let claims = decode_id_token(&tokens.id_token)?;

    let now = Utc::now();
    let expires_at = tokens
        .expires_in
        .map(|secs| now + chrono::Duration::seconds(secs));
    let account = Account {
        provider: AuthProvider::Google,
        stable_id: claims.sub,
        email: claims.email,
        display_name: claims.name,
        avatar_url: claims.picture,
        issued_at: now,
        expires_at,
        subscription_tier: SubscriptionTier::None,
        token_usage: TokenUsage::default(),
    };
    let bundle = AccountTokens {
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    };

    Ok((account, bundle))
}

fn build_authorize_url(redirect_uri: &str, pkce: &PkcePair, state: &str) -> String {
    let mut url = url::Url::parse(config::AUTHORIZATION_ENDPOINT)
        .expect("authorization endpoint must parse");
    url.query_pairs_mut()
        .append_pair("client_id", config::CLIENT_ID)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &config::SCOPES.join(" "))
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "select_account");
    url.to_string()
}

fn verify_state(callback: &CallbackParams, expected: &str) -> Result<()> {
    match callback.state.as_deref() {
        Some(state) if state == expected => Ok(()),
        Some(_) => Err(anyhow!("OAuth state parameter did not match — possible CSRF")),
        None => Err(anyhow!("OAuth callback missing state parameter")),
    }
}

#[derive(Debug, Serialize)]
struct TokenExchangeRequest<'a> {
    client_id: &'a str,
    client_secret: &'a str,
    code: &'a str,
    code_verifier: &'a str,
    grant_type: &'a str,
    redirect_uri: &'a str,
}

async fn exchange_code(code: &str, pkce: &PkcePair, redirect_uri: &str) -> Result<TokenResponse> {
    let client = reqwest::Client::new();
    let body = TokenExchangeRequest {
        client_id: config::CLIENT_ID,
        client_secret: config::CLIENT_SECRET,
        code,
        code_verifier: &pkce.verifier,
        grant_type: "authorization_code",
        redirect_uri,
    };
    let response = client
        .post(config::TOKEN_ENDPOINT)
        .form(&body)
        .send()
        .await
        .context("failed to call Google token endpoint")?;

    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read Google token response body")?;

    if !status.is_success() {
        return Err(anyhow!(
            "Google token endpoint returned {status}: {}",
            text.chars().take(500).collect::<String>()
        ));
    }

    let parsed: TokenResponse =
        serde_json::from_str(&text).context("failed to parse Google token response")?;
    Ok(parsed)
}

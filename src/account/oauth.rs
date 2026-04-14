//! Desktop OAuth 2.0 Authorization-Code + PKCE helpers shared between providers.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};

pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

pub fn generate_pkce() -> PkcePair {
    let verifier = random_url_safe(64);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    PkcePair { verifier, challenge }
}

pub fn generate_state() -> String {
    random_url_safe(32)
}

fn random_url_safe(byte_count: usize) -> String {
    let mut bytes = vec![0u8; byte_count];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[derive(Debug, Clone)]
pub struct CallbackParams {
    pub code: String,
    pub state: Option<String>,
}

pub struct LoopbackServer {
    server: tiny_http::Server,
    port: u16,
}

impl LoopbackServer {
    pub fn start() -> Result<Self> {
        let server = tiny_http::Server::http("127.0.0.1:0")
            .map_err(|e| anyhow!("failed to start loopback server: {e}"))?;
        let port = server
            .server_addr()
            .to_ip()
            .ok_or_else(|| anyhow!("loopback server has no IP address"))?
            .port();
        Ok(Self { server, port })
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/oauth2redirect", self.port)
    }

    pub fn wait_for_callback(self, timeout: Duration) -> Result<CallbackParams> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                bail!("sign in timed out waiting for browser redirect");
            }
            let request = match self
                .server
                .recv_timeout(remaining)
                .context("failed to receive callback request")?
            {
                Some(request) => request,
                None => bail!("sign in timed out waiting for browser redirect"),
            };

            let url = request.url().to_string();
            let parsed = parse_callback_query(&url);

            let (status, body) = match &parsed {
                Ok(_) => (200, success_html().to_string()),
                Err(e) => (400, error_html(&e.to_string())),
            };
            let response = tiny_http::Response::from_string(body)
                .with_status_code(status)
                .with_header(
                    "Content-Type: text/html; charset=utf-8"
                        .parse::<tiny_http::Header>()
                        .unwrap(),
                );
            let _ = request.respond(response);

            return parsed;
        }
    }
}

fn parse_callback_query(url: &str) -> Result<CallbackParams> {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        match key.as_ref() {
            "code" => code = Some(value.into_owned()),
            "state" => state = Some(value.into_owned()),
            "error" => error = Some(value.into_owned()),
            _ => {}
        }
    }
    if let Some(err) = error {
        bail!("OAuth provider reported error: {err}");
    }
    let code = code.ok_or_else(|| anyhow!("callback missing `code` parameter"))?;
    Ok(CallbackParams { code, state })
}

fn success_html() -> &'static str {
    r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Glide — Signed In</title>
<style>body{font-family:-apple-system,sans-serif;background:#faf9f6;color:#2d2a25;text-align:center;padding:96px 24px}h1{margin:0 0 8px}p{color:#666}</style>
</head><body><h1>Signed in to Glide</h1><p>You can close this tab and return to the app.</p></body></html>"#
}

fn error_html(detail: &str) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Glide — Sign In Failed</title>
<style>body{{font-family:-apple-system,sans-serif;background:#faf9f6;color:#2d2a25;text-align:center;padding:96px 24px}}h1{{margin:0 0 8px;color:#b22}}p{{color:#666}}</style>
</head><body><h1>Glide sign in failed</h1><p>{}</p></body></html>"#,
        html_escape(detail)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    #[serde(default)]
    pub access_token: Option<String>,
    pub id_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<i64>,
}

/// JWT signature is NOT verified; the token arrives over TLS directly from the
/// provider's token endpoint, so the transport already authenticates it.
#[derive(Debug, Clone, Deserialize)]
pub struct IdTokenClaims {
    pub sub: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

pub fn decode_id_token(token: &str) -> Result<IdTokenClaims> {
    let mut segments = token.split('.');
    let _header = segments.next().ok_or_else(|| anyhow!("JWT missing header"))?;
    let payload = segments
        .next()
        .ok_or_else(|| anyhow!("JWT missing payload"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .context("failed to base64url-decode JWT payload")?;
    let claims: IdTokenClaims =
        serde_json::from_slice(&decoded).context("failed to decode JWT payload JSON")?;
    Ok(claims)
}

pub fn open_browser(url: &str) -> Result<()> {
    open::that(url).context("failed to open system browser for sign in")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_generates_distinct_pairs() {
        let a = generate_pkce();
        let b = generate_pkce();
        assert_ne!(a.verifier, b.verifier);
        assert_ne!(a.challenge, b.challenge);
        assert_eq!(a.challenge.len(), 43);
    }

    #[test]
    fn test_state_is_random() {
        let a = generate_state();
        let b = generate_state();
        assert_ne!(a, b);
    }

    #[test]
    fn test_parse_callback_query_happy_path() {
        let parsed = parse_callback_query("/oauth2redirect?code=abc&state=xyz").unwrap();
        assert_eq!(parsed.code, "abc");
        assert_eq!(parsed.state.as_deref(), Some("xyz"));
    }

    #[test]
    fn test_parse_callback_query_reports_error() {
        let err = parse_callback_query("/oauth2redirect?error=access_denied").unwrap_err();
        assert!(err.to_string().contains("access_denied"));
    }

    #[test]
    fn test_parse_callback_query_missing_code() {
        assert!(parse_callback_query("/oauth2redirect?state=xyz").is_err());
    }

    #[test]
    fn test_decode_id_token_happy_path() {
        let header = URL_SAFE_NO_PAD.encode(br#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(br#"{"sub":"12345","email":"ada@example.com"}"#);
        let jwt = format!("{header}.{payload}.");
        let claims = decode_id_token(&jwt).unwrap();
        assert_eq!(claims.sub, "12345");
        assert_eq!(claims.email.as_deref(), Some("ada@example.com"));
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<a>&b"), "&lt;a&gt;&amp;b");
    }
}

//! Apple Sign In on macOS — placeholder.
//!
//! The native path (`ASAuthorizationAppleIDProvider` via objc2) is the right
//! long-term implementation but requires non-trivial FFI bridging. Until that
//! lands, the button is disabled on macOS. iOS uses the real native flow.

use anyhow::{Result, anyhow};

use super::{Account, AccountTokens};

pub const IS_AVAILABLE: bool = false;

pub async fn sign_in() -> Result<(Account, AccountTokens)> {
    Err(anyhow!(
        "Apple sign in is not yet available on macOS. Use Google sign in or continue as guest for now."
    ))
}

//! OAuth facilitation (core helper, module-facing).
//!
//! The core owns the OAuth mechanics; a module only supplies the provider env
//! (endpoints, client id, scopes) and later asks for a fresh access token. This
//! [`TokenStore`] holds the token-sets and performs **transparent refresh** via
//! the host HTTP client when an access token has expired.
//!
//! What is wired here (and testable offline): provider env registration, token
//! storage, `is_authorized`, `revoke`, and `access_token` with refresh-token
//! grant. The **interactive** `authorize` flow (open browser, capture redirect,
//! exchange the auth code) needs a desktop session and is the remaining piece;
//! it reuses this same store once a code is obtained.

use crate::http;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Refresh this long before actual expiry, to avoid races near the boundary.
const EXPIRY_SKEW: Duration = Duration::from_secs(30);

struct TokenSet {
    access: String,
    refresh: Option<String>,
    expires_at: Option<Instant>,
}

struct ProviderEnv {
    token_url: String,
    client_id: String,
    client_secret: Option<String>,
}

/// Per-provider OAuth token storage with transparent refresh.
#[derive(Default)]
pub struct TokenStore {
    tokens: Mutex<HashMap<String, TokenSet>>,
    providers: Mutex<HashMap<String, ProviderEnv>>,
}

/// The subset of a token endpoint response we consume.
#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    refresh_token: Option<String>,
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register the provider env (from a module's `provider-config`), needed for
    /// later refreshes.
    pub fn set_provider(
        &self,
        provider: &str,
        token_url: &str,
        client_id: &str,
        client_secret: Option<&str>,
    ) {
        self.providers.lock().unwrap().insert(
            provider.to_string(),
            ProviderEnv {
                token_url: token_url.to_string(),
                client_id: client_id.to_string(),
                client_secret: client_secret.map(str::to_string),
            },
        );
    }

    /// Seed a token-set (e.g. right after an interactive authorization, or in
    /// tests). `expires_in` is seconds from now. Used by tests today and by the
    /// pending interactive `authorize` flow once it captures an auth code.
    #[allow(dead_code)]
    pub fn seed(
        &self,
        provider: &str,
        access: &str,
        refresh: Option<&str>,
        expires_in: Option<u64>,
    ) {
        self.tokens.lock().unwrap().insert(
            provider.to_string(),
            TokenSet {
                access: access.to_string(),
                refresh: refresh.map(str::to_string),
                expires_at: expires_in.map(|s| Instant::now() + Duration::from_secs(s)),
            },
        );
    }

    /// Whether a usable (valid or refreshable) token-set exists.
    pub fn is_authorized(&self, provider: &str) -> bool {
        self.tokens
            .lock()
            .unwrap()
            .get(provider)
            .map(|t| !self.is_expired(t) || t.refresh.is_some())
            .unwrap_or(false)
    }

    pub fn revoke(&self, provider: &str) {
        self.tokens.lock().unwrap().remove(provider);
    }

    /// Return a currently-valid access token, refreshing transparently if it
    /// has expired and a refresh token is available.
    pub fn access_token(&self, provider: &str) -> Result<String, String> {
        {
            let tokens = self.tokens.lock().unwrap();
            match tokens.get(provider) {
                None => return Err(format!("not authorized: {provider}")),
                Some(t) if !self.is_expired(t) => return Ok(t.access.clone()),
                Some(t) if t.refresh.is_none() => {
                    return Err(format!("token for {provider} expired and not refreshable"))
                }
                Some(_) => {} // expired but refreshable — fall through
            }
        }
        self.refresh(provider)
    }

    fn is_expired(&self, token: &TokenSet) -> bool {
        match token.expires_at {
            Some(at) => Instant::now() + EXPIRY_SKEW >= at,
            None => false,
        }
    }

    fn refresh(&self, provider: &str) -> Result<String, String> {
        let (refresh_token, env_url, client_id, client_secret) = {
            let tokens = self.tokens.lock().unwrap();
            let providers = self.providers.lock().unwrap();
            let refresh = tokens
                .get(provider)
                .and_then(|t| t.refresh.clone())
                .ok_or_else(|| format!("no refresh token for {provider}"))?;
            let env = providers
                .get(provider)
                .ok_or_else(|| format!("no provider env for {provider}"))?;
            (
                refresh,
                env.token_url.clone(),
                env.client_id.clone(),
                env.client_secret.clone(),
            )
        };

        let mut form = format!(
            "grant_type=refresh_token&refresh_token={refresh_token}&client_id={client_id}"
        );
        if let Some(secret) = &client_secret {
            form.push_str(&format!("&client_secret={secret}"));
        }
        let resp = http::request(
            "POST",
            &env_url,
            &[(
                "Content-Type".into(),
                "application/x-www-form-urlencoded".into(),
            )],
            Some(form.as_bytes()),
        )?;
        if resp.status != 200 {
            return Err(format!("token endpoint returned {}", resp.status));
        }
        let parsed: TokenResponse =
            serde_json::from_slice(&resp.body).map_err(|e| format!("bad token response: {e}"))?;

        let mut tokens = self.tokens.lock().unwrap();
        let entry = tokens
            .get_mut(provider)
            .ok_or_else(|| format!("token-set vanished for {provider}"))?;
        entry.access = parsed.access_token.clone();
        if let Some(r) = parsed.refresh_token {
            entry.refresh = Some(r);
        }
        entry.expires_at = parsed
            .expires_in
            .map(|s| Instant::now() + Duration::from_secs(s));
        Ok(parsed.access_token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::test_server;

    #[test]
    fn returns_valid_token_without_refreshing() {
        let store = TokenStore::new();
        store.seed("twitch", "valid-token", None, Some(3600));
        assert!(store.is_authorized("twitch"));
        assert_eq!(store.access_token("twitch").unwrap(), "valid-token");
    }

    #[test]
    fn refreshes_an_expired_token() {
        let (base, _rx) =
            test_server::spawn(1, r#"{"access_token":"fresh-token","expires_in":3600}"#);
        let store = TokenStore::new();
        store.set_provider("twitch", &format!("{base}/token"), "client-123", None);
        // Already-expired access token, but a refresh token is present.
        store.seed("twitch", "stale", Some("refresh-abc"), Some(0));

        let token = store.access_token("twitch").unwrap();
        assert_eq!(token, "fresh-token");
        // The refreshed token is now valid and returned without re-refreshing.
        assert_eq!(store.access_token("twitch").unwrap(), "fresh-token");
    }

    #[test]
    fn unauthorized_and_revoke() {
        let store = TokenStore::new();
        assert!(!store.is_authorized("twitch"));
        assert!(store.access_token("twitch").is_err());

        store.seed("twitch", "t", None, Some(3600));
        assert!(store.is_authorized("twitch"));
        store.revoke("twitch");
        assert!(!store.is_authorized("twitch"));
    }
}

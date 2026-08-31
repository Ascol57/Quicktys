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
const EXPIRY_SKEW_SECS: u64 = 30;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct TokenSet {
    access: String,
    refresh: Option<String>,
    /// Absolute expiry, **unix seconds**. An `Instant` is monotonic-clock based
    /// and meaningless after a restart, which is exactly when we reload these.
    #[serde(default)]
    expires_at: Option<u64>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ProviderEnv {
    token_url: String,
    client_id: String,
    #[serde(default)]
    client_secret: Option<String>,
}

/// What lands on disk: the token-sets plus the provider envs they need in order
/// to refresh. Without the envs a reloaded refresh token would be unusable.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Persisted {
    #[serde(default)]
    tokens: HashMap<String, TokenSet>,
    #[serde(default)]
    providers: HashMap<String, ProviderEnv>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Per-provider OAuth token storage with transparent refresh.
#[derive(Default)]
pub struct TokenStore {
    tokens: Mutex<HashMap<String, TokenSet>>,
    providers: Mutex<HashMap<String, ProviderEnv>>,
    /// Device authorizations currently awaiting the user's approval.
    pending: Mutex<HashMap<String, PendingDevice>>,
    /// Where the token-sets are persisted. `None` → memory only (tests).
    path: Mutex<Option<std::path::PathBuf>>,
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

// --- Device Code Grant ------------------------------------------------------
//
// The flow a desktop app can actually use. The alternatives are worse here:
// Authorization Code needs a **client secret** (Twitch has no PKCE), which
// cannot be shipped in a distributed binary, and Implicit returns no refresh
// token. Device Code needs only a public `client_id`, returns a refresh token,
// and requires no loopback server or registered redirect URI — the user can
// even approve from their phone.
//
// Shape: POST the device endpoint (`client_id` + space-delimited `scopes`) for a
// `user_code` to show and a `verification_uri` to open, then poll the token
// endpoint until it stops answering `authorization_pending`.

/// What to show the user while a device authorization is pending.
#[derive(Clone, Debug, serde::Serialize)]
pub struct DeviceFlow {
    /// The short code the user types on the provider's page.
    pub user_code: String,
    /// Where they type it (Twitch pre-fills the code into this URL).
    pub verification_uri: String,
    /// Seconds until `user_code` expires.
    pub expires_in: u64,
    /// Seconds the provider asks us to wait between polls.
    pub interval: u64,
}

/// Where a device authorization stands.
#[derive(Clone, Debug, PartialEq)]
pub enum DeviceStatus {
    /// The user has not approved yet — keep polling.
    Pending,
    /// Tokens obtained and stored.
    Authorized,
    /// The code expired, or the provider refused. Start over.
    Failed(String),
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    interval: Option<u64>,
}

/// A device authorization we are polling for.
struct PendingDevice {
    device_code: String,
    scopes: String,
    expires_at: Instant,
}

const FORM: (&str, &str) = ("Content-Type", "application/x-www-form-urlencoded");

/// Percent-encode a form value (RFC 3986 unreserved set kept as-is). Scopes
/// carry `:` and are space-delimited, so they must be encoded.
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

impl TokenStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Back the store with a file, loading whatever it already holds.
    ///
    /// Without this a connected account is forgotten at every restart, which for
    /// a device-code flow means re-approving on Twitch each launch. Refresh
    /// tokens are long-lived, so persisting them is what makes "connect once"
    /// true.
    ///
    /// ⚠️ Stored as plain JSON, `0600` on unix. That protects it from other
    /// users on the machine, **not** from anything running as this user. The
    /// next step is the OS keyring; until then, treat the file as a secret.
    pub fn set_path(&self, path: std::path::PathBuf) {
        if let Ok(bytes) = std::fs::read(&path) {
            if let Ok(loaded) = serde_json::from_slice::<Persisted>(&bytes) {
                let mut tokens = self.tokens.lock().unwrap();
                let mut providers = self.providers.lock().unwrap();
                // Drop anything already dead and unrefreshable rather than
                // reload a token that can only fail.
                for (provider, set) in loaded.tokens {
                    let dead = set.refresh.is_none()
                        && set.expires_at.is_some_and(|at| now_unix() >= at);
                    if !dead {
                        tokens.insert(provider, set);
                    }
                }
                providers.extend(loaded.providers);
            }
        }
        *self.path.lock().unwrap() = Some(path);
    }

    /// Write the current token-sets out. Best-effort: a failure to persist must
    /// never break an otherwise working authorization.
    fn save(&self) {
        let path = self.path.lock().unwrap().clone();
        let Some(path) = path else { return };
        let snapshot = Persisted {
            tokens: self.tokens.lock().unwrap().clone(),
            providers: self.providers.lock().unwrap().clone(),
        };
        let Ok(bytes) = serde_json::to_vec_pretty(&snapshot) else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, bytes).is_err() {
            tracing::warn!("could not persist oauth tokens to {}", path.display());
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
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
        self.save();
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
                expires_at: expires_in.map(|s| now_unix() + s),
            },
        );
        self.save();
    }

    /// Begin a Device Code authorization: ask the provider for a `user_code`
    /// to display and a URL to open, and remember the `device_code` to poll
    /// with. Also registers the provider env, so the refresh path works once
    /// the tokens land.
    pub fn start_device(
        &self,
        provider: &str,
        device_url: &str,
        token_url: &str,
        client_id: &str,
        scopes: &str,
    ) -> Result<DeviceFlow, String> {
        if client_id.trim().is_empty() {
            return Err("no client id configured".to_string());
        }
        self.set_provider(provider, token_url, client_id, None);

        let form = format!(
            "client_id={}&scopes={}",
            form_encode(client_id),
            form_encode(scopes)
        );
        let resp = http::request(
            "POST",
            device_url,
            &[(FORM.0.into(), FORM.1.into())],
            Some(form.as_bytes()),
        )?;
        if resp.status != 200 {
            return Err(format!(
                "device endpoint returned {}: {}",
                resp.status,
                String::from_utf8_lossy(&resp.body)
            ));
        }
        let parsed: DeviceCodeResponse = serde_json::from_slice(&resp.body)
            .map_err(|e| format!("bad device response: {e}"))?;
        let expires_in = parsed.expires_in.unwrap_or(1800);
        let interval = parsed.interval.unwrap_or(5).max(1);

        self.pending.lock().unwrap().insert(
            provider.to_string(),
            PendingDevice {
                device_code: parsed.device_code,
                scopes: scopes.to_string(),
                expires_at: Instant::now() + Duration::from_secs(expires_in),
            },
        );
        Ok(DeviceFlow {
            user_code: parsed.user_code,
            verification_uri: parsed.verification_uri,
            expires_in,
            interval,
        })
    }

    /// Poll a pending device authorization once. Call at the `interval` the
    /// provider asked for; polling faster earns a `slow_down`.
    pub fn poll_device(&self, provider: &str) -> Result<DeviceStatus, String> {
        let (device_code, scopes) = {
            let pending = self.pending.lock().unwrap();
            let p = pending
                .get(provider)
                .ok_or_else(|| format!("no pending authorization for {provider}"))?;
            if Instant::now() >= p.expires_at {
                drop(pending);
                self.cancel_device(provider);
                return Ok(DeviceStatus::Failed("the code expired".to_string()));
            }
            (p.device_code.clone(), p.scopes.clone())
        };
        let (token_url, client_id) = {
            let providers = self.providers.lock().unwrap();
            let env = providers
                .get(provider)
                .ok_or_else(|| format!("no provider env for {provider}"))?;
            (env.token_url.clone(), env.client_id.clone())
        };

        let form = format!(
            "client_id={}&scopes={}&device_code={}&grant_type={}",
            form_encode(&client_id),
            form_encode(&scopes),
            form_encode(&device_code),
            form_encode("urn:ietf:params:oauth:grant-type:device_code")
        );
        let resp = http::request(
            "POST",
            &token_url,
            &[(FORM.0.into(), FORM.1.into())],
            Some(form.as_bytes()),
        )?;
        let body = String::from_utf8_lossy(&resp.body).to_string();

        if resp.status == 200 {
            let parsed: TokenResponse =
                serde_json::from_slice(&resp.body).map_err(|e| format!("bad token response: {e}"))?;
            self.seed(
                provider,
                &parsed.access_token,
                parsed.refresh_token.as_deref(),
                parsed.expires_in,
            );
            self.cancel_device(provider);
            return Ok(DeviceStatus::Authorized);
        }
        // Not an error: the user simply has not approved yet. `slow_down` means
        // we polled too fast — also "keep waiting".
        if body.contains("authorization_pending") || body.contains("slow_down") {
            return Ok(DeviceStatus::Pending);
        }
        self.cancel_device(provider);
        Ok(DeviceStatus::Failed(format!(
            "authorization refused ({}): {body}",
            resp.status
        )))
    }

    /// Forget a pending device authorization (user cancelled, or it failed).
    pub fn cancel_device(&self, provider: &str) {
        self.pending.lock().unwrap().remove(provider);
    }

    /// Whether a device authorization is awaiting approval.
    pub fn device_pending(&self, provider: &str) -> bool {
        self.pending.lock().unwrap().contains_key(provider)
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
        self.save();
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
            Some(at) => now_unix() + EXPIRY_SKEW_SECS >= at,
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
        entry.expires_at = parsed.expires_in.map(|s| now_unix() + s);
        drop(tokens);
        self.save();
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

    /// The whole device flow offline: device code → one `authorization_pending`
    /// → the token. Proves the polling loop stops for the right reason and that
    /// the resulting token-set is usable (and refreshable) afterwards.
    #[test]
    fn device_flow_polls_until_authorized() {
        let (base, requests) = test_server::spawn_sequence(vec![
            (
                200,
                r#"{"device_code":"dev-1","user_code":"ABCD-1234","verification_uri":"https://twitch.tv/activate","expires_in":1800,"interval":5}"#
                    .to_string(),
            ),
            (400, r#"{"status":400,"message":"authorization_pending"}"#.to_string()),
            (
                200,
                r#"{"access_token":"user-token","refresh_token":"r-1","expires_in":3600}"#
                    .to_string(),
            ),
        ]);
        let store = TokenStore::new();

        let flow = store
            .start_device(
                "twitch",
                &format!("{base}/device"),
                &format!("{base}/token"),
                "client-123",
                "chat:read chat:edit",
            )
            .unwrap();
        assert_eq!(flow.user_code, "ABCD-1234");
        assert_eq!(flow.interval, 5);
        assert!(store.device_pending("twitch"));
        assert!(!store.is_authorized("twitch"));

        assert_eq!(store.poll_device("twitch").unwrap(), DeviceStatus::Pending);
        assert_eq!(store.poll_device("twitch").unwrap(), DeviceStatus::Authorized);

        assert!(!store.device_pending("twitch"), "pending cleared once authorized");
        assert!(store.is_authorized("twitch"));
        assert_eq!(store.access_token("twitch").unwrap(), "user-token");

        // The device request carried the client id and URL-encoded scopes.
        let device_req = requests.recv().unwrap();
        assert!(device_req.contains("client_id=client-123"), "{device_req}");
        assert!(
            device_req.contains("scopes=chat%3Aread%20chat%3Aedit"),
            "scopes must be space-delimited and URL encoded: {device_req}"
        );
        // The poll carried the device code and the device-code grant type.
        let poll = requests.recv().unwrap();
        assert!(poll.contains("device_code=dev-1"), "{poll}");
        assert!(
            poll.contains("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"),
            "{poll}"
        );
    }

    /// A refusal (the user pressed Deny, or the code died) must end the flow
    /// rather than leave the UI polling forever.
    #[test]
    fn device_flow_stops_on_refusal() {
        let (base, _rx) = test_server::spawn_sequence(vec![
            (
                200,
                r#"{"device_code":"dev-1","user_code":"WXYZ","verification_uri":"https://twitch.tv/activate"}"#
                    .to_string(),
            ),
            (400, r#"{"status":400,"message":"invalid device code"}"#.to_string()),
        ]);
        let store = TokenStore::new();
        store
            .start_device(
                "twitch",
                &format!("{base}/device"),
                &format!("{base}/token"),
                "client-123",
                "chat:read",
            )
            .unwrap();

        match store.poll_device("twitch").unwrap() {
            DeviceStatus::Failed(msg) => assert!(msg.contains("invalid device code"), "{msg}"),
            other => panic!("expected a failure, got {other:?}"),
        }
        assert!(!store.device_pending("twitch"), "the flow must not stay pending");
        assert!(!store.is_authorized("twitch"));
    }

    /// A connected account must survive a restart: reload the file into a fresh
    /// store and the token (and the env needed to refresh it) are still there.
    #[test]
    fn tokens_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("qt-oauth-{}", now_unix()));
        let path = dir.join("oauth.json");
        let _ = std::fs::remove_dir_all(&dir);

        {
            let store = TokenStore::new();
            store.set_path(path.clone());
            store.set_provider("twitch", "https://id.twitch.tv/oauth2/token", "client-123", None);
            store.seed("twitch", "user-token", Some("refresh-1"), Some(3600));
            assert!(store.is_authorized("twitch"));
        }

        let reloaded = TokenStore::new();
        assert!(!reloaded.is_authorized("twitch"), "nothing before the file is attached");
        reloaded.set_path(path.clone());
        assert!(reloaded.is_authorized("twitch"), "the token came back");
        assert_eq!(reloaded.access_token("twitch").unwrap(), "user-token");

        // Disconnecting must clear the file too, not just memory.
        reloaded.revoke("twitch");
        let after = TokenStore::new();
        after.set_path(path);
        assert!(!after.is_authorized("twitch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An access token that is already dead and has no refresh token is not
    /// worth reloading — it could only produce a confusing "connected" state.
    #[test]
    fn a_dead_unrefreshable_token_is_not_reloaded() {
        let dir = std::env::temp_dir().join(format!("qt-oauth-dead-{}", now_unix()));
        let path = dir.join("oauth.json");
        let _ = std::fs::remove_dir_all(&dir);
        {
            let store = TokenStore::new();
            store.set_path(path.clone());
            store.seed("twitch", "stale", None, Some(0));
        }
        let reloaded = TokenStore::new();
        reloaded.set_path(path);
        assert!(!reloaded.is_authorized("twitch"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn device_flow_needs_a_client_id() {
        let store = TokenStore::new();
        assert!(store
            .start_device("twitch", "http://x/device", "http://x/token", "  ", "chat:read")
            .is_err());
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

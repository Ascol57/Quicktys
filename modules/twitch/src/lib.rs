//! `twitch` module — a certified driver for **Twitch's Helix API**.
//!
//! Being certified and declaring the `net` capability, it reaches the network
//! through the host; the sandboxed `twitch` plugin never could. It exposes typed
//! services (send a chat message, announce, generic authenticated Helix request,
//! stream info) consumed by the `twitch` plugin.
//!
//! Auth (an OAuth **user** token + the app **client-id**, plus the broadcaster /
//! sender user-ids) is injected via `set-auth` and persisted in `storage`.
//! `set-base-url` overrides the API root so tests point it at a stand-in server
//! (never the live api.twitch.tv).
//!
//! Not covered here: **EventSub** (live chat/follow/sub *triggers*) needs the
//! host's streaming `ws-connect → on-host-event` path, which is not wired yet —
//! this module is the request/response half (actions), documented as such.

use quicktys_module_sdk::{
    Capability, Module, ModuleManifest, Schema, ServiceDecl, Value,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::net::{fetch, HttpRequest};
use quicktys::api::oauth::access_token;
use quicktys::api::storage::{kv_get, kv_set};

struct Twitch;

// --- OAuth (Device Code Grant) ----------------------------------------------
//
// Twitch's endpoints and scopes are OUR business, not the core's: the core owns
// the OAuth mechanics and asks us for this config (ARCHITECTURE §4.6). It runs
// the flow, stores the token-set, and refreshes it; we just read the current
// access token back through `oauth.access-token`.
//
// Device Code is the only flow a distributed desktop app can honestly use here:
// Authorization Code requires a client secret (Twitch has no PKCE) and Implicit
// returns no refresh token.
const DEVICE_URL: &str = "https://id.twitch.tv/oauth2/device";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
/// What the actions in this module actually need. Space-delimited, as Twitch
/// wants. Keep in step with the services below — asking for more than we use is
/// a consent smell.
const SCOPES: &str = "user:read:chat user:write:chat user:bot channel:bot moderator:manage:announcements channel:read:subscriptions";

/// The access token to authenticate Helix with: the OAuth store first (a
/// connected account, refreshed transparently by the core), falling back to a
/// token injected by hand through `set-auth` (self-hosters and the tests).
fn auth_token() -> Option<String> {
    access_token("twitch").ok().or_else(|| stored("token"))
}

/// The Helix API root. Defaults to the real host; override with `set-base-url`.
fn base_url() -> String {
    kv_get("base_url")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "https://api.twitch.tv/helix".to_string())
}

fn stored(key: &str) -> Option<String> {
    kv_get(key).filter(|s| !s.is_empty())
}

/// The broadcaster/channel user-id (used for chat, announcements, streams).
fn broadcaster_id() -> Option<String> {
    stored("broadcaster_id")
}

/// The sending user-id — defaults to the broadcaster (single-account setup).
fn sender_id() -> String {
    stored("sender_id").or_else(broadcaster_id).unwrap_or_default()
}

/// Percent-encode a query-string value (RFC 3986 unreserved kept as-is).
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// One authenticated Helix request. `query` is a list of `(key, value)` pairs.
/// Returns `(status, parsed-body)`; the body is `Null` when empty.
fn helix(
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: Option<Value>,
) -> Result<(u16, Value), String> {
    let token = auth_token().ok_or("twitch: not connected (connect the account in Settings)")?;
    let client_id = stored("client_id").ok_or("twitch: missing client-id (set it in Settings)")?;

    let mut headers = vec![
        ("Authorization".to_string(), format!("Bearer {token}")),
        ("Client-Id".to_string(), client_id),
    ];
    let body_bytes = body.map(|b| {
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
        serde_json::to_vec(&b).unwrap_or_default()
    });

    let mut url = format!("{}{path}", base_url());
    if !query.is_empty() {
        let qs: Vec<String> = query
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect();
        url.push('?');
        url.push_str(&qs.join("&"));
    }

    let request = HttpRequest {
        method: method.to_string(),
        url,
        headers,
        body: body_bytes,
    };
    let resp = fetch(&request).map_err(|e| format!("network error: {e:?}"))?;
    let parsed = if resp.body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&resp.body).unwrap_or(Value::Null)
    };
    Ok((resp.status, parsed))
}

/// Helix with the common "4xx/5xx is an error" policy applied.
fn helix_ok(
    method: &str,
    path: &str,
    query: &[(String, String)],
    body: Option<Value>,
) -> Result<Value, String> {
    let (status, parsed) = helix(method, path, query, body)?;
    if status == 401 {
        return Err("twitch: unauthorized (token rejected — re-authenticate)".to_string());
    }
    if status >= 400 {
        let msg = parsed
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("request failed");
        return Err(format!("twitch helix {path} → {status}: {msg}"));
    }
    Ok(parsed)
}

impl Module for Twitch {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("twitch", "Twitch", "0.1.0")
            .capability(Capability::Net)
            // Inject auth: { token, client_id, broadcaster_id, sender_id? }.
            .service(ServiceDecl::new("set-auth").args(Schema::object([
                ("token".to_string(), Schema::string()),
                ("client_id".to_string(), Schema::string()),
                ("broadcaster_id".to_string(), Schema::string()),
                ("sender_id".to_string(), Schema::string()),
            ])))
            // The provider config the core asks for before running the device
            // flow (it owns the mechanics, we own the Twitch specifics).
            .service(ServiceDecl::new("oauth-config"))
            // The registered application's client id — required both for the
            // device flow and as Helix's `Client-Id` header.
            .service(
                ServiceDecl::new("set-client-id")
                    .args(Schema::object([("client_id".to_string(), Schema::string())])),
            )
            // Whether we can authenticate, and how (for the Settings page).
            .service(ServiceDecl::new("auth-status"))
            // Test/self-host override of the API root.
            .service(
                ServiceDecl::new("set-base-url")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
            // Send a chat message: { message, reply_parent_message_id? }.
            .service(ServiceDecl::new("send-chat").args(Schema::object([
                ("message".to_string(), Schema::string()),
                ("reply_parent_message_id".to_string(), Schema::string()),
            ])))
            // Announce (highlighted): { message, color? }.
            .service(ServiceDecl::new("announce").args(Schema::object([
                ("message".to_string(), Schema::string()),
                ("color".to_string(), Schema::string()),
            ])))
            // Current stream info (empty array data → offline).
            .service(ServiceDecl::new("get-stream"))
            // Generic authenticated Helix request escape hatch.
            .service(ServiceDecl::new("helix").args(Schema::object([
                ("method".to_string(), Schema::string()),
                ("path".to_string(), Schema::string()),
            ])))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        let s = |k: &str| args.get(k).and_then(Value::as_str).map(str::to_string);
        match method {
            "set-auth" => {
                for key in ["token", "client_id", "broadcaster_id", "sender_id"] {
                    if let Some(v) = s(key) {
                        kv_set(key, &v);
                    }
                }
                Ok(Value::Null)
            }
            "oauth-config" => Ok(quicktys_module_sdk::json!({
                "provider": "twitch",
                "device_url": DEVICE_URL,
                "token_url": TOKEN_URL,
                "client_id": stored("client_id").unwrap_or_default(),
                "scopes": SCOPES,
            })),
            "set-client-id" => {
                let id = s("client_id").ok_or("set-client-id: `client_id` is required")?;
                kv_set("client_id", id.trim());
                Ok(Value::Null)
            }
            "auth-status" => Ok(quicktys_module_sdk::json!({
                "clientId": stored("client_id").unwrap_or_default(),
                "hasClientId": stored("client_id").is_some(),
                // A manually injected token counts as usable too.
                "hasToken": auth_token().is_some(),
            })),
            "set-base-url" => {
                let url = s("url").ok_or("set-base-url: `url` is required")?;
                kv_set("base_url", url.trim_end_matches('/'));
                Ok(Value::Null)
            }
            "send-chat" => {
                let message = s("message").ok_or("send-chat: `message` is required")?;
                let bid = broadcaster_id().ok_or("twitch: no broadcaster_id (call set-auth)")?;
                let mut body = serde_json::Map::new();
                body.insert("broadcaster_id".into(), Value::String(bid));
                body.insert("sender_id".into(), Value::String(sender_id()));
                body.insert("message".into(), Value::String(message));
                if let Some(reply) = s("reply_parent_message_id") {
                    body.insert("reply_parent_message_id".into(), Value::String(reply));
                }
                helix_ok("POST", "/chat/messages", &[], Some(Value::Object(body)))
            }
            "announce" => {
                let message = s("message").ok_or("announce: `message` is required")?;
                let bid = broadcaster_id().ok_or("twitch: no broadcaster_id (call set-auth)")?;
                let mut body = serde_json::Map::new();
                body.insert("message".into(), Value::String(message));
                body.insert(
                    "color".into(),
                    Value::String(s("color").unwrap_or_else(|| "primary".into())),
                );
                let query = [
                    ("broadcaster_id".to_string(), bid),
                    ("moderator_id".to_string(), sender_id()),
                ];
                helix_ok("POST", "/chat/announcements", &query, Some(Value::Object(body)))
            }
            "get-stream" => {
                let bid = broadcaster_id().ok_or("twitch: no broadcaster_id (call set-auth)")?;
                let data = helix_ok("GET", "/streams", &[("user_id".to_string(), bid)], None)?;
                // Return the first stream object, or Null when offline.
                Ok(data
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|a| a.first())
                    .cloned()
                    .unwrap_or(Value::Null))
            }
            "helix" => {
                let m = s("method").unwrap_or_else(|| "GET".into());
                let path = s("path").ok_or("helix: `path` is required")?;
                let body = args.get("body").cloned().filter(|v| !v.is_null());
                let query: Vec<(String, String)> = args
                    .get("query")
                    .and_then(Value::as_object)
                    .map(|o| {
                        o.iter()
                            .map(|(k, v)| {
                                (k.clone(), v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                helix_ok(&m, &path, &query, body)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }

    fn on_load() -> Result<(), String> {
        Ok(())
    }

    fn on_host_event(_event_id: &str, _payload: Value) {}
}

quicktys_module_sdk::define_module!(Twitch);

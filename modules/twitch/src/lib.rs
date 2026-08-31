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
//! **Chat** is read live over Twitch IRC (`wss://irc-ws.chat.twitch.tv`) and
//! republished as `chat` events — anonymously, so a chat display works with no
//! account connected. Still not covered: **EventSub** (follow/sub/raid
//! triggers), which needs subscription management on top of the same socket.

use quicktys_module_sdk::{
    Capability, EventDecl, Module, ModuleManifest, Schema, ServiceDecl, Value,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::events::emit_event;
use quicktys::api::logging::{log, Level};
use quicktys::api::net::{close, fetch, send, ws_connect, HttpRequest};
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
/// `moderator:read:followers` is what `channel.follow` v2 requires; the rest
/// back the chat/announce actions and `channel.subscribe`. `channel.raid` needs
/// no scope. Adding a scope invalidates existing tokens — reconnect after.
const SCOPES: &str = "user:read:chat user:write:chat user:bot channel:bot moderator:manage:announcements channel:read:subscriptions moderator:read:followers";

// --- Chat (IRC over WebSocket) ----------------------------------------------
//
// Twitch decommissioned the unencrypted WebSocket endpoint, so this needs the
// host's `wss://` support. We connect **anonymously**: an `justinfan<n>` nick
// with no PASS gives a read-only feed, which is all a chat *display* needs — so
// the widget works before any account is connected, and no token ever travels
// over the wire. Sending is a separate, authenticated path (Helix `send-chat`).
//
// `twitch.tv/tags` is what makes a pretty widget possible: it decorates PRIVMSG
// with the sender's chosen colour, display name, badges and emote positions.
const CHAT_WS: &str = "wss://irc-ws.chat.twitch.tv:443";

/// The chat socket, overridable so tests point at a stand-in IRC server and
/// never touch the real Twitch (same rule as `set-base-url` for Helix).
fn chat_url() -> String {
    kv_get("chat_url")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| CHAT_WS.to_string())
}
/// Host event id for the chat socket (see `on_host_event`).
const CHAT_EVENT: &str = "chat-irc";

thread_local! {
    /// Write handle for the chat socket, for PONG replies and JOINs.
    static CHAT_HANDLE: std::cell::RefCell<Option<u64>> = const { std::cell::RefCell::new(None) };
}

/// Send one IRC line. Failures are logged rather than swallowed: if CAP/NICK/
/// JOIN do not reach the server the chat is silently dead, which is exactly the
/// bug that hid a racy `ws-connect`.
fn chat_send(line: &str) {
    CHAT_HANDLE.with(|h| match *h.borrow() {
        Some(handle) => {
            if let Err(e) = send(handle, format!("{line}\r\n").as_bytes()) {
                log(Level::Warn, &format!("twitch chat: send failed: {e:?}"));
            }
        }
        None => log(Level::Warn, "twitch chat: no socket to send on"),
    });
}

/// Split an IRC line's `@k=v;k2=v2` tag block from the rest.
fn split_tags(line: &str) -> (Vec<(String, String)>, &str) {
    let Some(rest) = line.strip_prefix('@') else {
        return (Vec::new(), line);
    };
    let (tags, remainder) = match rest.split_once(' ') {
        Some((t, r)) => (t, r),
        None => (rest, ""),
    };
    let parsed = tags
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), unescape_tag(v)))
        .collect();
    (parsed, remainder)
}

/// IRCv3 tag values escape spaces, semicolons, CR and LF.
fn unescape_tag(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some(':') => out.push(';'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// `emotes` tag → `[{id, start, end}]`, sorted by position. The widget turns
/// these into CDN images; without them a chat is just grey text.
fn parse_emotes(tag: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for group in tag.split('/').filter(|g| !g.is_empty()) {
        let Some((id, ranges)) = group.split_once(':') else {
            continue;
        };
        for range in ranges.split(',') {
            if let Some((a, b)) = range.split_once('-') {
                if let (Ok(start), Ok(end)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    out.push(quicktys_module_sdk::json!({
                        "id": id, "start": start, "end": end
                    }));
                }
            }
        }
    }
    out.sort_by_key(|e| e.get("start").and_then(Value::as_u64).unwrap_or(0));
    out
}

/// Turn one IRC line into a `chat` event, answering PINGs on the way.
fn on_irc_line(line: &str) {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return;
    }
    // Keepalive: the server drops us if we don't answer.
    if let Some(token) = line.strip_prefix("PING") {
        chat_send(&format!("PONG{token}"));
        return;
    }
    let (tags, rest) = split_tags(line);
    let tag = |k: &str| {
        tags.iter()
            .find(|(name, _)| name == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };

    // :nick!user@host PRIVMSG #channel :text
    let Some((prefix, command)) = rest.strip_prefix(':').and_then(|r| r.split_once(' ')) else {
        return;
    };
    if !command.starts_with("PRIVMSG") {
        return;
    }
    let Some((_, text)) = command.split_once(" :") else {
        return;
    };
    let nick = prefix.split('!').next().unwrap_or_default().to_string();
    let display = match tag("display-name") {
        d if d.is_empty() => nick.clone(),
        d => d,
    };
    // Keep `set/version` intact — the version is what indexes the badge image
    // map from Helix (`subscriber/12` is a different image from `subscriber/1`).
    let badges: Vec<Value> = tag("badges")
        .split(',')
        .filter(|b| !b.is_empty())
        .map(|b| Value::String(b.to_string()))
        .collect();

    emit_event(
        "chat",
        &quicktys_module_sdk::json!({
            "id": tag("id"),
            "user": nick,
            "displayName": display,
            // Twitch leaves `color` empty for users who never picked one; the
            // widget assigns a stable colour from the name in that case.
            "color": tag("color"),
            "text": text,
            "badges": badges,
            "emotes": parse_emotes(&tag("emotes")),
            "channel": stored("chat_channel").unwrap_or_default(),
        })
        .to_string(),
    );
}

// --- EventSub (follow / sub / raid) -----------------------------------------
//
// Chat comes over IRC; everything else comes over EventSub. The shape: open the
// socket, wait for `session_welcome`, then create the subscriptions over Helix
// with `transport: {method:"websocket", session_id}` — **within 10 seconds**, or
// Twitch closes the connection with code 4003. Notifications then arrive on the
// same socket. Unlike chat this needs a *user* token (app tokens are refused),
// so it only works once an account is connected.
const EVENTSUB_WS: &str = "wss://eventsub.wss.twitch.tv/ws";
const EVENTSUB_EVENT: &str = "eventsub";

/// Overridable so tests point at a stand-in server (never the real Twitch).
fn eventsub_url() -> String {
    kv_get("eventsub_url")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| EVENTSUB_WS.to_string())
}

/// The subscriptions we create, with the exact versions and conditions Twitch
/// documents. `channel.follow` is v2 — v1 is deprecated and rejected.
fn subscription_specs(broadcaster: &str, moderator: &str) -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "channel.follow",
            "2",
            quicktys_module_sdk::json!({
                "broadcaster_user_id": broadcaster,
                "moderator_user_id": moderator,
            }),
        ),
        (
            "channel.subscribe",
            "1",
            quicktys_module_sdk::json!({ "broadcaster_user_id": broadcaster }),
        ),
        (
            // Raids INTO this channel. `from_`/`to_` are mutually exclusive.
            "channel.raid",
            "1",
            quicktys_module_sdk::json!({ "to_broadcaster_user_id": broadcaster }),
        ),
    ]
}

/// Twitch subscription type → the module event we publish.
fn event_name(subscription_type: &str) -> Option<&'static str> {
    match subscription_type {
        "channel.follow" => Some("follow"),
        "channel.subscribe" => Some("subscribe"),
        "channel.raid" => Some("raid"),
        _ => None,
    }
}

/// Create every subscription against the freshly-welcomed session.
fn create_subscriptions(session_id: &str) {
    let (Some(broadcaster), Some(moderator)) =
        (stored("broadcaster_id"), stored("moderator_id"))
    else {
        log(Level::Warn, "twitch eventsub: no broadcaster id, skipping");
        return;
    };
    for (kind, version, condition) in subscription_specs(&broadcaster, &moderator) {
        let body = quicktys_module_sdk::json!({
            "type": kind,
            "version": version,
            "condition": condition,
            "transport": { "method": "websocket", "session_id": session_id },
        });
        match helix_ok("POST", "/eventsub/subscriptions", &[], Some(body)) {
            Ok(_) => log(Level::Info, &format!("twitch eventsub: subscribed to {kind}")),
            Err(e) => log(Level::Warn, &format!("twitch eventsub: {kind} failed: {e}")),
        }
    }
}

/// Handle one EventSub frame.
fn on_eventsub(message: &Value) {
    let meta_type = message
        .get("metadata")
        .and_then(|m| m.get("message_type"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let payload = message.get("payload");

    match meta_type {
        // The 10-second clock starts here.
        "session_welcome" => {
            if let Some(id) = payload
                .and_then(|p| p.get("session"))
                .and_then(|s| s.get("id"))
                .and_then(Value::as_str)
            {
                create_subscriptions(id);
            }
        }
        "notification" => {
            let kind = payload
                .and_then(|p| p.get("subscription"))
                .and_then(|s| s.get("type"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let Some(name) = event_name(kind) else { return };
            let event = payload.and_then(|p| p.get("event")).cloned().unwrap_or(Value::Null);
            emit_event(name, &event.to_string());
        }
        // Twitch is migrating us: reconnect, then it closes the old socket.
        "session_reconnect" => {
            if let Some(url) = payload
                .and_then(|p| p.get("session"))
                .and_then(|s| s.get("reconnect_url"))
                .and_then(Value::as_str)
            {
                log(Level::Info, "twitch eventsub: reconnecting");
                if let Ok(handle) = ws_connect(url, EVENTSUB_EVENT) {
                    EVENTSUB_HANDLE.with(|h| *h.borrow_mut() = Some(handle));
                }
            }
        }
        "revocation" => log(Level::Warn, "twitch eventsub: a subscription was revoked"),
        _ => {} // session_keepalive and anything new
    }
}

thread_local! {
    /// Handle of the EventSub socket (kept so a reconnect can replace it).
    static EVENTSUB_HANDLE: std::cell::RefCell<Option<u64>> = const { std::cell::RefCell::new(None) };
}

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
            // Join a channel's chat (read-only, anonymous) and stream it as
            // `chat` events. Idempotent: re-joining just switches channel.
            .service(
                ServiceDecl::new("connect-chat")
                    .args(Schema::object([("channel".to_string(), Schema::string())])),
            )
            // Open the EventSub socket and subscribe to follow/sub/raid.
            .service(
                ServiceDecl::new("connect-events")
                    .args(Schema::object([("channel".to_string(), Schema::string())])),
            )
            .service(
                ServiceDecl::new("set-eventsub-url")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
            .event(EventDecl::new("follow").payload(Schema::object([
                ("user_name".to_string(), Schema::string()),
                ("user_login".to_string(), Schema::string()),
            ])))
            .event(EventDecl::new("subscribe").payload(Schema::object([
                ("user_name".to_string(), Schema::string()),
                ("tier".to_string(), Schema::string()),
                ("is_gift".to_string(), Schema::boolean()),
            ])))
            .event(EventDecl::new("raid").payload(Schema::object([
                ("from_broadcaster_user_name".to_string(), Schema::string()),
                ("viewers".to_string(), Schema::number()),
            ])))
            // Badge images for the chat widget: `set/version` → CDN URL.
            .service(
                ServiceDecl::new("chat-badges")
                    .args(Schema::object([("channel".to_string(), Schema::string())])),
            )
            // Test/self-host override of the chat socket.
            .service(
                ServiceDecl::new("set-chat-url")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
            .event(
                EventDecl::new("chat").payload(Schema::object([
                    ("user".to_string(), Schema::string()),
                    ("displayName".to_string(), Schema::string()),
                    ("text".to_string(), Schema::string()),
                    ("color".to_string(), Schema::string()),
                ])),
            )
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
            "connect-chat" => {
                let channel = s("channel")
                    .map(|c| c.trim().trim_start_matches('#').to_lowercase())
                    .filter(|c| !c.is_empty())
                    .ok_or("connect-chat: `channel` is required")?;
                kv_set("chat_channel", &channel);

                // One socket, always. Several widgets can ask to watch the same
                // channel (a dashboard box *and* an overlay); without closing the
                // previous connection every message would be delivered twice.
                CHAT_HANDLE.with(|h| {
                    if let Some(old) = h.borrow_mut().take() {
                        close(old);
                    }
                });
                let handle = ws_connect(&chat_url(), CHAT_EVENT)
                    .map_err(|e| format!("twitch chat: connect failed: {e:?}"))?;
                CHAT_HANDLE.with(|h| *h.borrow_mut() = Some(handle));

                // Tags carry the colour, display name, badges and emotes; the
                // commands cap is a prerequisite for tags.
                chat_send("CAP REQ :twitch.tv/tags twitch.tv/commands");
                // Anonymous read-only login. The suffix only has to be unique
                // per connection.
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(1);
                chat_send(&format!("NICK justinfan{}", 10_000 + nonce % 80_000));
                chat_send(&format!("JOIN #{channel}"));
                Ok(quicktys_module_sdk::json!({ "channel": channel }))
            }
            // Global + channel badge images, flattened to `set/version` → URL.
            // Needs a token (any Helix call does), so this is only available once
            // an account is connected; the widget falls back to text pills.
            "chat-badges" => {
                let channel = s("channel")
                    .map(|c| c.trim().trim_start_matches('#').to_lowercase())
                    .filter(|c| !c.is_empty())
                    .ok_or("chat-badges: `channel` is required")?;

                let mut map = quicktys_module_sdk::serde_json::Map::new();
                let mut absorb = |sets: &Value| {
                    for set in sets.get("data").and_then(Value::as_array).into_iter().flatten() {
                        let set_id = set.get("set_id").and_then(Value::as_str).unwrap_or_default();
                        for version in set.get("versions").and_then(Value::as_array).into_iter().flatten() {
                            let id = version.get("id").and_then(Value::as_str).unwrap_or_default();
                            let url = version
                                .get("image_url_2x")
                                .or_else(|| version.get("image_url_1x"))
                                .and_then(Value::as_str)
                                .unwrap_or_default();
                            if !set_id.is_empty() && !id.is_empty() && !url.is_empty() {
                                map.insert(format!("{set_id}/{id}"), Value::String(url.to_string()));
                            }
                        }
                    }
                };

                absorb(&helix_ok("GET", "/chat/badges/global", &[], None)?);
                // Channel badges (custom sub badges) need the broadcaster's id.
                let users = helix_ok(
                    "GET",
                    "/users",
                    &[("login".to_string(), channel.clone())],
                    None,
                )?;
                if let Some(id) = users
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|d| d.first())
                    .and_then(|u| u.get("id"))
                    .and_then(Value::as_str)
                {
                    absorb(&helix_ok(
                        "GET",
                        "/chat/badges",
                        &[("broadcaster_id".to_string(), id.to_string())],
                        None,
                    )?);
                }
                Ok(Value::Object(map))
            }
            // Resolve the ids EventSub conditions need, then open the socket.
            // `moderator_user_id` for channel.follow must be the *authenticated*
            // user — Helix `/users` with no login returns exactly that.
            "connect-events" => {
                let channel = s("channel")
                    .map(|c| c.trim().trim_start_matches('#').to_lowercase())
                    .filter(|c| !c.is_empty())
                    .ok_or("connect-events: `channel` is required")?;

                let id_of = |query: &[(String, String)]| -> Result<String, String> {
                    helix_ok("GET", "/users", query, None)?
                        .get("data")
                        .and_then(Value::as_array)
                        .and_then(|d| d.first())
                        .and_then(|u| u.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| "twitch: no such user".to_string())
                };
                let broadcaster = id_of(&[("login".to_string(), channel.clone())])?;
                let me = id_of(&[])?;
                kv_set("broadcaster_id", &broadcaster);
                kv_set("moderator_id", &me);

                let handle = ws_connect(&eventsub_url(), EVENTSUB_EVENT)
                    .map_err(|e| format!("twitch eventsub: connect failed: {e:?}"))?;
                EVENTSUB_HANDLE.with(|h| *h.borrow_mut() = Some(handle));
                Ok(quicktys_module_sdk::json!({
                    "channel": channel, "broadcasterId": broadcaster
                }))
            }
            "set-eventsub-url" => {
                let url = s("url").ok_or("set-eventsub-url: `url` is required")?;
                kv_set("eventsub_url", url.trim_end_matches('/'));
                Ok(Value::Null)
            }
            "set-chat-url" => {
                let url = s("url").ok_or("set-chat-url: `url` is required")?;
                kv_set("chat_url", url.trim_end_matches('/'));
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

    /// Chat frames arrive as raw IRC text (not JSON), and one frame may carry
    /// several `\r\n`-separated lines.
    fn on_host_event(event_id: &str, payload: Value) {
        // EventSub speaks JSON; chat speaks raw IRC text.
        if event_id == EVENTSUB_EVENT {
            on_eventsub(&payload);
            return;
        }
        if event_id != CHAT_EVENT {
            return;
        }
        let Some(text) = payload.as_str() else { return };
        for line in text.split('\n') {
            on_irc_line(line);
        }
    }
}

quicktys_module_sdk::define_module!(Twitch);

//! `pear` module — a certified driver for Pear Desktop's local HTTP API
//! (a youtube-music fork exposing `/api/v1` on `http://localhost:26538`, with a
//! bearer token obtained from `POST /auth/{id}`).
//!
//! Being certified and declaring the `net` capability, it may reach the network
//! through the host; a plugin never could. It exposes typed playback services
//! consumed by the `pear` plugin.

use quicktys_module_sdk::{
    json, Capability, EventDecl, Module, ModuleManifest, Schema, ServiceDecl, Value,
};
use std::cell::RefCell;

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::events::emit_event;
use quicktys::api::logging::{log, Level};
use quicktys::api::net::{fetch, ws_connect, HttpRequest};
use quicktys::api::storage::{kv_get, kv_set};

thread_local! {
    /// (song identity, paused) at the last `poll`, to detect changes.
    static LAST_SONG: RefCell<Option<(String, bool)>> = const { RefCell::new(None) };
    /// Whether we've already attempted the WebSocket connection (opened lazily on
    /// the first successful poll, so it targets the *configured* base URL).
    static WS_TRIED: RefCell<bool> = const { RefCell::new(false) };
}

/// A stable-enough identity for the current song (video id, else title).
fn song_identity(song: &Value) -> String {
    song.get("videoId")
        .and_then(Value::as_str)
        .or_else(|| song.get("title").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

/// A best-effort GET (returns None on any error) — for optional enrichment
/// endpoints that a given Pear build may or may not expose.
fn api_opt(path: &str) -> Option<Value> {
    api("GET", path, None).ok()
}

/// Extract a text field that may be a plain string, `{simpleText}`, or
/// `{runs:[{text}]}` (YouTube renderer shapes).
fn runs_text(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        return s.to_string();
    }
    if let Some(s) = v.get("simpleText").and_then(Value::as_str) {
        return s.to_string();
    }
    if let Some(runs) = v.get("runs").and_then(Value::as_array) {
        return runs
            .iter()
            .filter_map(|r| r.get("text").and_then(Value::as_str))
            .collect();
    }
    String::new()
}

/// First non-empty text among `keys` on `obj`.
fn pick_text(obj: &Value, keys: &[&str]) -> String {
    for k in keys {
        if let Some(v) = obj.get(k) {
            let t = runs_text(v);
            if !t.is_empty() {
                return t;
            }
        }
    }
    String::new()
}

/// Dig common YouTube queue wrappers down to the video renderer (or the item
/// itself if the API is already flat).
fn renderer(item: &Value) -> &Value {
    if let Some(r) = item.get("playlistPanelVideoRenderer") {
        return r;
    }
    if let Some(r) =
        item.pointer("/playlistPanelVideoWrapperRenderer/primaryRenderer/playlistPanelVideoRenderer")
    {
        return r;
    }
    item
}

/// Last (largest) thumbnail URL.
fn thumb(r: &Value) -> String {
    if let Some(url) = r
        .pointer("/thumbnail/thumbnails")
        .and_then(Value::as_array)
        .and_then(|a| a.last())
        .and_then(|t| t.get("url"))
        .and_then(Value::as_str)
    {
        return url.to_string();
    }
    r.get("thumbnail")
        .and_then(Value::as_str)
        .or_else(|| r.get("imageSrc").and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

/// Find the queue's item array wherever it sits (the response may be an array,
/// or wrap it under `items`/`queue`/`contents`/…, possibly nested).
fn find_items(v: &Value) -> Vec<Value> {
    if let Some(a) = v.as_array() {
        return a.clone();
    }
    if let Some(obj) = v.as_object() {
        for key in ["items", "queue", "contents", "tracks", "list", "songs"] {
            if let Some(a) = obj.get(key).and_then(Value::as_array) {
                return a.clone();
            }
        }
        // Recurse one level into object values (e.g. `{ queue: { items: [...] } }`).
        for val in obj.values() {
            if val.is_object() {
                let a = find_items(val);
                if !a.is_empty() {
                    return a;
                }
            }
        }
    }
    Vec::new()
}

/// Fetch the queue. `/queue` is the current endpoint (Pear/YT-Music);
/// `/queue-info` is deprecated. Logs the outcome (app console) for debugging.
fn fetch_queue() -> Option<Value> {
    for path in ["/queue", "/queue-info"] {
        match api_opt(path) {
            Some(v) => {
                let n = find_items(&v).len();
                log(Level::Info, &format!("pear queue: {path} → {n} item(s)"));
                if n > 0 {
                    return Some(v);
                }
                let peek: String = serde_json::to_string(&v)
                    .unwrap_or_default()
                    .chars()
                    .take(400)
                    .collect();
                log(Level::Debug, &format!("pear queue {path} shape: {peek}"));
            }
            None => log(Level::Debug, &format!("pear queue: {path} no response")),
        }
    }
    None
}

/// Build a structured queue (`[{title, artist, thumbnail, videoId}]`) from a
/// queue payload — defensive across API shapes.
fn queue_tracks(queue: &Value) -> Vec<Value> {
    find_items(queue)
        .iter()
        .map(|it| {
            let r = renderer(it);
            json!({
                "title": pick_text(r, &["title"]),
                "artist": pick_text(r, &["shortBylineText", "longBylineText", "subtitle", "author", "artist"]),
                "thumbnail": thumb(r),
                "videoId": r.get("videoId").or_else(|| it.get("videoId")).and_then(Value::as_str).unwrap_or(""),
                // `selected` marks the currently-playing item in Pear's queue.
                "selected": r.get("selected").or_else(|| it.get("selected")).and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// Fetch the current song and emit events when it moved since the last poll:
/// - `song`: on **any** change — a new title **or** a play/pause toggle,
///   including the first observation → the plugin mirrors it into reactive
///   variables (`pear.title`, `pear.playing`, …), so they track the player.
/// - `song-changed`: only when the **title** changes from a **known** previous
///   song (NOT the first poll) → the trigger, so launching Quicktys doesn't fire.
///
/// The heavy enrichment (queue + lyrics) runs only on a real title change.
///
/// Both the HTTP poll and the WebSocket feed call this; the shared `LAST_SONG`
/// dedups, so whichever observes a change first emits and the other is a no-op —
/// no double `song-changed`, whether updates arrive over WS (instant) or poll.
fn process_song(mut song: Value) {
    let id = song_identity(&song);
    let paused = song.get("isPaused").and_then(Value::as_bool).unwrap_or(false);
    let (sync, title_changed, is_trigger) = LAST_SONG.with(|last| {
        let mut last = last.borrow_mut();
        let prev = last.clone();
        let title_changed = prev.as_ref().map(|(pid, _)| pid != &id).unwrap_or(true);
        let changed = prev
            .as_ref()
            .map(|(pid, pp)| pid != &id || *pp != paused)
            .unwrap_or(true);
        if changed {
            *last = Some((id.clone(), paused));
        }
        (changed, title_changed, prev.is_some() && title_changed)
    });
    if !sync || id.is_empty() {
        return;
    }
    // Enrich only on a real title change (avoid wiping playlist/lyrics on a
    // pause toggle: they're then absent, so the plugin leaves them untouched).
    if title_changed {
        if let Some(obj) = song.as_object_mut() {
            if let Some(queue) = fetch_queue() {
                let tracks = queue_tracks(&queue);
                let titles: Vec<Value> = tracks
                    .iter()
                    .map(|t| t.get("title").cloned().unwrap_or(Value::Null))
                    .collect();
                obj.insert("playlist".into(), Value::Array(titles));
                obj.insert("queue".into(), Value::Array(tracks));
            }
            if let Some(lyrics) = api_opt("/lyrics") {
                let text = lyrics
                    .get("lyrics")
                    .and_then(Value::as_str)
                    .or_else(|| lyrics.as_str())
                    .unwrap_or("");
                obj.insert("lyrics".into(), Value::String(text.to_string()));
            }
        }
    }
    let payload = serde_json::to_string(&song).unwrap_or_else(|_| "{}".into());
    emit_event("song", &payload);
    if is_trigger {
        emit_event("song-changed", &payload);
    }
}

/// HTTP fallback: fetch the current song and process it. The app's poller calls
/// this every few seconds; when the WebSocket is live it finds no change.
fn poll() -> Result<Value, String> {
    let song = api("GET", "/song", None)?;
    // The player answered, so it's up: open the live WebSocket once (against the
    // now-configured base). If it fails, this poll loop stays the fallback.
    let first = WS_TRIED.with(|t| {
        let was = *t.borrow();
        *t.borrow_mut() = true;
        !was
    });
    if first {
        connect_ws();
    }
    process_song(song.clone());
    Ok(song)
}

/// Open the player's WebSocket for live push updates (`ws://…/api/v1/ws?token`).
/// Best-effort: on failure the poll fallback keeps things working.
fn connect_ws() {
    let Some(token) = token() else {
        log(Level::Debug, "pear: no token yet, WS skipped (polling)");
        return;
    };
    let ws_url = base_url().replacen("http://", "ws://", 1);
    let url = format!("{ws_url}/api/v1/ws?token={token}");
    match ws_connect(&url, "player") {
        Ok(_) => log(Level::Info, "pear: WebSocket connecting"),
        Err(e) => log(Level::Warn, &format!("pear: ws-connect failed ({e:?}), polling")),
    }
}

/// Handle a player WebSocket message: `PLAYER_INFO`/`VIDEO_CHANGED` carry a full
/// song; `PLAYER_STATE_CHANGED` carries just play/pause.
fn on_ws_message(msg: &Value) {
    match msg.get("type").and_then(Value::as_str).unwrap_or("") {
        "PLAYER_INFO" | "VIDEO_CHANGED" => {
            if let Some(song) = msg.get("song") {
                if song.is_object() {
                    process_song(song.clone());
                }
            }
        }
        "PLAYER_STATE_CHANGED" => {
            let playing = msg.get("isPlaying").and_then(Value::as_bool).unwrap_or(true);
            // Keep dedup state in sync so the poll won't re-emit.
            LAST_SONG.with(|last| {
                if let Some((_, p)) = last.borrow_mut().as_mut() {
                    *p = !playing;
                }
            });
            emit_event("song", &json!({ "isPaused": !playing }).to_string());
        }
        "POSITION_CHANGED" => {
            // Lightweight seek/tick: mirror the elapsed time only (the widget
            // interpolates between ticks for a smooth progress bar).
            if let Some(pos) = msg.get("position").and_then(Value::as_f64) {
                emit_event("song", &json!({ "elapsedSeconds": pos }).to_string());
            }
        }
        _ => {}
    }
}

/// Base URL of the Pear Desktop API. Defaults to the documented port; override
/// with the `set-base-url` service (settings UI later).
fn base_url() -> String {
    kv_get("base_url")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:26538".to_string())
}

struct Pear;

// --- Authentication ---------------------------------------------------------
//
// Pear's api-server (`src/plugins/api-server` in pear-desktop) guards `/api/*`
// with a **JWT**: `POST /auth/{id}` mints one, signed HS256 with the player's
// `secret`, and every later call presents it as `Authorization: Bearer <jwt>`.
// The player then *also* checks that the token's `id` is still in its persisted
// `authorizedClients` list — so a valid signature is not enough.
//
// The detail that dictates the design here: under the default `AUTH_AT_FIRST`
// strategy, `POST /auth/{id}` for an id the player does not know yet opens a
// **native modal dialog on the user's screen** (Allow / Deny, defaulting to
// Deny) and answers **403** if they deny. Once allowed, the id is remembered and
// later re-auth for the same id is silent.
//
// So we must never authorize implicitly in a loop: the host polls this module
// every 3s, and a naive "no token? authenticate" inside every call would pop a
// dialog every 3 seconds forever. Hence:
//   * a denial is **sticky** (`auth_state = denied`) — only an explicit
//     `authorize` call asks again;
//   * a *failed* attempt is rate-limited to one per `AUTH_RETRY_SECS`, while a
//     successful one clears the timer so a legitimately stale token recovers
//     immediately;
//   * a 401 clears the token and re-authorizes **once**, then replays the call.
//
// With `AuthStrategy::NONE` the player serves `/api/*` unauthenticated, so a
// missing token is never fatal on its own: we try the call and only authorize
// if it actually comes back 401.

/// Client id presented to the player; shown in its Allow/Deny dialog.
const DEFAULT_CLIENT_ID: &str = "quicktys";
/// Minimum delay between two *failed* authorization attempts.
const AUTH_RETRY_SECS: u64 = 60;

/// `authorized` | `denied` | `unavailable` | `""` (never tried).
fn auth_state() -> String {
    kv_get("auth_state").unwrap_or_default()
}

fn client_id() -> String {
    kv_get("client_id")
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_CLIENT_ID.to_string())
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The cached bearer token, if we hold one. Never performs I/O.
fn token() -> Option<String> {
    kv_get("token").filter(|t| !t.is_empty())
}

/// Forget the token and everything we knew about the authorization state.
fn clear_auth() {
    kv_set("token", "");
    kv_set("auth_state", "");
    kv_set("auth_attempt_at", "0");
}

/// Ask the player for a token. `explicit` = the user asked for it, which
/// bypasses both the sticky denial and the retry throttle.
fn authorize(explicit: bool) -> Result<String, String> {
    if !explicit {
        if auth_state() == "denied" {
            return Err(
                "pear desktop denied this client — call `authorize` to ask again".to_string()
            );
        }
        let last = kv_get("auth_attempt_at")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);
        let elapsed = now_secs().saturating_sub(last);
        if last > 0 && elapsed < AUTH_RETRY_SECS {
            return Err(format!(
                "authorization failed less than {AUTH_RETRY_SECS}s ago; not retrying yet"
            ));
        }
    }
    kv_set("auth_attempt_at", &now_secs().to_string());

    let id = client_id();
    let request = HttpRequest {
        method: "POST".to_string(),
        // `/auth/{id}` sits at the root, NOT under `/api/v1`.
        url: format!("{}/auth/{id}", base_url()),
        headers: Vec::new(),
        body: None,
    };
    let resp = fetch(&request).map_err(|e| {
        kv_set("auth_state", "unavailable");
        format!("network error reaching pear desktop: {e:?}")
    })?;

    match resp.status {
        200 => {
            let value: Value = serde_json::from_slice(&resp.body)
                .map_err(|e| format!("malformed /auth response: {e}"))?;
            let t = value
                .get("accessToken")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "no accessToken in the /auth response".to_string())?
                .to_string();
            kv_set("token", &t);
            kv_set("auth_state", "authorized");
            // Successful: clear the throttle so a stale token recovers at once.
            kv_set("auth_attempt_at", "0");
            log(Level::Info, &format!("pear: authorized as '{id}'"));
            Ok(t)
        }
        // The user pressed Deny in the player's dialog. Sticky: retrying on our
        // own would just reopen the dialog.
        403 => {
            kv_set("token", "");
            kv_set("auth_state", "denied");
            log(Level::Warn, "pear: authorization denied in the player");
            Err("pear desktop refused: the request was denied in the player".to_string())
        }
        status => {
            kv_set("auth_state", "unavailable");
            Err(format!("pear /auth/{id} returned {status}"))
        }
    }
}

/// One HTTP round-trip to `/api/v1<path>`, with the token if we have one.
fn api_once(
    method: &str,
    path: &str,
    body: Option<&Value>,
    bearer: Option<&str>,
) -> Result<(u16, Vec<u8>), String> {
    let mut headers = Vec::new();
    if let Some(t) = bearer {
        headers.push(("Authorization".to_string(), format!("Bearer {t}")));
    }
    let body_bytes = body.map(|b| {
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
        serde_json::to_vec(b).unwrap_or_default()
    });

    let request = HttpRequest {
        method: method.to_string(),
        url: format!("{}/api/v1{path}", base_url()),
        headers,
        body: body_bytes,
    };
    let resp = fetch(&request).map_err(|e| format!("network error: {e:?}"))?;
    Ok((resp.status, resp.body))
}

/// Perform an authenticated API call. `body` (if any) is sent as JSON.
fn api(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    // No token yet? Try to get one, but tolerate failure: the player may be
    // running with `AuthStrategy::NONE`, where `/api/*` needs none.
    let mut bearer = token();
    if bearer.is_none() {
        bearer = authorize(false).ok();
    }

    let (mut status, mut body_bytes) = api_once(method, path, body.as_ref(), bearer.as_deref())?;

    // A 401 means the token is stale — the player restarted with a fresh
    // `secret`, or dropped us from `authorizedClients`. Re-authorize once and
    // replay. Still throttled, so a player that always 401s can't spin.
    if status == 401 {
        kv_set("token", "");
        let refreshed = authorize(false)?;
        let replay = api_once(method, path, body.as_ref(), Some(&refreshed))?;
        status = replay.0;
        body_bytes = replay.1;
    }

    if status == 401 {
        return Err("unauthorized (pear desktop rejected the token)".to_string());
    }
    if status >= 400 {
        return Err(format!("pear api {path} returned {status}"));
    }
    if body_bytes.is_empty() {
        return Ok(Value::Null);
    }
    Ok(serde_json::from_slice(&body_bytes).unwrap_or(Value::Null))
}

impl Module for Pear {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("pear", "Pear Desktop", "0.1.0")
            .capability(Capability::Net)
            // Playback control (no args).
            .service(ServiceDecl::new("play"))
            .service(ServiceDecl::new("pause"))
            .service(ServiceDecl::new("toggle"))
            .service(ServiceDecl::new("next"))
            .service(ServiceDecl::new("previous"))
            .service(ServiceDecl::new("like"))
            .service(ServiceDecl::new("dislike"))
            .service(ServiceDecl::new("toggle-mute"))
            .service(ServiceDecl::new("shuffle"))
            // With args.
            .service(
                ServiceDecl::new("set-volume")
                    .args(Schema::object([("volume".to_string(), Schema::number())])),
            )
            .service(
                ServiceDecl::new("seek-to")
                    .args(Schema::object([("seconds".to_string(), Schema::number())])),
            )
            // Jump to a queue position (PATCH /queue { index }).
            .service(
                ServiceDecl::new("set-queue-index")
                    .args(Schema::object([("index".to_string(), Schema::number())])),
            )
            // Queries.
            .service(ServiceDecl::new("current-song"))
            .service(ServiceDecl::new("get-volume"))
            // Poll for a song change; emits `song` (current song, incl. first
            // observation) and `song-changed` (only on a real change).
            .service(ServiceDecl::new("poll"))
            .event(
                EventDecl::new("song").payload(Schema::object([
                    ("title".to_string(), Schema::string()),
                    ("artist".to_string(), Schema::string()),
                    ("videoId".to_string(), Schema::string()),
                ])),
            )
            .event(
                EventDecl::new("song-changed").payload(Schema::object([
                    ("title".to_string(), Schema::string()),
                    ("artist".to_string(), Schema::string()),
                    ("videoId".to_string(), Schema::string()),
                ])),
            )
            // Authentication (see the auth block above). `authorize` is the
            // explicit ask — it is the only thing that reopens the player's
            // Allow/Deny dialog after a denial.
            .service(ServiceDecl::new("authorize"))
            .service(ServiceDecl::new("auth-status"))
            .service(ServiceDecl::new("clear-auth"))
            .service(
                ServiceDecl::new("set-client-id")
                    .args(Schema::object([("id".to_string(), Schema::string())])),
            )
            // Configuration (endpoint override; a settings UI comes later).
            .service(
                ServiceDecl::new("set-base-url")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
    }

    fn on_host_event(event_id: &str, payload: Value) {
        if event_id == "player" {
            on_ws_message(&payload);
        }
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "play" => api("POST", "/play", None),
            "pause" => api("POST", "/pause", None),
            "toggle" => api("POST", "/toggle-play", None),
            "next" => api("POST", "/next", None),
            "previous" => api("POST", "/previous", None),
            "like" => api("POST", "/like", None),
            "dislike" => api("POST", "/dislike", None),
            "toggle-mute" => api("POST", "/toggle-mute", None),
            "shuffle" => api("POST", "/shuffle", None),
            "set-volume" => {
                let volume = args.get("volume").cloned().unwrap_or(json!(50));
                api("POST", "/volume", Some(json!({ "volume": volume })))
            }
            "seek-to" => {
                let seconds = args.get("seconds").cloned().unwrap_or(json!(0));
                api("POST", "/seek-to", Some(json!({ "seconds": seconds })))
            }
            "current-song" => api("GET", "/song", None),
            "get-volume" => api("GET", "/volume", None),
            "set-queue-index" => {
                let index = args.get("index").cloned().unwrap_or(json!(0));
                api("PATCH", "/queue", Some(json!({ "index": index })))
            }
            "poll" => poll(),
            // Explicit: bypasses the sticky denial and the retry throttle, so
            // this is what a "Connect to Pear" button in the UI calls.
            "authorize" => authorize(true).map(|_| {
                json!({ "state": auth_state(), "clientId": client_id() })
            }),
            "auth-status" => Ok(json!({
                "state": auth_state(),
                "authorized": token().is_some() && auth_state() == "authorized",
                "hasToken": token().is_some(),
                "clientId": client_id(),
                "baseUrl": base_url(),
            })),
            "clear-auth" => {
                clear_auth();
                Ok(Value::Null)
            }
            // A different id is a different client as far as the player is
            // concerned: the old token is useless and consent must be asked again.
            "set-client-id" => {
                let id = args.get("id").and_then(Value::as_str).unwrap_or_default().trim();
                if id.is_empty() {
                    return Err("client id is required".to_string());
                }
                kv_set("client_id", id);
                clear_auth();
                Ok(Value::Null)
            }
            "set-base-url" => {
                let url = args.get("url").and_then(Value::as_str).unwrap_or_default();
                kv_set("base_url", url);
                clear_auth(); // another player instance → another secret, re-auth
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Pear);

//! `obs` module — certified driver for **obs-websocket v5** (OBS Studio).
//!
//! Being certified and declaring the `net` capability, it may reach OBS over a
//! WebSocket through the host. It performs the v5 handshake (Hello → Identify,
//! with SHA-256 auth when the server requires it), subscribes to OBS events, and
//! re-emits the meaningful ones as **module events** that the `obs` plugin turns
//! into user-facing triggers + reactive state. Connection is opened lazily on the
//! first `poll` (so it targets the *configured* endpoint), like the `pear` module.

use quicktys_module_sdk::{json, Capability, EventDecl, Module, ModuleManifest, Schema, ServiceDecl, Value};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeSet, HashMap};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::events::emit_event;
use quicktys::api::logging::{log, Level};
use quicktys::api::net::{close, send, ws_connect};
use quicktys::api::storage::{kv_get, kv_set};

thread_local! {
    /// The `ws-connect` handle (to `net.send` the Identify + requests). `None`
    /// means disconnected → the next poll (re)connects.
    static HANDLE: RefCell<Option<u64>> = const { RefCell::new(None) };
    /// Monotonic obs-websocket request id.
    static REQ_SEQ: RefCell<u64> = const { RefCell::new(0) };
    /// A `GetSceneItemId` request awaiting its response, mapping requestId →
    /// (follow-up requestType, scene name, extra fields). When the id resolves we
    /// send `<follow-up> { sceneName, sceneItemId, ...extra }`.
    static PENDING: RefCell<HashMap<String, (String, String, Value)>> = RefCell::new(HashMap::new());
    /// All filter names seen across sources (for the `obs-filter` autocomplete).
    static FILTERS: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
    /// The current program scene (target for `add-browser-source`).
    static CURRENT_SCENE: RefCell<String> = const { RefCell::new(String::new()) };
}

fn next_request_id() -> String {
    REQ_SEQ.with(|s| {
        let mut s = s.borrow_mut();
        *s += 1;
        format!("fm-{s}")
    })
}

/// obs-websocket `EventSubscription` bitmask: General | Scenes | Inputs |
/// Outputs | MediaInputs | Ui — scene / input / stream / record / replay / vcam
/// / media playback / studio.
const EVENT_SUBS: u64 = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 6) | (1 << 8) | (1 << 10);

/// The requests we fire once on connect to seed the reactive `obs.*` variables.
const SYNC_REQUESTS: &[&str] = &[
    "GetSceneList",
    "GetInputList",
    "GetStreamStatus",
    "GetRecordStatus",
    "GetVirtualCamStatus",
    "GetReplayBufferStatus",
    "GetStudioModeEnabled",
    "GetProfileList",
    "GetSceneCollectionList",
    "GetSceneTransitionList",
    "GetVideoSettings",
    "GetHotkeyList",
    "GetStats",
];
/// Requests re-fired on every poll to keep live values fresh (durations, stats).
const POLL_REQUESTS: &[&str] = &["GetStats", "GetStreamStatus", "GetRecordStatus"];

struct Obs;

fn base_url() -> String {
    kv_get("url")
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "ws://127.0.0.1:4455".to_string())
}
fn password() -> String {
    kv_get("password").unwrap_or_default()
}

/// Drop the current connection so the next `poll` reconnects — used when the URL
/// or password changes, or when a send fails (OBS closed / restarted).
fn reset_connection() {
    if let Some(h) = HANDLE.with(|x| x.borrow_mut().take()) {
        close(h);
    }
}

/// Open the obs-websocket connection (frames delivered to `on_host_event`).
fn connect() {
    let url = base_url();
    match ws_connect(&url, "obs") {
        Ok(h) => {
            HANDLE.with(|x| *x.borrow_mut() = Some(h));
            log(Level::Info, &format!("obs: connecting {url}"));
        }
        Err(e) => log(Level::Warn, &format!("obs: ws-connect failed ({e:?})")),
    }
}

/// Send a JSON message on the connection. Returns `false` if there is no live
/// connection or the write failed (the socket is dead → reconnect).
fn ws_send(v: &Value) -> bool {
    match HANDLE.with(|x| *x.borrow()) {
        Some(h) => match send(h, v.to_string().as_bytes()) {
            Ok(()) => true,
            Err(e) => {
                log(Level::Debug, &format!("obs: send failed ({e:?})"));
                false
            }
        },
        None => false,
    }
}

/// Send an obs-websocket request (op 6) with empty data.
fn request(request_type: &str) {
    request_with_data(request_type, json!({}));
}

/// Send an obs-websocket request with data, returning its request id.
fn request_with_data(request_type: &str, data: Value) -> String {
    let id = next_request_id();
    ws_send(&json!({
        "op": 6,
        "d": { "requestType": request_type, "requestId": id, "requestData": data }
    }));
    id
}

/// Resolve a source to its scene-item id (`GetSceneItemId`), then fire
/// `follow` with `{ sceneName, sceneItemId, ...extra }` once the id comes back.
fn resolve_scene_item(scene: &str, source: &str, follow: &str, extra: Value) {
    let id = request_with_data(
        "GetSceneItemId",
        json!({ "sceneName": scene, "sourceName": source }),
    );
    PENDING.with(|p| {
        p.borrow_mut().insert(id, (follow.to_string(), scene.to_string(), extra));
    });
}

/// obs-websocket auth: base64(sha256(base64(sha256(password + salt)) + challenge)).
fn auth_string(password: &str, salt: &str, challenge: &str) -> String {
    let sha = |s: &str| {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        h.finalize().to_vec()
    };
    let secret = base64(&sha(&format!("{password}{salt}")));
    base64(&sha(&format!("{secret}{challenge}")))
}

/// Handle one obs-websocket message (`{ op, d }`).
fn handle_message(msg: &Value) {
    let op = msg.get("op").and_then(Value::as_u64).unwrap_or(u64::MAX);
    let d = msg.get("d").cloned().unwrap_or(Value::Null);
    match op {
        0 => {
            // Hello → reply with Identify (authenticating if challenged).
            let mut identify = json!({ "rpcVersion": 1, "eventSubscriptions": EVENT_SUBS });
            if let Some(auth) = d.get("authentication") {
                if let (Some(challenge), Some(salt)) = (
                    auth.get("challenge").and_then(Value::as_str),
                    auth.get("salt").and_then(Value::as_str),
                ) {
                    identify["authentication"] =
                        Value::String(auth_string(&password(), salt, challenge));
                }
            }
            ws_send(&json!({ "op": 1, "d": identify }));
        }
        2 => {
            log(Level::Info, "obs: identified");
            // Seed the reactive variables with OBS's current state.
            for r in SYNC_REQUESTS {
                request(r);
            }
        }
        5 => {
            let ev = d.get("eventType").and_then(Value::as_str).unwrap_or("");
            let data = d.get("eventData").cloned().unwrap_or(Value::Null);
            dispatch_event(ev, &data);
        }
        7 => {
            let rtype = d.get("requestType").and_then(Value::as_str).unwrap_or("");
            let req_id = d.get("requestId").and_then(Value::as_str).unwrap_or("");
            let data = d.get("responseData").cloned().unwrap_or(Value::Null);
            // A resolved GetSceneItemId → fire its chained follow-up request.
            if rtype == "GetSceneItemId" {
                if let Some((follow, scene, mut extra)) = PENDING.with(|p| p.borrow_mut().remove(req_id))
                {
                    if let Some(obj) = extra.as_object_mut() {
                        obj.insert("sceneName".into(), Value::String(scene));
                        obj.insert("sceneItemId".into(), data.get("sceneItemId").cloned().unwrap_or(Value::Null));
                    }
                    request_with_data(&follow, extra);
                }
                return;
            }
            // A source's filter list → merge its names into the global set.
            if rtype == "GetSourceFilterList" {
                aggregate_filters(&data);
                return;
            }
            dispatch_response(rtype, &data);
            // Once the input list is known, fetch each input's filters.
            if rtype == "GetInputList" {
                if let Some(inputs) = data.get("inputs").and_then(Value::as_array) {
                    for input in inputs {
                        if let Some(name) = input.get("inputName").and_then(Value::as_str) {
                            request_with_data("GetSourceFilterList", json!({ "sourceName": name }));
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// Map an obs-websocket event to a module event the plugin consumes.
fn dispatch_event(ev: &str, data: &Value) {
    match ev {
        "CurrentProgramSceneChanged" => {
            let scene = data.get("sceneName").and_then(Value::as_str).unwrap_or("");
            CURRENT_SCENE.with(|s| *s.borrow_mut() = scene.to_string());
            emit("scene-changed", json!({ "scene": scene }));
        }
        "StreamStateChanged" => {
            if let Some(a) = output_active(data) {
                emit("stream-state", json!({ "active": a }));
            }
        }
        "RecordStateChanged" => {
            if let Some(a) = output_active(data) {
                emit("record-state", json!({ "active": a }));
            }
        }
        "ReplayBufferStateChanged" => {
            if let Some(a) = output_active(data) {
                emit("replay-state", json!({ "active": a }));
            }
        }
        "VirtualcamStateChanged" => {
            if let Some(a) = output_active(data) {
                emit("vcam-state", json!({ "active": a }));
            }
        }
        "StudioModeStateChanged" => {
            let enabled = data
                .get("studioModeEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            emit("studio-state", json!({ "enabled": enabled }));
        }
        "MediaInputPlaybackEnded" => {
            let source = data.get("inputName").and_then(Value::as_str).unwrap_or("");
            emit("media-ended", json!({ "source": source }));
        }
        "MediaInputPlaybackStarted" => {
            let source = data.get("inputName").and_then(Value::as_str).unwrap_or("");
            emit("media-started", json!({ "source": source }));
        }
        "ReplayBufferSaved" => {
            let path = data.get("savedReplayPath").and_then(Value::as_str).unwrap_or("");
            emit("replay-saved", json!({ "path": path }));
        }
        // Keep the scene / input lists fresh when OBS's set changes.
        "SceneListChanged" | "SceneCreated" | "SceneRemoved" | "SceneNameChanged" => {
            request("GetSceneList");
        }
        "InputCreated" | "InputRemoved" | "InputNameChanged" => request("GetInputList"),
        "CurrentProfileChanged" => request("GetProfileList"),
        "CurrentSceneCollectionChanged" => request("GetSceneCollectionList"),
        _ => {}
    }
}

/// Turn a request response into a `state` module event carrying reactive `obs.*`
/// variables. State-only (never a trigger); the plugin mirrors present keys.
fn dispatch_response(request_type: &str, data: &Value) {
    let names = |key: &str, field: &str| -> Value {
        data.get(key)
            .and_then(Value::as_array)
            .map(|a| {
                Value::Array(
                    a.iter()
                        .filter_map(|o| o.get(field).cloned())
                        .collect(),
                )
            })
            .unwrap_or(Value::Null)
    };
    let get = |k: &str| data.get(k).cloned().unwrap_or(Value::Null);

    let state = match request_type {
        "GetSceneList" => {
            if let Some(scene) = data.get("currentProgramSceneName").and_then(Value::as_str) {
                CURRENT_SCENE.with(|s| *s.borrow_mut() = scene.to_string());
            }
            json!({
                "scenes": names("scenes", "sceneName"),
                "scene": get("currentProgramSceneName"),
            })
        }
        "GetInputList" => json!({ "inputs": names("inputs", "inputName") }),
        "GetStreamStatus" => json!({
            "streaming": get("outputActive"),
            "stream-timecode": get("outputTimecode"),
            "stream-duration": get("outputDuration"),
            "stream-congestion": get("outputCongestion"),
            "stream-skipped-frames": get("outputSkippedFrames"),
        }),
        "GetRecordStatus" => json!({
            "recording": get("outputActive"),
            "record-timecode": get("outputTimecode"),
            "record-duration": get("outputDuration"),
        }),
        "GetVirtualCamStatus" => json!({ "virtual-cam": get("outputActive") }),
        "GetReplayBufferStatus" => json!({ "replay-buffering": get("outputActive") }),
        "GetStudioModeEnabled" => json!({ "studio-mode": get("studioModeEnabled") }),
        "GetProfileList" => json!({
            "profile": get("currentProfileName"),
            "profiles": get("profiles"),
        }),
        "GetSceneCollectionList" => json!({ "scene-collection": get("currentSceneCollectionName") }),
        "GetSceneTransitionList" => json!({
            "transitions": names("transitions", "transitionName"),
            "transition": get("currentSceneTransitionName"),
        }),
        "GetHotkeyList" => json!({ "hotkeys": get("hotkeys") }),
        "GetVideoSettings" => json!({
            "canvas-width": get("baseWidth"),
            "canvas-height": get("baseHeight"),
            "output-width": get("outputWidth"),
            "output-height": get("outputHeight"),
        }),
        "GetStats" => json!({
            "fps": get("activeFps"),
            "cpu-usage": get("cpuUsage"),
            "memory-usage": get("memoryUsage"),
            "free-disk-space": get("availableDiskSpace"),
            "render-skipped-frames": get("renderSkippedFrames"),
            "render-total-frames": get("renderTotalFrames"),
            "output-skipped-frames": get("outputSkippedFrames"),
            "output-total-frames": get("outputTotalFrames"),
        }),
        _ => return,
    };
    emit("state", state);
}

/// Final on/off for an output event, ignoring transitional states (STARTING /
/// STOPPING / PAUSED / RESUMED) so we only fire on real start/stop.
fn output_active(data: &Value) -> Option<bool> {
    if let Some(state) = data.get("outputState").and_then(Value::as_str) {
        if state.ends_with("_STARTED") {
            return Some(true);
        }
        if state.ends_with("_STOPPED") {
            return Some(false);
        }
        return None;
    }
    data.get("outputActive").and_then(Value::as_bool)
}

fn emit(id: &str, payload: Value) {
    emit_event(id, &payload.to_string());
}

/// Merge a `GetSourceFilterList` response's filter names into the global set and
/// publish the sorted list as `obs.filters` (for the `obs-filter` autocomplete).
fn aggregate_filters(data: &Value) {
    let Some(filters) = data.get("filters").and_then(Value::as_array) else { return };
    let mut changed = false;
    FILTERS.with(|f| {
        let mut set = f.borrow_mut();
        for filter in filters {
            if let Some(name) = filter.get("filterName").and_then(Value::as_str) {
                changed |= set.insert(name.to_string());
            }
        }
    });
    if changed {
        let list: Vec<Value> = FILTERS.with(|f| f.borrow().iter().map(|s| json!(s)).collect());
        emit("state", json!({ "filters": list }));
    }
}

impl Module for Obs {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("obs", "OBS Studio", "0.1.0")
            .capability(Capability::Net)
            .service(
                ServiceDecl::new("set-url")
                    .args(Schema::object([("url".to_string(), Schema::string())])),
            )
            .service(
                ServiceDecl::new("set-password")
                    .args(Schema::object([("password".to_string(), Schema::string())])),
            )
            .service(ServiceDecl::new("poll"))
            // Generic obs-websocket request (op 6). The `obs` plugin's actions
            // map to `{ type: <requestType>, data: <requestData> }`.
            .service(ServiceDecl::new("request").args(Schema::object([
                ("type".to_string(), Schema::string()),
                ("data".to_string(), Schema::object([])),
            ])))
            // Scene-item requests (need a GetSceneItemId round-trip first).
            .service(ServiceDecl::new("set-source-enabled").args(Schema::object([
                ("scene".to_string(), Schema::string()),
                ("source".to_string(), Schema::string()),
                ("enabled".to_string(), Schema::boolean()),
            ])))
            .service(ServiceDecl::new("set-source-transform").args(Schema::object([
                ("scene".to_string(), Schema::string()),
                ("source".to_string(), Schema::string()),
                ("transform".to_string(), Schema::object([])),
            ])))
            .service(ServiceDecl::new("add-browser-source").args(Schema::object([
                ("url".to_string(), Schema::string()),
                ("name".to_string(), Schema::string()),
            ])))
            .event(
                EventDecl::new("scene-changed")
                    .payload(Schema::object([("scene".to_string(), Schema::string())])),
            )
            .event(
                EventDecl::new("stream-state")
                    .payload(Schema::object([("active".to_string(), Schema::boolean())])),
            )
            .event(
                EventDecl::new("record-state")
                    .payload(Schema::object([("active".to_string(), Schema::boolean())])),
            )
            .event(
                EventDecl::new("replay-state")
                    .payload(Schema::object([("active".to_string(), Schema::boolean())])),
            )
            .event(
                EventDecl::new("vcam-state")
                    .payload(Schema::object([("active".to_string(), Schema::boolean())])),
            )
            .event(
                EventDecl::new("studio-state")
                    .payload(Schema::object([("enabled".to_string(), Schema::boolean())])),
            )
            .event(
                EventDecl::new("media-ended")
                    .payload(Schema::object([("source".to_string(), Schema::string())])),
            )
            .event(
                EventDecl::new("media-started")
                    .payload(Schema::object([("source".to_string(), Schema::string())])),
            )
            .event(
                EventDecl::new("replay-saved")
                    .payload(Schema::object([("path".to_string(), Schema::string())])),
            )
            // Bulk state sync (scene/input lists, statuses, stats) — no trigger.
            .event(EventDecl::new("state").payload(Schema::object([])))
    }

    fn on_host_event(event_id: &str, payload: Value) {
        if event_id == "obs" {
            handle_message(&payload);
        }
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "set-url" => {
                let url = args.get("url").and_then(Value::as_str).unwrap_or_default();
                kv_set("url", url);
                reset_connection();
                Ok(Value::Null)
            }
            "set-password" => {
                let pw = args.get("password").and_then(Value::as_str).unwrap_or_default();
                kv_set("password", pw);
                reset_connection();
                Ok(Value::Null)
            }
            "poll" => {
                // Disconnected (first poll, or after OBS closed) → (re)connect.
                if HANDLE.with(|x| x.borrow().is_none()) {
                    connect();
                } else {
                    // Refresh live values; a failed send means the socket died
                    // (OBS closed/restarted) → drop it and reconnect next poll.
                    let alive = POLL_REQUESTS.iter().all(|r| {
                        ws_send(&json!({
                            "op": 6,
                            "d": { "requestType": r, "requestId": next_request_id(), "requestData": {} }
                        }))
                    });
                    if !alive {
                        log(Level::Info, "obs: connection lost, reconnecting");
                        reset_connection();
                        connect();
                    }
                }
                Ok(Value::Null)
            }
            "request" => {
                let rtype = args.get("type").and_then(Value::as_str).unwrap_or_default();
                if rtype.is_empty() {
                    return Err("request type is required".to_string());
                }
                let data = args.get("data").cloned().unwrap_or_else(|| json!({}));
                request_with_data(rtype, data);
                Ok(Value::Null)
            }
            "set-source-enabled" => {
                let scene = args.get("scene").and_then(Value::as_str).unwrap_or_default();
                let source = args.get("source").and_then(Value::as_str).unwrap_or_default();
                let enabled = args.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                resolve_scene_item(
                    scene,
                    source,
                    "SetSceneItemEnabled",
                    json!({ "sceneItemEnabled": enabled }),
                );
                Ok(Value::Null)
            }
            "set-source-transform" => {
                let scene = args.get("scene").and_then(Value::as_str).unwrap_or_default();
                let source = args.get("source").and_then(Value::as_str).unwrap_or_default();
                let transform = args.get("transform").cloned().unwrap_or_else(|| json!({}));
                resolve_scene_item(
                    scene,
                    source,
                    "SetSceneItemTransform",
                    json!({ "sceneItemTransform": transform }),
                );
                Ok(Value::Null)
            }
            "add-browser-source" => {
                let url = args.get("url").and_then(Value::as_str).unwrap_or_default();
                let name = args.get("name").and_then(Value::as_str).unwrap_or("Quicktys Overlay");
                let scene = CURRENT_SCENE.with(|s| s.borrow().clone());
                if scene.is_empty() {
                    return Err("no current scene yet".to_string());
                }
                request_with_data(
                    "CreateInput",
                    json!({
                        "sceneName": scene,
                        "inputName": name,
                        "inputKind": "browser_source",
                        "inputSettings": { "url": url, "width": 1920, "height": 1080 }
                    }),
                );
                Ok(Value::Null)
            }
            other => Err(format!("unknown method: {other}")),
        }
    }
}

/// Standard base64 (RFC 4648, padded).
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

quicktys_module_sdk::define_module!(Obs);

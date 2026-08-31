//! The `obs` path end to end: the module connects to a stand-in obs-websocket
//! server, completes the v5 handshake (**Hello → Identify**, exercising the new
//! `net.send` capability), receives a `CurrentProgramSceneChanged` event, and the
//! `obs` plugin turns it into a host trigger (`obs.scene-changed`) + mirrors
//! `obs.scene`. A raw TCP stand-in plays the server, so we never touch real OBS.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

/// Send an unmasked server text frame (server → client frames are unmasked).
fn send_text(stream: &mut TcpStream, s: &str) {
    let mut f = vec![0x81u8]; // FIN + text
    let b = s.as_bytes();
    if b.len() < 126 {
        f.push(b.len() as u8);
    } else {
        f.push(126);
        f.extend_from_slice(&(b.len() as u16).to_be_bytes());
    }
    f.extend_from_slice(b);
    let _ = stream.write_all(&f);
}

/// Read one masked client text frame and return its payload.
fn read_text(stream: &mut TcpStream) -> Option<String> {
    let mut hdr = [0u8; 2];
    stream.read_exact(&mut hdr).ok()?;
    let mut len = (hdr[1] & 0x7f) as usize;
    if len == 126 {
        let mut ext = [0u8; 2];
        stream.read_exact(&mut ext).ok()?;
        len = u16::from_be_bytes(ext) as usize;
    }
    let mut mask = [0u8; 4];
    stream.read_exact(&mut mask).ok()?; // client frames are masked
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).ok()?;
    for (i, b) in payload.iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Some(String::from_utf8_lossy(&payload).into_owned())
}

/// A stand-in obs-websocket v5 server. Sends the `Identify` payload it received
/// back over `ident_tx`, then pushes a scene-changed event.
fn spawn_obs(ident_tx: mpsc::Sender<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        // Read the client's HTTP Upgrade request, answer 101 (our client doesn't
        // validate Sec-WebSocket-Accept).
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        // Hello (op 0), no auth challenge.
        send_text(&mut stream, &json!({ "op": 0, "d": { "rpcVersion": 1 } }).to_string());
        // The module replies with Identify (op 1) — capture it.
        if let Some(ident) = read_text(&mut stream) {
            let _ = ident_tx.send(ident);
        }
        // Identified (op 2), then a scene change event (op 5).
        send_text(&mut stream, &json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string());
        send_text(
            &mut stream,
            &json!({
                "op": 5,
                "d": { "eventType": "CurrentProgramSceneChanged", "eventData": { "sceneName": "Gameplay" } }
            })
            .to_string(),
        );
        std::thread::sleep(Duration::from_millis(400));
    });
    url
}

/// A stand-in that, after the handshake, forwards each client frame it reads
/// over `tx` (first the Identify, then whatever request the action sends).
fn spawn_obs_capture(tx: mpsc::Sender<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        send_text(&mut stream, &json!({ "op": 0, "d": { "rpcVersion": 1 } }).to_string());
        // First frame is the Identify — ack it, then forward every subsequent
        // client frame (the sync requests, then the action's request).
        let Some(ident) = read_text(&mut stream) else { return };
        if tx.send(ident).is_err() {
            return;
        }
        send_text(&mut stream, &json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string());
        while let Some(frame) = read_text(&mut stream) {
            if tx.send(frame).is_err() {
                return;
            }
        }
    });
    url
}

/// A stand-in that answers the first sync request (GetSceneList) with a scene
/// list — to exercise the op-7 round-trip → `obs.*` variables.
fn spawn_obs_scene_list() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        send_text(&mut stream, &json!({ "op": 0, "d": { "rpcVersion": 1 } }).to_string());
        let _ = read_text(&mut stream); // Identify
        send_text(&mut stream, &json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string());
        // The module now fires its sync requests; GetSceneList is first.
        let _ = read_text(&mut stream);
        send_text(
            &mut stream,
            &json!({
                "op": 7,
                "d": {
                    "requestType": "GetSceneList",
                    "requestId": "fm-1",
                    "requestStatus": { "result": true, "code": 100 },
                    "responseData": {
                        "currentProgramSceneName": "Intro",
                        "scenes": [{ "sceneName": "Intro" }, { "sceneName": "Gameplay" }]
                    }
                }
            })
            .to_string(),
        );
        std::thread::sleep(Duration::from_millis(400));
    });
    url
}

/// A stand-in that answers `GetSceneItemId` with a fixed id and forwards every
/// client frame — to exercise the round-trip action chain.
fn spawn_obs_scene_item(tx: mpsc::Sender<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        send_text(&mut stream, &json!({ "op": 0, "d": { "rpcVersion": 1 } }).to_string());
        let Some(ident) = read_text(&mut stream) else { return };
        let _ = tx.send(ident);
        send_text(&mut stream, &json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string());
        while let Some(frame) = read_text(&mut stream) {
            let v: Value = serde_json::from_str(&frame).unwrap_or(Value::Null);
            if v["d"].get("requestType").and_then(Value::as_str) == Some("GetSceneItemId") {
                let id = v["d"].get("requestId").and_then(Value::as_str).unwrap_or("");
                send_text(
                    &mut stream,
                    &json!({
                        "op": 7,
                        "d": {
                            "requestType": "GetSceneItemId",
                            "requestId": id,
                            "requestStatus": { "result": true, "code": 100 },
                            "responseData": { "sceneItemId": 5 }
                        }
                    })
                    .to_string(),
                );
            }
            if tx.send(frame).is_err() {
                return;
            }
        }
    });
    url
}

fn build(dir_rel: &str, name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dir_rel);
    let wasm = dir.join(format!("target/wasm32-wasip2/release/{name}.wasm"));
    if !wasm.exists() {
        let status = Command::new("cargo")
            .args(["build", "--target", "wasm32-wasip2", "--release"])
            .current_dir(&dir)
            .status()
            .unwrap_or_else(|_| panic!("build {name}"));
        assert!(status.success());
    }
    wasm
}

#[tokio::test]
async fn obs_scene_change_fires_a_trigger_after_handshake() {
    let (ident_tx, ident_rx) = mpsc::channel();
    let url = spawn_obs(ident_tx);
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus.clone()).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true)
        .expect("load obs module");
    host.load_plugin(&build("../../plugins/obs", "obs_plugin"))
        .expect("load obs plugin");
    host.call_module("obs", "set-url", json!({ "url": url })).expect("set-url");

    let mut rx = bus.subscribe();

    // First poll opens the WS; the reader thread then drives the handshake + event.
    host.call_module("obs", "poll", json!({})).expect("poll");

    // The module sent an Identify over `net.send` — check the handshake reply.
    let ident = ident_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("module sent Identify");
    let ident: Value = serde_json::from_str(&ident).unwrap();
    assert_eq!(ident.get("op").and_then(Value::as_u64), Some(1));
    assert_eq!(ident["d"].get("rpcVersion").and_then(Value::as_u64), Some(1));
    assert!(ident["d"].get("eventSubscriptions").is_some());

    // The scene event became a host trigger with the new scene as context.
    let ev = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("trigger within 2s")
        .expect("bus open");
    assert_eq!(ev.id, "obs.scene-changed");
    assert_eq!(ev.context.get("scene").and_then(Value::as_str), Some("Gameplay"));

    // And `obs.scene` was mirrored into reactive state.
    assert_eq!(state.get("obs.scene"), Some(json!("Gameplay")));
}

#[tokio::test]
async fn obs_action_sends_the_matching_request() {
    let (tx, rx) = mpsc::channel();
    let url = spawn_obs_capture(tx);
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state, bus).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true)
        .expect("load obs module");
    host.load_plugin(&build("../../plugins/obs", "obs_plugin"))
        .expect("load obs plugin");
    host.call_module("obs", "set-url", json!({ "url": url })).expect("set-url");
    host.call_module("obs", "poll", json!({})).expect("poll");

    // Wait for the Identify — proves the connection is up before we act.
    let ident = rx.recv_timeout(Duration::from_secs(2)).expect("Identify");
    assert!(serde_json::from_str::<Value>(&ident).unwrap()["d"]
        .get("rpcVersion")
        .is_some());

    // Invoke the plugin action → it must send the mapped obs-websocket request.
    host.invoker()
        .invoke("obs", "set-scene", json!({ "scene": "BRB" }))
        .await
        .expect("action ok");

    // Among the frames (sync requests + the action), find SetCurrentProgramScene.
    let mut found = None;
    while let Ok(frame) = rx.recv_timeout(Duration::from_secs(2)) {
        let v: Value = serde_json::from_str(&frame).unwrap();
        if v["d"].get("requestType").and_then(Value::as_str) == Some("SetCurrentProgramScene") {
            found = Some(v);
            break;
        }
    }
    let req = found.expect("the action sent a SetCurrentProgramScene request");
    assert_eq!(req.get("op").and_then(Value::as_u64), Some(6));
    assert_eq!(req["d"]["requestData"].get("sceneName").and_then(Value::as_str), Some("BRB"));

    // A media-control action maps its friendly enum to the OBS constant.
    host.invoker()
        .invoke("obs", "media-control", json!({ "source": "Clip", "action": "pause" }))
        .await
        .expect("action ok");
    let mut media = None;
    while let Ok(frame) = rx.recv_timeout(Duration::from_secs(2)) {
        let v: Value = serde_json::from_str(&frame).unwrap();
        if v["d"].get("requestType").and_then(Value::as_str) == Some("TriggerMediaInputAction") {
            media = Some(v);
            break;
        }
    }
    let m = media.expect("media-control sent TriggerMediaInputAction");
    assert_eq!(m["d"]["requestData"].get("inputName").and_then(Value::as_str), Some("Clip"));
    assert_eq!(
        m["d"]["requestData"].get("mediaAction").and_then(Value::as_str),
        Some("OBS_WEBSOCKET_MEDIA_INPUT_ACTION_PAUSE")
    );
}

#[tokio::test]
async fn obs_state_sync_populates_variables_without_a_trigger() {
    let url = spawn_obs_scene_list();
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus.clone()).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true)
        .expect("load obs module");
    host.load_plugin(&build("../../plugins/obs", "obs_plugin"))
        .expect("load obs plugin");
    host.call_module("obs", "set-url", json!({ "url": url })).expect("set-url");

    let mut rx = bus.subscribe();
    host.call_module("obs", "poll", json!({})).expect("poll");

    // The op-7 GetSceneList response mirrors the scene list into reactive state.
    let mut scenes = None;
    for _ in 0..40 {
        if let Some(v) = state.get("obs.scenes") {
            scenes = Some(v);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(scenes, Some(json!(["Intro", "Gameplay"])));
    assert_eq!(state.get("obs.scene"), Some(json!("Intro")));

    // State sync must NOT fire any trigger (it reflects current state, not change).
    assert!(rx.try_recv().is_err(), "state sync must not fire a trigger");
}

#[tokio::test]
async fn obs_source_visibility_resolves_scene_item_then_sets_it() {
    let (tx, rx) = mpsc::channel();
    let url = spawn_obs_scene_item(tx);
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state, bus).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true).unwrap();
    host.load_plugin(&build("../../plugins/obs", "obs_plugin")).unwrap();
    host.call_module("obs", "set-url", json!({ "url": url })).unwrap();
    host.call_module("obs", "poll", json!({})).unwrap();

    // Connection up.
    rx.recv_timeout(Duration::from_secs(2)).expect("Identify");

    host.invoker()
        .invoke(
            "obs",
            "set-source-visibility",
            json!({ "scene": "Main", "source": "Cam", "visible": false }),
        )
        .await
        .expect("action ok");

    // The chain must end in SetSceneItemEnabled with the resolved id + flag.
    let mut set = None;
    while let Ok(frame) = rx.recv_timeout(Duration::from_secs(2)) {
        let v: Value = serde_json::from_str(&frame).unwrap();
        if v["d"].get("requestType").and_then(Value::as_str) == Some("SetSceneItemEnabled") {
            set = Some(v);
            break;
        }
    }
    let req = set.expect("SetSceneItemEnabled was sent after GetSceneItemId");
    assert_eq!(req["d"]["requestData"].get("sceneName").and_then(Value::as_str), Some("Main"));
    assert_eq!(req["d"]["requestData"].get("sceneItemId").and_then(Value::as_u64), Some(5));
    assert_eq!(req["d"]["requestData"].get("sceneItemEnabled").and_then(Value::as_bool), Some(false));
}

/// A stand-in that answers GetSceneList (setting the current scene) and forwards
/// every client frame — for the "Add to OBS" path.
fn spawn_obs_with_scene(tx: mpsc::Sender<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("ws://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let _ = stream.write_all(
            b"HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\r\n",
        );
        send_text(&mut stream, &json!({ "op": 0, "d": { "rpcVersion": 1 } }).to_string());
        let Some(ident) = read_text(&mut stream) else { return };
        let _ = tx.send(ident);
        send_text(&mut stream, &json!({ "op": 2, "d": { "negotiatedRpcVersion": 1 } }).to_string());
        while let Some(frame) = read_text(&mut stream) {
            let v: Value = serde_json::from_str(&frame).unwrap_or(Value::Null);
            if v["d"].get("requestType").and_then(Value::as_str) == Some("GetSceneList") {
                send_text(
                    &mut stream,
                    &json!({
                        "op": 7,
                        "d": {
                            "requestType": "GetSceneList",
                            "requestId": v["d"].get("requestId"),
                            "requestStatus": { "result": true, "code": 100 },
                            "responseData": { "currentProgramSceneName": "Live", "scenes": [{ "sceneName": "Live" }] }
                        }
                    })
                    .to_string(),
                );
            }
            if tx.send(frame).is_err() {
                return;
            }
        }
    });
    url
}

#[tokio::test]
async fn obs_add_browser_source_targets_the_current_scene() {
    let (tx, rx) = mpsc::channel();
    let url = spawn_obs_with_scene(tx);
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true).unwrap();
    host.load_plugin(&build("../../plugins/obs", "obs_plugin")).unwrap();
    host.call_module("obs", "set-url", json!({ "url": url })).unwrap();
    host.call_module("obs", "poll", json!({})).unwrap();
    rx.recv_timeout(Duration::from_secs(2)).expect("Identify");

    // Wait until the current scene is synced (obs.scene = "Live").
    for _ in 0..40 {
        if state.get("obs.scene") == Some(json!("Live")) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    host.invoker()
        .invoke("obs", "add-browser-source", json!({ "url": "http://localhost:8787/overlay/x", "name": "Overlay X" }))
        .await
        .expect("action ok");

    let mut created = None;
    while let Ok(frame) = rx.recv_timeout(Duration::from_secs(2)) {
        let v: Value = serde_json::from_str(&frame).unwrap();
        if v["d"].get("requestType").and_then(Value::as_str) == Some("CreateInput") {
            created = Some(v);
            break;
        }
    }
    let req = created.expect("add-browser-source sent CreateInput");
    let data = &req["d"]["requestData"];
    assert_eq!(data.get("sceneName").and_then(Value::as_str), Some("Live"));
    assert_eq!(data.get("inputKind").and_then(Value::as_str), Some("browser_source"));
    assert_eq!(data.get("inputName").and_then(Value::as_str), Some("Overlay X"));
    assert_eq!(data["inputSettings"].get("url").and_then(Value::as_str), Some("http://localhost:8787/overlay/x"));
}

/// Read-only smoke test against a *real* OBS. Run with:
///   FM_OBS_PASSWORD=... cargo test -p quicktys-host --test obs_trigger --ignored
#[tokio::test]
#[ignore = "needs a running OBS (set FM_OBS_PASSWORD)"]
async fn real_obs_populates_variables() {
    let password = std::env::var("FM_OBS_PASSWORD").unwrap_or_default();
    let state = Arc::new(StateStore::new());
    let bus = Arc::new(TriggerBus::new());
    let host = WasmHost::new(state.clone(), bus).unwrap();
    host.load_module(&build("../../modules/obs", "obs_module"), true).unwrap();
    host.load_plugin(&build("../../plugins/obs", "obs_plugin")).unwrap();
    host.call_module("obs", "set-url", json!({ "url": "ws://127.0.0.1:4455" })).unwrap();
    host.call_module("obs", "set-password", json!({ "password": password })).unwrap();
    host.call_module("obs", "poll", json!({})).unwrap();

    let mut scenes = None;
    for _ in 0..60 {
        if let Some(v) = state.get("obs.scenes") {
            scenes = Some(v);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let scenes = scenes.expect("obs.scenes populated from real OBS");
    println!("obs.scenes = {scenes}");
    println!("obs.scene = {:?}", state.get("obs.scene"));
    println!("obs.streaming = {:?}", state.get("obs.streaming"));
    println!("obs.fps = {:?}", state.get("obs.fps"));
    // Give filter/hotkey round-trips a moment, then show them.
    tokio::time::sleep(Duration::from_millis(400)).await;
    println!("obs.hotkeys (count) = {:?}", state.get("obs.hotkeys").and_then(|v| v.as_array().map(|a| a.len())));
    println!("obs.filters = {:?}", state.get("obs.filters"));
    println!("obs.transitions = {:?}", state.get("obs.transitions"));
    assert!(scenes.as_array().is_some_and(|a| !a.is_empty()));
}

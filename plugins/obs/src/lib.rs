//! `obs` plugin — user-facing OBS **triggers** + reactive state.
//!
//! It subscribes to the certified `obs` module's events (scene / stream / record
//! / replay / virtual-cam / studio-mode) and turns each into a host trigger on
//! the bus, while mirroring the current OBS state into `obs.*` reactive variables
//! (usable in conditions and templates). OBS is event-driven, so triggers only
//! fire on real changes after connect (never at startup). Actions come next.

use quicktys_plugin_sdk::{
    json, ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, TriggerDecl, UiDecl, Value,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::{call as module_call, subscribe as module_subscribe};
use quicktys::api::reactivity::{emit_trigger, set_state};

/// Fields are labelled from the shared `plugin.obs.fields.*` keys.
fn label(key: &str) -> String {
    format!("plugin.obs.fields.{key}")
}
/// A templatable string field (`@var` / `{{ }}`) — for free text like source text.
fn field(key: &str) -> Schema {
    Schema::template_string().named(&label(key))
}
/// A string field autocompleted from a live OBS list (`obs-scene` / `obs-input`).
fn suggest_field(key: &str, source: &str) -> Schema {
    Schema::string().suggest(source).named(&label(key))
}
fn bool_field(key: &str) -> Schema {
    Schema::boolean().named(&label(key))
}
fn num_field(key: &str) -> Schema {
    Schema::number().named(&label(key))
}

fn set_var(key: &str, value: Value) {
    let json = quicktys_plugin_sdk::serde_json::to_string(&value).unwrap_or_else(|_| "null".into());
    set_state(key, &json);
}

/// Fire a trigger with no context.
fn fire(trigger: &str) {
    emit_trigger(trigger, "{}");
}

/// An on/off output event → mirror `obs.<state>` and fire the matching trigger.
fn output(payload: &Value, state_key: &str, on: &str, off: &str) {
    let active = payload.get("active").and_then(Value::as_bool).unwrap_or(false);
    set_var(state_key, Value::Bool(active));
    fire(if active { on } else { off });
}

/// Call an `obs` module service with JSON args.
fn call_service(service: &str, args: Value) -> Result<Value, ActionError> {
    let json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
    module_call("obs", service, &json)
        .map_err(|e| ActionError::failed(format!("obs {service} failed: {e:?}")))?;
    Ok(Value::Null)
}

/// Send a raw obs-websocket request (op 6) via the module's generic `request`.
fn obs_request(request_type: &str, data: Value) -> Result<Value, ActionError> {
    call_service("request", json!({ "type": request_type, "data": data }))
}

struct ObsPlugin;

impl Plugin for ObsPlugin {
    fn manifest() -> Manifest {
        Manifest::new("obs", "OBS Studio", "0.1.0")
            .icon("mdi mdi-video-wireless")
            .color("#302e3a")
            .requires("obs", "^0")
            // UI: an "Add to OBS" button in the webview editor toolbar. The slot
            // passes { url, name } of the current overlay into the action config.
            .ui(
                UiDecl::button("webview.toolbar", "add-browser-source")
                    .label("plugin.obs.ui.add-to-obs")
                    .icon("mdi mdi-television-play"),
            )
            // --- actions (obs-websocket requests) ---
            .action(ActionDecl::new("add-browser-source").config(Schema::object([
                ("url".to_string(), Schema::string()),
                ("name".to_string(), Schema::string()),
            ])))
            .action(ActionDecl::new("set-scene").config(Schema::object([
                ("scene".to_string(), suggest_field("scene", "obs-scene")),
                // Optional: apply this transition (+ duration ms) before switching.
                ("transition".to_string(), suggest_field("transition", "obs-transition")),
                ("duration".to_string(), num_field("duration")),
            ])))
            .action(ActionDecl::new("start-stream"))
            .action(ActionDecl::new("stop-stream"))
            .action(ActionDecl::new("toggle-stream"))
            .action(ActionDecl::new("start-record"))
            .action(ActionDecl::new("stop-record"))
            .action(ActionDecl::new("toggle-record"))
            .action(ActionDecl::new("pause-record"))
            .action(ActionDecl::new("resume-record"))
            .action(ActionDecl::new("start-replay"))
            .action(ActionDecl::new("stop-replay"))
            .action(ActionDecl::new("save-replay"))
            .action(ActionDecl::new("start-vcam"))
            .action(ActionDecl::new("stop-vcam"))
            .action(ActionDecl::new("toggle-vcam"))
            .action(
                ActionDecl::new("set-studio-mode")
                    .config(Schema::object([("enabled".to_string(), bool_field("enabled"))])),
            )
            .action(ActionDecl::new("studio-transition"))
            .action(ActionDecl::new("set-mute").config(Schema::object([
                ("input".to_string(), suggest_field("input", "obs-input")),
                ("muted".to_string(), bool_field("muted")),
            ])))
            .action(ActionDecl::new("toggle-mute").config(Schema::object([(
                "input".to_string(),
                suggest_field("input", "obs-input"),
            )])))
            .action(ActionDecl::new("set-text").config(Schema::object([
                ("input".to_string(), suggest_field("input", "obs-input")),
                ("text".to_string(), field("text")),
            ])))
            .action(ActionDecl::new("set-browser-url").config(Schema::object([
                ("input".to_string(), suggest_field("input", "obs-input")),
                ("url".to_string(), field("url")),
            ])))
            // scene-item actions (need a GetSceneItemId round-trip in the module)
            .action(ActionDecl::new("set-source-visibility").config(Schema::object([
                ("scene".to_string(), suggest_field("scene", "obs-scene")),
                ("source".to_string(), suggest_field("source", "obs-input")),
                ("visible".to_string(), bool_field("visible")),
            ])))
            .action(ActionDecl::new("set-source-transform").config(Schema::object([
                ("scene".to_string(), suggest_field("scene", "obs-scene")),
                ("source".to_string(), suggest_field("source", "obs-input")),
                ("pos-x".to_string(), num_field("pos-x")),
                ("pos-y".to_string(), num_field("pos-y")),
                ("scale-x".to_string(), num_field("scale-x")),
                ("scale-y".to_string(), num_field("scale-y")),
                ("rotation".to_string(), num_field("rotation")),
            ])))
            .action(ActionDecl::new("screenshot").config(Schema::object([
                ("source".to_string(), suggest_field("source", "obs-input")),
                ("path".to_string(), field("path")),
            ])))
            .action(ActionDecl::new("set-filter-visibility").config(Schema::object([
                ("source".to_string(), suggest_field("source", "obs-input")),
                ("filter".to_string(), suggest_field("filter", "obs-filter")),
                ("enabled".to_string(), bool_field("enabled")),
            ])))
            .action(ActionDecl::new("media-control").config(Schema::object([
                ("source".to_string(), suggest_field("source", "obs-input")),
                (
                    "action".to_string(),
                    Schema::string()
                        .named(&label("media-action"))
                        .options(["play", "pause", "stop", "restart", "next", "previous"]),
                ),
            ])))
            .action(ActionDecl::new("refresh-browser").config(Schema::object([(
                "source".to_string(),
                suggest_field("source", "obs-input"),
            )])))
            .action(ActionDecl::new("hotkey").config(Schema::object([(
                "name".to_string(),
                suggest_field("name", "obs-hotkey"),
            )])))
            // --- triggers (all of CastMate's OBS state changes) ---
            .trigger(
                TriggerDecl::new("scene-changed")
                    .context(Schema::object([("scene".to_string(), Schema::string())])),
            )
            .trigger(TriggerDecl::new("stream-started"))
            .trigger(TriggerDecl::new("stream-stopped"))
            .trigger(TriggerDecl::new("record-started"))
            .trigger(TriggerDecl::new("record-stopped"))
            .trigger(TriggerDecl::new("replay-started"))
            .trigger(TriggerDecl::new("replay-stopped"))
            .trigger(TriggerDecl::new("vcam-started"))
            .trigger(TriggerDecl::new("vcam-stopped"))
            .trigger(TriggerDecl::new("studio-mode-enabled"))
            .trigger(TriggerDecl::new("studio-mode-disabled"))
            .trigger(
                TriggerDecl::new("media-ended")
                    .context(Schema::object([("source".to_string(), Schema::string())])),
            )
            .trigger(
                TriggerDecl::new("media-started")
                    .context(Schema::object([("source".to_string(), Schema::string())])),
            )
            .trigger(
                TriggerDecl::new("replay-saved")
                    .context(Schema::object([("path".to_string(), Schema::string())])),
            )
            // --- reactive state (mirrors OBS; synced on connect + on change) ---
            .state_field(StateDecl::new("scene").schema(Schema::string()))
            .state_field(StateDecl::new("scenes").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("inputs").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("filters").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("hotkeys").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("transitions").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("transition").schema(Schema::string()))
            .state_field(StateDecl::new("streaming").schema(Schema::boolean()))
            .state_field(StateDecl::new("recording").schema(Schema::boolean()))
            .state_field(StateDecl::new("replay-buffering").schema(Schema::boolean()))
            .state_field(StateDecl::new("virtual-cam").schema(Schema::boolean()))
            .state_field(StateDecl::new("studio-mode").schema(Schema::boolean()))
            .state_field(StateDecl::new("stream-timecode").schema(Schema::string()))
            .state_field(StateDecl::new("stream-duration").schema(Schema::number()))
            .state_field(StateDecl::new("stream-congestion").schema(Schema::number()))
            .state_field(StateDecl::new("stream-skipped-frames").schema(Schema::number()))
            .state_field(StateDecl::new("record-timecode").schema(Schema::string()))
            .state_field(StateDecl::new("record-duration").schema(Schema::number()))
            .state_field(StateDecl::new("profile").schema(Schema::string()))
            .state_field(StateDecl::new("scene-collection").schema(Schema::string()))
            .state_field(StateDecl::new("canvas-width").schema(Schema::number()))
            .state_field(StateDecl::new("canvas-height").schema(Schema::number()))
            .state_field(StateDecl::new("output-width").schema(Schema::number()))
            .state_field(StateDecl::new("output-height").schema(Schema::number()))
            .state_field(StateDecl::new("fps").schema(Schema::number()))
            .state_field(StateDecl::new("cpu-usage").schema(Schema::number()))
            .state_field(StateDecl::new("memory-usage").schema(Schema::number()))
            .state_field(StateDecl::new("free-disk-space").schema(Schema::number()))
            .state_field(StateDecl::new("render-skipped-frames").schema(Schema::number()))
            .state_field(StateDecl::new("render-total-frames").schema(Schema::number()))
            .state_field(StateDecl::new("output-skipped-frames").schema(Schema::number()))
            .state_field(StateDecl::new("output-total-frames").schema(Schema::number()))
    }

    fn on_load() -> Result<(), String> {
        for event in [
            "scene-changed",
            "stream-state",
            "record-state",
            "replay-state",
            "vcam-state",
            "studio-state",
            "media-ended",
            "media-started",
            "replay-saved",
            "state",
        ] {
            module_subscribe("obs", event).map_err(|e| format!("subscribe {event} failed: {e:?}"))?;
        }
        Ok(())
    }

    fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
        if module_id != "obs" {
            return;
        }
        match event_id {
            "scene-changed" => {
                let scene = payload.get("scene").cloned().unwrap_or(Value::Null);
                set_var("scene", scene.clone());
                let ctx = quicktys_plugin_sdk::serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{}".into());
                emit_trigger("scene-changed", &ctx);
            }
            "stream-state" => output(&payload, "streaming", "stream-started", "stream-stopped"),
            "record-state" => output(&payload, "recording", "record-started", "record-stopped"),
            "replay-state" => output(&payload, "replay-buffering", "replay-started", "replay-stopped"),
            "vcam-state" => output(&payload, "virtual-cam", "vcam-started", "vcam-stopped"),
            "studio-state" => {
                let enabled = payload.get("enabled").and_then(Value::as_bool).unwrap_or(false);
                set_var("studio-mode", Value::Bool(enabled));
                fire(if enabled { "studio-mode-enabled" } else { "studio-mode-disabled" });
            }
            // Media / replay events carry a context → forward as-is.
            "media-ended" | "media-started" | "replay-saved" => {
                let ctx = quicktys_plugin_sdk::serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{}".into());
                emit_trigger(event_id, &ctx);
            }
            // Bulk sync (from op-7 responses): mirror every present key to `obs.*`.
            // No triggers — this reflects OBS's current state, not a change.
            "state" => {
                if let Some(obj) = payload.as_object() {
                    for (key, value) in obj {
                        if !value.is_null() {
                            set_var(key, value.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let s = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let b = |k: &str| config.get(k).and_then(Value::as_bool).unwrap_or(false);
        let n = |k: &str| config.get(k).and_then(Value::as_f64);

        // Scene change: optionally set the transition (+ duration) first, since
        // OBS applies the active transition on the next scene switch.
        if action == "set-scene" {
            let transition = s("transition");
            if !transition.is_empty() {
                obs_request("SetCurrentSceneTransition", json!({ "transitionName": transition }))?;
            }
            if let Some(d) = n("duration") {
                obs_request("SetCurrentSceneTransitionDuration", json!({ "transitionDuration": d }))?;
            }
            return obs_request("SetCurrentProgramScene", json!({ "sceneName": s("scene") }));
        }

        // Scene-item actions go through dedicated module services (they need a
        // GetSceneItemId round-trip in the module).
        match action {
            "set-source-visibility" => {
                return call_service(
                    "set-source-enabled",
                    json!({ "scene": s("scene"), "source": s("source"), "enabled": b("visible") }),
                );
            }
            "add-browser-source" => {
                return call_service(
                    "add-browser-source",
                    json!({ "url": s("url"), "name": s("name") }),
                );
            }
            "set-source-transform" => {
                let mut t = quicktys_plugin_sdk::serde_json::Map::new();
                for (cfg, obs) in [
                    ("pos-x", "positionX"),
                    ("pos-y", "positionY"),
                    ("scale-x", "scaleX"),
                    ("scale-y", "scaleY"),
                    ("rotation", "rotation"),
                ] {
                    if let Some(v) = n(cfg) {
                        t.insert(obs.to_string(), json!(v));
                    }
                }
                return call_service(
                    "set-source-transform",
                    json!({ "scene": s("scene"), "source": s("source"), "transform": Value::Object(t) }),
                );
            }
            _ => {}
        }

        // Map the action to an obs-websocket (requestType, requestData).
        let (rtype, data): (&str, Value) = match action {
            "start-stream" => ("StartStream", json!({})),
            "stop-stream" => ("StopStream", json!({})),
            "toggle-stream" => ("ToggleStream", json!({})),
            "start-record" => ("StartRecord", json!({})),
            "stop-record" => ("StopRecord", json!({})),
            "toggle-record" => ("ToggleRecord", json!({})),
            "pause-record" => ("PauseRecord", json!({})),
            "resume-record" => ("ResumeRecord", json!({})),
            "start-replay" => ("StartReplayBuffer", json!({})),
            "stop-replay" => ("StopReplayBuffer", json!({})),
            "save-replay" => ("SaveReplayBuffer", json!({})),
            "start-vcam" => ("StartVirtualCam", json!({})),
            "stop-vcam" => ("StopVirtualCam", json!({})),
            "toggle-vcam" => ("ToggleVirtualCam", json!({})),
            "set-studio-mode" => ("SetStudioModeEnabled", json!({ "studioModeEnabled": b("enabled") })),
            "studio-transition" => ("TriggerStudioModeTransition", json!({})),
            "set-mute" => ("SetInputMute", json!({ "inputName": s("input"), "inputMuted": b("muted") })),
            "toggle-mute" => ("ToggleInputMute", json!({ "inputName": s("input") })),
            "set-text" => (
                "SetInputSettings",
                json!({ "inputName": s("input"), "inputSettings": { "text": s("text") }, "overlay": true }),
            ),
            "set-browser-url" => (
                "SetInputSettings",
                json!({ "inputName": s("input"), "inputSettings": { "url": s("url") }, "overlay": true }),
            ),
            "screenshot" => (
                "SaveSourceScreenshot",
                json!({ "sourceName": s("source"), "imageFilePath": s("path"), "imageFormat": "png" }),
            ),
            "set-filter-visibility" => (
                "SetSourceFilterEnabled",
                json!({ "sourceName": s("source"), "filterName": s("filter"), "filterEnabled": b("enabled") }),
            ),
            "media-control" => (
                "TriggerMediaInputAction",
                json!({
                    "inputName": s("source"),
                    "mediaAction": format!("OBS_WEBSOCKET_MEDIA_INPUT_ACTION_{}", s("action").to_uppercase()),
                }),
            ),
            "refresh-browser" => (
                "PressInputPropertiesButton",
                json!({ "inputName": s("source"), "propertyName": "refreshnocache" }),
            ),
            "hotkey" => ("TriggerHotkeyByName", json!({ "hotkeyName": s("name") })),
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };

        let args = json!({ "type": rtype, "data": data });
        let args_json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        module_call("obs", "request", &args_json)
            .map_err(|e| ActionError::failed(format!("obs.{rtype} failed: {e:?}")))?;
        Ok(Value::Null)
    }
}

quicktys_plugin_sdk::define_plugin!(ObsPlugin);

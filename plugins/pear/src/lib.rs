//! `pear` plugin — Pear Desktop playback controls.
//!
//! Provides user-facing actions (Play, Pause, Next, …) that reach Pear Desktop
//! through the certified `pear` module's services. The plugin itself is
//! sandboxed (no network); it declares `requires: pear` and calls the module
//! via the host `modules` interface.

use quicktys_plugin_sdk::{
    json, ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, TriggerDecl, Value,
};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::{call as module_call, subscribe as module_subscribe};
use quicktys::api::reactivity::{emit_trigger, set_state};

/// Set a reactive state variable (`pear.<key>`) to a JSON value.
fn set_var(key: &str, value: &Value) {
    let json = quicktys_plugin_sdk::serde_json::to_string(value).unwrap_or_else(|_| "null".into());
    set_state(key, &json);
}

/// Split an "Artist A, Artist B & C" string into a list of names.
fn split_artists(artist: &Value) -> Value {
    match artist.as_str() {
        Some(s) => Value::Array(
            s.split([',', '&'])
                .map(str::trim)
                .filter(|a| !a.is_empty())
                .map(|a| Value::String(a.to_string()))
                .collect(),
        ),
        None => Value::Array(Vec::new()),
    }
}

/// Mirror the fields of a song payload into `pear.*` reactive state variables.
/// Every field is updated **only when present**, so a partial payload (e.g. a
/// WebSocket `PLAYER_STATE_CHANGED` carrying just `isPaused`) updates only what
/// it carries and never wipes the rest.
fn set_song_state(song: &Value) {
    if let Some(v) = song.get("title") {
        set_var("title", v);
    }
    if let Some(v) = song.get("artist") {
        set_var("artist", v);
        set_var("artists", &split_artists(v));
    }
    if let Some(v) = song.get("imageSrc") {
        set_var("cover", v);
    }
    if let Some(v) = song.get("url") {
        set_var("url", v);
    }
    if let Some(v) = song.get("album") {
        set_var("album", v);
    }
    if let Some(v) = song.get("videoId") {
        set_var("video-id", v);
    }
    if let Some(paused) = song.get("isPaused").and_then(Value::as_bool) {
        set_var("playing", &Value::Bool(!paused));
    }
    // Playback time (seconds) — drives the widget's progress bar.
    if let Some(d) = song.get("songDuration").and_then(Value::as_f64) {
        set_var("duration", &json!(d));
    }
    if let Some(p) = song.get("elapsedSeconds").and_then(Value::as_f64) {
        set_var("position", &json!(p));
    }
    if let Some(playlist) = song.get("playlist") {
        set_var("playlist", playlist);
    }
    if let Some(queue) = song.get("queue") {
        set_var("queue", queue);
    }
    if let Some(lyrics) = song.get("lyrics").and_then(Value::as_str) {
        set_var("lyrics", &Value::String(lyrics.to_string()));
    }
}

struct PearPlugin;

impl Plugin for PearPlugin {
    fn manifest() -> Manifest {
        Manifest::new("pear", "Pear Desktop", "0.1.0")
            .icon("mdi mdi-music")
            .color("#7ec27e")
            .requires("pear", "^0")
            .action(ActionDecl::new("play"))
            .action(ActionDecl::new("pause"))
            .action(ActionDecl::new("toggle"))
            .action(ActionDecl::new("next"))
            .action(ActionDecl::new("previous"))
            .action(ActionDecl::new("like"))
            .action(ActionDecl::new("dislike"))
            .action(ActionDecl::new("toggle-mute"))
            .action(ActionDecl::new("shuffle"))
            .action(
                ActionDecl::new("set-volume")
                    .config(Schema::object([("volume".to_string(), volume_schema())])),
            )
            .action(
                ActionDecl::new("seek-to")
                    .config(Schema::object([("seconds".to_string(), Schema::number())])),
            )
            .action(
                ActionDecl::new("set-queue-index")
                    .config(Schema::object([("index".to_string(), Schema::number())])),
            )
            // Fires when the playing song changes (bridged from the module's
            // `song-changed` event). Context = the new song.
            .trigger(
                TriggerDecl::new("song-changed").context(Schema::object([
                    ("title".to_string(), Schema::string()),
                    ("artist".to_string(), Schema::string()),
                    ("videoId".to_string(), Schema::string()),
                ])),
            )
            // Reactive variables exposed to templates (updated on song change).
            .state_field(StateDecl::new("title").schema(Schema::string()))
            .state_field(StateDecl::new("artist").schema(Schema::string()))
            .state_field(StateDecl::new("artists").schema(Schema::array(Schema::string())))
            .state_field(StateDecl::new("cover").schema(Schema::string()))
            .state_field(StateDecl::new("url").schema(Schema::string()))
            .state_field(StateDecl::new("album").schema(Schema::string()))
            .state_field(StateDecl::new("video-id").schema(Schema::string()))
            .state_field(StateDecl::new("playing").schema(Schema::boolean()))
            .state_field(StateDecl::new("duration").schema(Schema::number()))
            .state_field(StateDecl::new("position").schema(Schema::number()))
            .state_field(StateDecl::new("lyrics").schema(Schema::string()))
            .state_field(StateDecl::new("playlist").schema(Schema::array(Schema::string())))
            .state_field(
                StateDecl::new("queue").schema(Schema::array(Schema::object([
                    ("title".to_string(), Schema::string()),
                    ("artist".to_string(), Schema::string()),
                    ("thumbnail".to_string(), Schema::string()),
                    ("videoId".to_string(), Schema::string()),
                ]))),
            )
    }

    fn on_load() -> Result<(), String> {
        // `song` keeps the reactive variables current (incl. at startup);
        // `song-changed` fires only on a real change → the trigger.
        module_subscribe("pear", "song").map_err(|e| format!("subscribe failed: {e:?}"))?;
        module_subscribe("pear", "song-changed").map_err(|e| format!("subscribe failed: {e:?}"))?;
        Ok(())
    }

    fn on_module_event(module_id: &str, event_id: &str, payload: Value) {
        if module_id != "pear" {
            return;
        }
        match event_id {
            // Mirror the song's fields into reactive variables (pear.title, …).
            "song" => set_song_state(&payload),
            // A real change → fire the trigger, carrying the song as context.
            "song-changed" => {
                let ctx = quicktys_plugin_sdk::serde_json::to_string(&payload)
                    .unwrap_or_else(|_| "{}".into());
                emit_trigger("song-changed", &ctx);
            }
            _ => {}
        }
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        // Map the action to a module service call (method + JSON args).
        let (method, args) = match action {
            "play" | "pause" | "toggle" | "next" | "previous" | "like" | "dislike"
            | "toggle-mute" | "shuffle" => (action, Value::Null),
            "set-volume" => (
                "set-volume",
                json!({ "volume": config.get("volume").cloned().unwrap_or(json!(50)) }),
            ),
            "seek-to" => (
                "seek-to",
                json!({ "seconds": config.get("seconds").cloned().unwrap_or(json!(0)) }),
            ),
            "set-queue-index" => (
                "set-queue-index",
                json!({ "index": config.get("index").cloned().unwrap_or(json!(0)) }),
            ),
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };

        let args_json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        let result = module_call("pear", method, &args_json)
            .map_err(|e| ActionError::failed(format!("pear.{method} failed: {e:?}")))?;
        Ok(quicktys_plugin_sdk::serde_json::from_str(&result).unwrap_or(Value::Null))
    }
}

fn volume_schema() -> Schema {
    Schema::Number {
        common: Default::default(),
        default: Some(50.0),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        slider: true,
        unit: None,
        template: true,
    }
}

quicktys_plugin_sdk::define_plugin!(PearPlugin);

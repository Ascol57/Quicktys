//! `sound` plugin — "Play Sound" action. Sandboxed (no audio access itself); it
//! routes to the certified `sound` module, which plays the file host-side.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::modules::call as module_call;

struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn manifest() -> Manifest {
        Manifest::new("sound", "Sound", "0.1.0")
            .icon("mdi mdi-volume-high")
            .color("#e0a458")
            .requires("sound", "^0")
            .action(ActionDecl::new("play").config(Schema::object([
                ("file".to_string(), Schema::template_string().named("plugin.sound.fields.file")),
                ("volume".to_string(), Schema::number().named("plugin.sound.fields.volume")),
            ])))
            // Text-to-speech through the OS voice.
            .action(ActionDecl::new("tts").config(Schema::object([
                ("text".to_string(), Schema::template_string().named("plugin.sound.fields.text")),
                (
                    "voice".to_string(),
                    Schema::string().suggest("tts-voice").named("plugin.sound.fields.voice"),
                ),
            ])))
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let s = |k: &str| config.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
        let (service, args) = match action {
            "play" => {
                let file = s("file");
                if file.is_empty() {
                    return Err(ActionError::failed("sound file is required".to_string()));
                }
                // Volume entered as a percent (0–100, default 100) → 0.0–1.0 gain.
                let percent = config.get("volume").and_then(Value::as_f64).unwrap_or(100.0);
                ("play", json!({ "path": file, "volume": (percent / 100.0).clamp(0.0, 4.0) }))
            }
            "tts" => {
                let text = s("text");
                if text.is_empty() {
                    return Err(ActionError::failed("text is required".to_string()));
                }
                ("speak", json!({ "text": text, "voice": s("voice") }))
            }
            other => return Err(ActionError::failed(format!("unknown action: {other}"))),
        };
        let json = quicktys_plugin_sdk::serde_json::to_string(&args).unwrap_or_else(|_| "null".into());
        module_call("sound", service, &json)
            .map_err(|e| ActionError::failed(format!("sound.{service} failed: {e:?}")))?;
        Ok(Value::Null)
    }
}

quicktys_plugin_sdk::define_plugin!(SoundPlugin);

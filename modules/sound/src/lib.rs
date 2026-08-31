//! `sound` module — certified driver for local audio playback. Holding the
//! `audio` system capability, it plays a sound file (wav/mp3/ogg/flac) through
//! the host, decoded and mixed host-side. The `sound` plugin exposes this as a
//! user-facing "Play Sound" action.

use quicktys_module_sdk::{Capability, Module, ModuleManifest, Schema, ServiceDecl, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "module",
});

use quicktys::api::audio::{list_voices, play_file, speak};

struct Sound;

impl Module for Sound {
    fn manifest() -> ModuleManifest {
        ModuleManifest::new("sound", "Sound", "0.1.0")
            .capability(Capability::Audio)
            // play { path, volume } — volume is a 0.0–1.0 gain (1.0 = 100%).
            .service(ServiceDecl::new("play").args(Schema::object([
                ("path".to_string(), Schema::string()),
                ("volume".to_string(), Schema::number()),
            ])))
            .service(ServiceDecl::new("speak").args(Schema::object([
                ("text".to_string(), Schema::string()),
                ("voice".to_string(), Schema::string()),
            ])))
            .service(ServiceDecl::new("voices"))
    }

    fn call(method: &str, args: Value) -> Result<Value, String> {
        match method {
            "play" => {
                let path = args.get("path").and_then(Value::as_str).unwrap_or_default();
                if path.is_empty() {
                    return Err("sound path is required".to_string());
                }
                let volume = args.get("volume").and_then(Value::as_f64).unwrap_or(1.0) as f32;
                play_file(path, volume)?;
                Ok(Value::Null)
            }
            "speak" => {
                let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
                if text.is_empty() {
                    return Err("text is required".to_string());
                }
                let voice = args.get("voice").and_then(Value::as_str).unwrap_or_default();
                speak(text, voice)?;
                Ok(Value::Null)
            }
            "voices" => Ok(Value::Array(
                list_voices().into_iter().map(Value::String).collect(),
            )),
            other => Err(format!("unknown method: {other}")),
        }
    }
}

quicktys_module_sdk::define_module!(Sound);

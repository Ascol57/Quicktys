//! Real audio playback backing the `audio` capability (the `sound` module).
//!
//! `rodio`'s `OutputStream` is `!Send`, so a single dedicated thread owns it and
//! receives `(path, volume)` play commands over a channel. Playback is
//! fire-and-forget (each sound gets a detached sink, so several can overlap). If
//! no output device is available (headless/CI), commands are drained as no-ops.

use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;

/// The lazily-spawned audio thread's command sender.
fn player() -> &'static Sender<(String, f32)> {
    static TX: OnceLock<Sender<(String, f32)>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = channel::<(String, f32)>();
        std::thread::Builder::new()
            .name("quicktys-audio".into())
            .spawn(move || {
                // Open the default device on this thread (the stream is !Send).
                let stream = match rodio::OutputStream::try_default() {
                    Ok(s) => Some(s),
                    Err(e) => {
                        tracing::warn!("audio: no output device ({e}); sounds will be silent");
                        None
                    }
                };
                for (path, volume) in rx {
                    if let Some((_stream, handle)) = &stream {
                        if let Err(e) = play_one(handle, &path, volume) {
                            tracing::warn!("audio: play {path}: {e}");
                        }
                    }
                }
            })
            .expect("spawn audio thread");
        tx
    })
}

/// Decode and play a file on the shared output stream, detached (plays to end).
fn play_one(handle: &rodio::OutputStreamHandle, path: &str, volume: f32) -> Result<(), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let source = rodio::Decoder::new(std::io::BufReader::new(file)).map_err(|e| format!("decode: {e}"))?;
    let sink = rodio::Sink::try_new(handle).map_err(|e| format!("sink: {e}"))?;
    sink.set_volume(volume);
    sink.append(source);
    sink.detach();
    Ok(())
}

/// Queue an audio file for playback at `volume` (1.0 = 100%). Non-blocking.
pub fn play_file(path: &str, volume: f32) -> Result<(), String> {
    let volume = if volume.is_finite() && volume > 0.0 { volume } else { 1.0 };
    player()
        .send((path.to_string(), volume))
        .map_err(|e| format!("audio unavailable: {e}"))
}

/// Text-to-speech through the OS voice (non-blocking). `voice` is a platform
/// voice name (macOS/Windows) or language code (Linux); empty = OS default.
/// Uses `say` on macOS, SAPI on Windows, `spd-say` (speech-dispatcher) on Linux —
/// no build dependency, just the platform tool.
pub fn speak(text: &str, voice: &str) -> Result<(), String> {
    use std::process::Command;
    let voice = voice.trim();
    let spawn = if cfg!(target_os = "macos") {
        let mut cmd = Command::new("say");
        if !voice.is_empty() {
            cmd.args(["-v", voice]);
        }
        cmd.arg(text).spawn()
    } else if cfg!(target_os = "windows") {
        let select = if voice.is_empty() {
            String::new()
        } else {
            format!("$s.SelectVoice('{}');", voice.replace('\'', "''"))
        };
        let script = format!(
            "Add-Type -AssemblyName System.Speech; \
             $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; {select} $s.Speak('{}')",
            text.replace('\'', "''")
        );
        Command::new("powershell").args(["-NoProfile", "-Command", &script]).spawn()
    } else {
        // speech-dispatcher; `-l` sets the language. `-w` waits so calls queue.
        let mut cmd = Command::new("spd-say");
        cmd.arg("-w");
        if !voice.is_empty() {
            cmd.args(["-l", voice]);
        }
        cmd.arg(text).spawn()
    };
    spawn.map(|_| ()).map_err(|e| format!("tts: {e}"))
}

/// The installed TTS voices / languages, best-effort per platform.
pub fn list_voices() -> Vec<String> {
    use std::process::Command;
    if cfg!(target_os = "macos") {
        // `say -v ?` → lines like "Thomas   fr_FR   # Bonjour…"; take the name.
        let out = Command::new("say").arg("-v").arg("?").output();
        return out
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|l| l.split_whitespace().next().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
    }
    if cfg!(target_os = "windows") {
        let script = "Add-Type -AssemblyName System.Speech; \
             (New-Object System.Speech.Synthesis.SpeechSynthesizer).GetInstalledVoices() | \
             ForEach-Object { $_.VoiceInfo.Name }";
        let out = Command::new("powershell").args(["-NoProfile", "-Command", script]).output();
        return out
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
    }
    // Linux (speech-dispatcher): common ISO language codes (spd-say `-l`).
    ["en", "fr", "de", "es", "it", "pt", "nl", "ru", "pl", "sv", "ja", "zh"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

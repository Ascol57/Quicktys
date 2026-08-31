//! The `input` capability: synthesize keyboard / mouse input cross-platform via
//! `enigo` (X11/Wayland on Linux, SendInput on Windows, CGEvent on macOS).
//!
//! A fresh `Enigo` is created per call (it is not `Send`). Needs a desktop
//! session; on a headless host `Enigo::new` fails and the call returns an error.
//!
//! Every interaction runs on a dedicated OS thread via [`on_input_thread`]:
//! enigo's libei/Wayland backend drives the desktop portal through `ashpd`,
//! which blocks on its own async runtime internally. Calling that from a Tokio
//! worker (the host invokes capabilities inside the async runtime) panics with
//! "Cannot start a runtime from within a runtime". A plain `std::thread` has no
//! ambient runtime, so the nested `block_on` is legal there.

use enigo::{Button, Coordinate, Direction, Enigo, Key, Keyboard, Mouse, Settings};

fn make() -> Result<Enigo, String> {
    Enigo::new(&Settings::default()).map_err(|e| format!("input backend: {e}"))
}

/// Run an enigo interaction on a fresh OS thread with no ambient Tokio runtime.
fn on_input_thread<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    std::thread::spawn(f)
        .join()
        .map_err(|_| "input thread panicked".to_string())?
}

/// Type a string of text.
pub fn type_text(text: &str) -> Result<(), String> {
    let text = text.to_string();
    on_input_thread(move || make()?.text(&text).map_err(|e| format!("type: {e}")))
}

/// Press a chord: all but the last token are modifiers held around the last key.
pub fn key_combo(keys: &[String]) -> Result<(), String> {
    if keys.is_empty() {
        return Ok(());
    }
    let parsed: Vec<Key> = keys
        .iter()
        .map(|k| parse_key(k).ok_or_else(|| format!("unknown key: {k}")))
        .collect::<Result<_, _>>()?;
    on_input_thread(move || {
        let mut enigo = make()?;
        let (main, mods) = parsed.split_last().unwrap();
        for m in mods {
            enigo.key(*m, Direction::Press).map_err(|e| format!("key: {e}"))?;
        }
        let res = enigo.key(*main, Direction::Click).map_err(|e| format!("key: {e}"));
        // Always release modifiers, even if the main key failed.
        for m in mods.iter().rev() {
            let _ = enigo.key(*m, Direction::Release);
        }
        res
    })
}

/// Click a mouse button (0=left, 1=middle, 2=right).
pub fn mouse_click(button: u8) -> Result<(), String> {
    on_input_thread(move || {
        make()?
            .button(mouse_button(button), Direction::Click)
            .map_err(|e| format!("click: {e}"))
    })
}

/// Move the mouse to absolute screen coordinates.
pub fn mouse_move(x: i32, y: i32) -> Result<(), String> {
    on_input_thread(move || {
        make()?
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| format!("move: {e}"))
    })
}

/// Press/release a mouse button.
pub fn mouse_button(_button: u8) -> Button {
    match _button {
        1 => Button::Middle,
        2 => Button::Right,
        _ => Button::Left,
    }
}

fn parse_key(token: &str) -> Option<Key> {
    let t = token.trim().to_ascii_lowercase();
    Some(match t.as_str() {
        "ctrl" | "control" => Key::Control,
        "shift" => Key::Shift,
        "alt" | "option" => Key::Alt,
        "meta" | "super" | "win" | "windows" | "cmd" | "command" => Key::Meta,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "esc" | "escape" => Key::Escape,
        "space" | "spacebar" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        f if f.starts_with('f') && f[1..].parse::<u8>().is_ok() => {
            let n: u8 = f[1..].parse().ok()?;
            match n {
                1 => Key::F1, 2 => Key::F2, 3 => Key::F3, 4 => Key::F4,
                5 => Key::F5, 6 => Key::F6, 7 => Key::F7, 8 => Key::F8,
                9 => Key::F9, 10 => Key::F10, 11 => Key::F11, 12 => Key::F12,
                _ => return None,
            }
        }
        other => {
            let mut chars = other.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None; // multi-char, not a named key
            }
            Key::Unicode(c)
        }
    })
}

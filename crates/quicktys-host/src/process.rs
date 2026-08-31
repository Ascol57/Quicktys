//! The `process` capability: launch programs / shell commands / open files, and
//! kill spawned children. Cross-platform via `cfg!` (Windows `cmd`/`start`,
//! else `sh`/`open`/`xdg-open`).

use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

/// Spawned children by handle, so `kill` can reach them.
fn children() -> &'static Mutex<HashMap<u64, Child>> {
    static C: OnceLock<Mutex<HashMap<u64, Child>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}
fn next_handle() -> u64 {
    static N: AtomicU64 = AtomicU64::new(1);
    N.fetch_add(1, Ordering::Relaxed)
}

fn track(child: Child) -> u64 {
    let h = next_handle();
    children().lock().unwrap().insert(h, child);
    h
}

/// Launch a program with args.
pub fn spawn(program: &str, args: &[String]) -> Result<u64, String> {
    let child = Command::new(program)
        .args(args)
        .spawn()
        .map_err(|e| format!("spawn {program}: {e}"))?;
    Ok(track(child))
}

/// Run a command line through the OS shell.
pub fn run_shell(command: &str) -> Result<u64, String> {
    let child = if cfg!(windows) {
        Command::new("cmd").args(["/C", command]).spawn()
    } else {
        Command::new("sh").args(["-c", command]).spawn()
    }
    .map_err(|e| format!("shell: {e}"))?;
    Ok(track(child))
}

/// Open a file / URL with the OS default handler.
pub fn open_path(target: &str) -> Result<(), String> {
    let result = if cfg!(target_os = "windows") {
        // `start` is a cmd builtin; the empty "" is the window title arg.
        Command::new("cmd").args(["/C", "start", "", target]).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(target).spawn()
    } else {
        Command::new("xdg-open").arg(target).spawn()
    };
    result.map(|_| ()).map_err(|e| format!("open {target}: {e}"))
}

/// Kill a tracked child (best-effort).
pub fn kill(handle: u64) {
    if let Some(mut child) = children().lock().unwrap().remove(&handle) {
        let _ = child.kill();
    }
}

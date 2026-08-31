//! The `hid` capability (M7): enumerate / open / read / write USB-HID devices —
//! e.g. an Elgato Stream Deck as a hardware button surface. A module `open`s a
//! device with an `event-id`; a reader thread pushes each input report to the
//! module via `on-host-event`, mirroring the `net.ws-connect` pattern. Only a
//! **certified** module reaching this cap (gated at dispatch).
//!
//! Linux uses the pure-Rust `linux-native` hidraw backend (no system libs).

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use hidapi::{HidApi, HidDevice};

/// The shared HID context (enumerating twice warns; keep exactly one).
fn api() -> std::sync::MutexGuard<'static, HidApi> {
    static API: OnceLock<Mutex<HidApi>> = OnceLock::new();
    API.get_or_init(|| Mutex::new(HidApi::new().expect("hidapi init")))
        .lock()
        .unwrap()
}

/// Open-handle state, so `write`/`close` can reach a reader-thread device.
struct Handle {
    path: String,
    stop: Arc<AtomicBool>,
}
fn handles() -> &'static Mutex<HashMap<u64, Handle>> {
    static H: OnceLock<Mutex<HashMap<u64, Handle>>> = OnceLock::new();
    H.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Device paths matching an optional vendor/product filter.
pub fn enumerate(vendor: Option<u16>, product: Option<u16>) -> Vec<String> {
    let mut api = api();
    let _ = api.refresh_devices();
    api.device_list()
        .filter(|d| vendor.is_none_or(|v| d.vendor_id() == v))
        .filter(|d| product.is_none_or(|p| d.product_id() == p))
        .filter_map(|d| d.path().to_str().ok().map(str::to_string))
        .collect()
}

/// Open a device by path (for the reader thread to own).
pub fn open_device(path: &str) -> Result<HidDevice, String> {
    let cpath = std::ffi::CString::new(path).map_err(|e| e.to_string())?;
    api().open_path(&cpath).map_err(|e| e.to_string())
}

/// Record an open handle so `write`/`close` can find it.
pub fn register(handle: u64, path: String, stop: Arc<AtomicBool>) {
    handles().lock().unwrap().insert(handle, Handle { path, stop });
}

/// Write an output report to an open handle (re-opens for the write side, so it
/// doesn't contend with the blocking reader).
pub fn write(handle: u64, data: &[u8]) -> Result<(), String> {
    let path = handles()
        .lock()
        .unwrap()
        .get(&handle)
        .map(|h| h.path.clone())
        .ok_or("unknown hid handle")?;
    open_device(&path)?.write(data).map(|_| ()).map_err(|e| e.to_string())
}

/// Stop the reader thread and forget the handle.
pub fn close(handle: u64) {
    if let Some(h) = handles().lock().unwrap().remove(&handle) {
        h.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

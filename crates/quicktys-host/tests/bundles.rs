//! Bundle loading: extensions are shipped as self-contained *bundle* directories
//! (`<id>.wasm` + `lang/` + `bundle.json`) produced by `scripts/build-bundles.sh`.
//! This proves the host loads a real bundle from disk via its descriptor — the
//! same path the desktop app takes — rather than a hardcoded dev artifact path.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

#[derive(Deserialize)]
struct BundleManifest {
    id: String,
    kind: String,
    wasm: String,
    #[serde(default)]
    certified: bool,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Ensure `dist/bundles` exists by running the bundling script on demand.
fn ensure_bundles() -> PathBuf {
    let dist = repo_root().join("dist/bundles");
    if !dist.join("modules/pear/bundle.json").exists() {
        let status = Command::new("bash")
            .arg(repo_root().join("scripts/build-bundles.sh"))
            .current_dir(repo_root())
            .status()
            .expect("run build-bundles.sh");
        assert!(status.success(), "build-bundles.sh failed");
    }
    dist
}

fn read_manifest(dir: &std::path::Path) -> BundleManifest {
    let text = std::fs::read_to_string(dir.join("bundle.json")).expect("read bundle.json");
    serde_json::from_str(&text).expect("parse bundle.json")
}

#[test]
fn loads_extensions_from_their_bundles() {
    let dist = ensure_bundles();
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();

    // Module bundle: descriptor drives cert + wasm entry.
    let mod_dir = dist.join("modules/pear");
    let mm = read_manifest(&mod_dir);
    assert_eq!(mm.kind, "module");
    assert!(mm.certified, "pear module bundle is certified");
    let module = host
        .load_module(&mod_dir.join(&mm.wasm), mm.certified)
        .expect("load module bundle");
    assert_eq!(module.id, mm.id);
    assert!(host.has_module("pear"));

    // Plugin bundle (depends on the module) + it ships its own locales.
    let plug_dir = dist.join("plugins/pear");
    let pm = read_manifest(&plug_dir);
    assert_eq!(pm.kind, "plugin");
    assert!(plug_dir.join("lang/en.json").exists(), "bundle carries lang/");
    let plugin = host
        .load_plugin(&plug_dir.join(&pm.wasm))
        .expect("load plugin bundle");
    assert_eq!(plugin.id, pm.id);
    assert!(host.has_plugin("pear"));
}

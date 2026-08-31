//! The `random` plugin (pure, safe caps only): each action returns a value we
//! can bind. Randomness makes exact values unpredictable, so we assert the
//! *shape* and *bounds* of results over several draws.

use quicktys_core::{StateStore, TriggerBus};
use quicktys_host::WasmHost;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

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
async fn random_actions_stay_within_bounds() {
    let host = WasmHost::new(Arc::new(StateStore::new()), Arc::new(TriggerBus::new())).unwrap();
    host.load_plugin(&build("../../plugins/random", "random"))
        .expect("load random plugin");
    let invoker = host.invoker();

    // Draw enough times that an off-by-one bound bug would surface.
    for _ in 0..64 {
        // integer number in [1, 6]
        let n = invoker
            .invoke("random", "number", json!({ "min": 1, "max": 6, "integer": true }))
            .await
            .expect("number");
        let n = n.as_i64().expect("integer");
        assert!((1..=6).contains(&n), "number out of range: {n}");

        // float number in [0, 1)
        let f = invoker
            .invoke("random", "number", json!({ "min": 0, "max": 1 }))
            .await
            .expect("float");
        let f = f.as_f64().expect("float");
        assert!((0.0..=1.0).contains(&f), "float out of range: {f}");

        // 3d6 → total in [3, 18], three rolls each in [1, 6]
        let d = invoker
            .invoke("random", "dice", json!({ "count": 3, "sides": 6 }))
            .await
            .expect("dice");
        let total = d.get("total").and_then(|v| v.as_i64()).expect("total");
        let rolls = d.get("rolls").and_then(|v| v.as_array()).expect("rolls");
        assert_eq!(rolls.len(), 3);
        assert!((3..=18).contains(&total), "3d6 total out of range: {total}");
        for r in rolls {
            let r = r.as_i64().unwrap();
            assert!((1..=6).contains(&r), "die out of range: {r}");
        }

        // coin → heads | tails
        let c = invoker.invoke("random", "coin", json!({})).await.expect("coin");
        assert!(matches!(c.as_str(), Some("heads") | Some("tails")));

        // pick → one of the items
        let p = invoker
            .invoke("random", "pick", json!({ "items": ["a", "b", "c"] }))
            .await
            .expect("pick");
        assert!(matches!(p.as_str(), Some("a") | Some("b") | Some("c")));
    }

    // chance(0) is never true, chance(100) is always true.
    let never = invoker
        .invoke("random", "chance", json!({ "percent": 0 }))
        .await
        .expect("chance 0");
    assert_eq!(never, json!(false));
    let always = invoker
        .invoke("random", "chance", json!({ "percent": 100 }))
        .await
        .expect("chance 100");
    assert_eq!(always, json!(true));
}

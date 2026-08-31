//! `random` plugin — randomness for automations: a number in a range, a coin
//! flip, dice, a percentage chance, or a random pick from a list.
//!
//! Pure: it declares no module and only safe capabilities. Each action returns
//! its result (bind it with `bind:` to a local, or read it downstream). Entropy
//! comes from the host runtime's `wasi:random` via `getrandom` — no seed state.

use quicktys_plugin_sdk::{json, ActionDecl, ActionError, Manifest, Plugin, Schema, Value};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

struct RandomPlugin;

/// A fresh 64-bit draw from the host RNG.
fn rand_u64() -> u64 {
    let mut buf = [0u8; 8];
    // getrandom never fails on wasip2 (backed by wasi:random); fall back to 0.
    let _ = getrandom::getrandom(&mut buf);
    u64::from_le_bytes(buf)
}

/// A uniform float in `[0, 1)`.
fn rand_unit() -> f64 {
    // 53 bits of mantissa precision — the standard trick.
    (rand_u64() >> 11) as f64 / (1u64 << 53) as f64
}

/// A uniform integer in `[min, max]` (inclusive). `min > max` is swapped.
fn rand_int(min: i64, max: i64) -> i64 {
    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
    let span = (hi - lo) as u128 + 1;
    lo + (rand_u64() as u128 % span) as i64
}

fn num(config: &Value, key: &str, default: f64) -> f64 {
    config.get(key).and_then(Value::as_f64).unwrap_or(default)
}

impl Plugin for RandomPlugin {
    fn manifest() -> Manifest {
        Manifest::new("random", "Random", "0.1.0")
            .icon("mdi mdi-dice-5")
            .color("#9b6bdf")
            // A number in [min, max]. `integer: true` rounds to a whole number.
            .action(
                ActionDecl::new("number")
                    .config(Schema::object([
                        ("min".to_string(), Schema::number()),
                        ("max".to_string(), Schema::number()),
                        ("integer".to_string(), Schema::boolean()),
                    ]))
                    .result(Schema::number()),
            )
            // Roll `count` dice of `sides` faces → { total, rolls: [..] }.
            .action(
                ActionDecl::new("dice")
                    .config(Schema::object([
                        ("count".to_string(), Schema::number()),
                        ("sides".to_string(), Schema::number()),
                    ]))
                    .result(Schema::object([
                        ("total".to_string(), Schema::number()),
                        ("rolls".to_string(), Schema::array(Schema::number())),
                    ])),
            )
            // Heads or tails.
            .action(ActionDecl::new("coin").result(Schema::string()))
            // True with `percent`% probability (0..100) — gate a branch on luck.
            .action(
                ActionDecl::new("chance")
                    .config(Schema::object([("percent".to_string(), Schema::number())]))
                    .result(Schema::boolean()),
            )
            // Pick one element out of `items` (any JSON values).
            .action(
                ActionDecl::new("pick")
                    .config(Schema::object([(
                        "items".to_string(),
                        Schema::array(Schema::string()),
                    )]))
                    .result(Schema::string()),
            )
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        match action {
            "number" => {
                let min = num(&config, "min", 0.0);
                let max = num(&config, "max", 1.0);
                let integer = config
                    .get("integer")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if integer {
                    Ok(json!(rand_int(min as i64, max as i64)))
                } else {
                    let (lo, hi) = if min <= max { (min, max) } else { (max, min) };
                    Ok(json!(lo + rand_unit() * (hi - lo)))
                }
            }
            "dice" => {
                let count = num(&config, "count", 1.0).max(0.0) as u64;
                let sides = num(&config, "sides", 6.0).max(1.0) as i64;
                let rolls: Vec<i64> = (0..count).map(|_| rand_int(1, sides)).collect();
                let total: i64 = rolls.iter().sum();
                Ok(json!({ "total": total, "rolls": rolls }))
            }
            "coin" => Ok(json!(if rand_u64() & 1 == 0 { "heads" } else { "tails" })),
            "chance" => {
                let percent = num(&config, "percent", 50.0).clamp(0.0, 100.0);
                Ok(json!(rand_unit() * 100.0 < percent))
            }
            "pick" => {
                let items = config
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                if items.is_empty() {
                    return Err(ActionError::failed("pick: `items` is empty"));
                }
                let i = rand_int(0, items.len() as i64 - 1) as usize;
                Ok(items[i].clone())
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(RandomPlugin);

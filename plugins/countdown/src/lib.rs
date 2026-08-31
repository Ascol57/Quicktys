//! `countdown` plugin — **named** countdown timers, driven from automations and
//! rendered by the `countdown` widget.
//!
//! Like the `alert` plugin, every action takes a `name` so a user can run several
//! independent countdowns ("brb", "giveaway", …). Each one is published as the
//! reactive state key `countdown.<name>`, holding an object the widget watches:
//!
//! ```json
//! { "running": true, "endAt": 1756640000000, "remainingMs": 92000,
//!   "remaining": 92, "totalMs": 300000, "display": "01:32",
//!   "label": "Back soon", "seq": 3 }
//! ```
//!
//! **Who counts.** `endAt` is an absolute wall-clock instant, so nothing has to
//! tick host-side: the widget derives the remaining time from `endAt` on its own
//! animation frame, and this plugin only recomputes on an action. That keeps a
//! second-accurate display without a `timers` capability (which the plugin tier
//! does not get anyway).
//!
//! **The `finished` trigger** is therefore reported *back* by a displayed widget
//! when it observes the run reaching zero (`report-finished`), and deduplicated
//! here on `seq` so several widgets showing the same countdown fire it once. A
//! countdown nobody displays never fires it — see the plugin README.
//!
//! Timers are persisted through the `storage` capability, so a restart restores
//! them; a run that expired while the app was closed is restored as finished
//! *without* firing the trigger (same rule as `pear`'s first observation).

use quicktys_plugin_sdk::{
    json, ActionDecl, ActionError, Manifest, Plugin, Schema, StateDecl, TriggerDecl, Value,
};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

wit_bindgen::generate!({
    path: "../../wit",
    world: "plugin",
});

use quicktys::api::logging::{log, Level};
use quicktys::api::reactivity::{emit_trigger, set_state};
use quicktys::api::storage::{kv_get, kv_set};

/// Storage key holding every timer (the plugin's whole persisted state).
const STORE_KEY: &str = "timers";
/// Name used when an action leaves `name` empty.
const DEFAULT_NAME: &str = "timer";

/// One named countdown.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Timer {
    /// Epoch ms at which it reaches zero. Authoritative **while running**.
    end_at: i64,
    /// Remaining time, frozen, while paused or stopped.
    remaining_ms: i64,
    /// Length of the current run — the widget's progress bar needs a total.
    total_ms: i64,
    running: bool,
    label: String,
    /// Bumped on every (re)start: identifies one run.
    seq: u64,
    /// Last run we already fired `finished` for. Dedups repeated reports.
    finished_seq: u64,
}

impl Timer {
    fn remaining(&self, now: i64) -> i64 {
        if self.running {
            (self.end_at - now).max(0)
        } else {
            self.remaining_ms.max(0)
        }
    }
}

thread_local! {
    static TIMERS: RefCell<BTreeMap<String, Timer>> = RefCell::new(BTreeMap::new());
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// `mm:ss`, or `h:mm:ss` past an hour. Rounded **up**, so a 5s countdown reads
/// `00:05` the instant it starts rather than `00:04`.
fn format_clock(ms: i64) -> String {
    let total = (ms.max(0) + 999) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

/// The JSON the widget (and templates) read from `countdown.<name>`.
fn state_of(t: &Timer, now: i64) -> Value {
    let rem = t.remaining(now);
    json!({
        "running": t.running,
        "endAt": if t.running { t.end_at } else { 0 },
        "remainingMs": rem,
        "remaining": (rem + 999) / 1000,
        "totalMs": t.total_ms,
        "display": format_clock(rem),
        "label": t.label,
        "seq": t.seq,
    })
}

fn publish(name: &str, t: &Timer, now: i64) -> Value {
    let payload = state_of(t, now);
    // The host namespaces this as `countdown.<name>` in the state store.
    set_state(name, &payload.to_string());
    payload
}

fn save() {
    TIMERS.with(|m| {
        if let Ok(dump) = quicktys_plugin_sdk::serde_json::to_string(&*m.borrow()) {
            kv_set(STORE_KEY, &dump);
        }
    });
}

/// Read `name` from an action config, falling back to the default timer.
fn timer_name(config: &Value) -> String {
    let name = config
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        DEFAULT_NAME.to_string()
    } else {
        name.to_string()
    }
}

/// Apply `edit` to the named timer, then persist and publish it.
fn with_timer(name: &str, edit: impl FnOnce(&mut Timer, i64)) -> Value {
    let now = now_ms();
    let updated = TIMERS.with(|m| {
        let mut map = m.borrow_mut();
        let t = map.entry(name.to_string()).or_default();
        edit(t, now);
        t.clone()
    });
    save();
    publish(name, &updated, now)
}

struct CountdownPlugin;

impl Plugin for CountdownPlugin {
    fn manifest() -> Manifest {
        let name_field = || {
            Schema::string()
                .suggest("countdown")
                // Shared by every action, so it lives outside `actions.*`.
                .named("plugin.countdown.fields.name")
        };
        Manifest::new("countdown", "Countdown", "0.1.0")
            .icon("mdi mdi-timer-sand")
            .color("#e0a458")
            .action(
                ActionDecl::new("start").config(Schema::object([
                    ("name".to_string(), name_field()),
                    (
                        "duration".to_string(),
                        Schema::Duration {
                            common: Default::default(),
                            default: Some(300.0),
                            min: Some(0.0),
                            max: None,
                            template: true,
                        }
                        .named("plugin.countdown.actions.start.config.duration"),
                    ),
                    (
                        "label".to_string(),
                        Schema::template_string()
                            .named("plugin.countdown.actions.start.config.label"),
                    ),
                ])),
            )
            .action(
                ActionDecl::new("add").config(Schema::object([
                    ("name".to_string(), name_field()),
                    (
                        "seconds".to_string(),
                        Schema::Number {
                            common: Default::default(),
                            default: Some(60.0),
                            min: None,
                            max: None,
                            step: Some(1.0),
                            slider: false,
                            unit: Some("s".to_string()),
                            template: true,
                        }
                        .named("plugin.countdown.actions.add.config.seconds"),
                    ),
                ])),
            )
            .action(
                ActionDecl::new("pause")
                    .config(Schema::object([("name".to_string(), name_field())])),
            )
            .action(
                ActionDecl::new("resume")
                    .config(Schema::object([("name".to_string(), name_field())])),
            )
            .action(
                ActionDecl::new("stop")
                    .config(Schema::object([("name".to_string(), name_field())])),
            )
            // Fires the `finished` trigger. A displayed widget calls this when it
            // sees the run hit zero; a user can also call it to end a countdown
            // early and still run whatever the trigger is bound to.
            .action(
                ActionDecl::new("report-finished").config(Schema::object([
                    ("name".to_string(), name_field()),
                    (
                        "seq".to_string(),
                        Schema::number().named("plugin.countdown.actions.report-finished.config.seq"),
                    ),
                ])),
            )
            .trigger(
                TriggerDecl::new("finished").context(Schema::object([
                    (
                        "name".to_string(),
                        Schema::string().named("plugin.countdown.triggers.finished.context.name"),
                    ),
                    (
                        "label".to_string(),
                        Schema::string().named("plugin.countdown.triggers.finished.context.label"),
                    ),
                ])),
            )
            // The default timer, declared so `@countdown.timer.display` shows up
            // in the template autocomplete. Other names appear once first used.
            .state_field(StateDecl::new(DEFAULT_NAME).schema(Schema::object([
                ("running".to_string(), Schema::boolean()),
                ("display".to_string(), Schema::string()),
                ("remaining".to_string(), Schema::number()),
                ("remainingMs".to_string(), Schema::number()),
                ("totalMs".to_string(), Schema::number()),
                ("endAt".to_string(), Schema::number()),
                ("label".to_string(), Schema::string()),
                ("seq".to_string(), Schema::number()),
            ])))
    }

    /// Restore persisted timers and republish them, so widgets resync after a
    /// restart. A run that expired while the app was closed comes back finished
    /// and pre-acknowledged: no late `finished` trigger on boot.
    fn on_load() -> Result<(), String> {
        let Some(dump) = kv_get(STORE_KEY) else {
            return Ok(());
        };
        let mut map: BTreeMap<String, Timer> =
            quicktys_plugin_sdk::serde_json::from_str(&dump).unwrap_or_default();
        let now = now_ms();
        for t in map.values_mut() {
            if t.running && t.end_at <= now {
                t.running = false;
                t.remaining_ms = 0;
                t.finished_seq = t.seq;
            }
        }
        for (name, t) in &map {
            publish(name, t, now);
        }
        let restored = map.len();
        TIMERS.with(|m| *m.borrow_mut() = map);
        if restored > 0 {
            log(Level::Debug, &format!("countdown: restored {restored} timer(s)"));
        }
        Ok(())
    }

    fn invoke_action(action: &str, config: Value, _context: Value) -> Result<Value, ActionError> {
        let name = timer_name(&config);
        match action {
            "start" => {
                let seconds = config
                    .get("duration")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    .max(0.0);
                let ms = (seconds * 1000.0) as i64;
                let label = config
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                Ok(with_timer(&name, |t, now| {
                    t.seq += 1;
                    t.end_at = now + ms;
                    t.remaining_ms = ms;
                    t.total_ms = ms;
                    t.running = true;
                    t.label = label;
                }))
            }
            "add" => {
                let delta = (config
                    .get("seconds")
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
                    * 1000.0) as i64;
                Ok(with_timer(&name, |t, now| {
                    if t.running {
                        // Never push the end before now: subtracting more than
                        // what is left lands on zero, not in the past.
                        t.end_at = (t.end_at + delta).max(now);
                        t.remaining_ms = t.end_at - now;
                    } else {
                        t.remaining_ms = (t.remaining_ms + delta).max(0);
                    }
                    // Keep the total at least as large as what is left, so the
                    // widget's progress bar stays within [0, 1].
                    t.total_ms = t.total_ms.saturating_add(delta).max(t.remaining(now));
                }))
            }
            "pause" => Ok(with_timer(&name, |t, now| {
                if t.running {
                    t.remaining_ms = t.remaining(now);
                    t.running = false;
                    t.end_at = 0;
                }
            })),
            "resume" => Ok(with_timer(&name, |t, now| {
                if !t.running && t.remaining_ms > 0 {
                    t.end_at = now + t.remaining_ms;
                    t.running = true;
                }
            })),
            // Clear it. The run is marked acknowledged so a widget that was
            // mid-tick cannot report a `finished` for a countdown we cancelled.
            "stop" => Ok(with_timer(&name, |t, _now| {
                t.running = false;
                t.end_at = 0;
                t.remaining_ms = 0;
                t.total_ms = 0;
                t.finished_seq = t.seq;
            })),
            "report-finished" => {
                let reported = config.get("seq").and_then(Value::as_u64);
                let mut fired: Option<String> = None;
                let payload = with_timer(&name, |t, _now| {
                    let seq = reported.unwrap_or(t.seq);
                    // Only the current run, and only once.
                    if seq == t.seq && t.seq > 0 && t.finished_seq < t.seq {
                        t.finished_seq = t.seq;
                        t.running = false;
                        t.end_at = 0;
                        t.remaining_ms = 0;
                        fired = Some(t.label.clone());
                    }
                });
                if let Some(label) = fired {
                    let ctx = json!({ "name": name, "label": label });
                    emit_trigger("finished", &ctx.to_string());
                }
                Ok(payload)
            }
            other => Err(ActionError::failed(format!("unknown action: {other}"))),
        }
    }
}

quicktys_plugin_sdk::define_plugin!(CountdownPlugin);

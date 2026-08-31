//! Host-side reactive state store.
//!
//! Holds the live state values that plugins/modules publish (`set-state`) and
//! that expressions read. Built on `tokio::sync` (a `broadcast` of changed
//! keys + an `RwLock` of values) — NOT an auto-tracking signals framework:
//! Quicktys's dependency graph is **explicit**, extracted from the Rhai AST
//! (`quicktys_expr::CompiledExpr::deps`). So re-evaluation is driven by "did one
//! of *these named keys* change", which a key-addressed broadcast models
//! directly (ARCHITECTURE.md §16).
//!
//! [`StateStore::wait_until`] is the reactive primitive behind the runner's
//! `WaitFor::Until`: it re-checks a predicate only when a key in its dependency
//! set changes.

use serde_json::Value;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::broadcast;

/// Capacity of the change-broadcast buffer. A slow waiter that overflows it sees
/// a `Lagged` error, which we treat as "something changed, re-check" — correct,
/// just less precise.
const CHANGE_BUFFER: usize = 256;

/// Live reactive state: `key → JSON value`, with change notifications by key.
pub struct StateStore {
    values: RwLock<serde_json::Map<String, Value>>,
    changes: broadcast::Sender<String>,
}

impl StateStore {
    pub fn new() -> Self {
        let (changes, _) = broadcast::channel(CHANGE_BUFFER);
        StateStore {
            values: RwLock::new(serde_json::Map::new()),
            changes,
        }
    }

    /// Set a state value. Notifies subscribers only if the value actually
    /// changed (idempotent writes are silent).
    pub fn set(&self, key: impl Into<String>, value: Value) {
        let key = key.into();
        {
            let mut values = self.values.write().unwrap();
            if values.get(&key) == Some(&value) {
                return;
            }
            values.insert(key.clone(), value);
        }
        // Err only means there are no subscribers — fine.
        let _ = self.changes.send(key);
    }

    /// Read one value.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.values.read().unwrap().get(key).cloned()
    }

    /// A full snapshot of current state (used to build an evaluation context).
    pub fn snapshot(&self) -> serde_json::Map<String, Value> {
        self.values.read().unwrap().clone()
    }

    /// Subscribe to the stream of changed key names.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.changes.subscribe()
    }

    /// Await until `predicate` holds, re-checking it whenever a key in `deps`
    /// changes (or on any change if `deps` is empty). Returns `true` when the
    /// predicate became true, `false` if `timeout` elapsed first.
    ///
    /// Subscribes *before* the first check, so a change racing the initial
    /// evaluation is never missed.
    pub async fn wait_until<F>(
        &self,
        deps: &[String],
        mut predicate: F,
        timeout: Option<Duration>,
    ) -> bool
    where
        F: FnMut() -> bool,
    {
        let mut rx = self.changes.subscribe();
        let inner = async {
            if predicate() {
                return true;
            }
            loop {
                match rx.recv().await {
                    Ok(key) => {
                        // A dependency is a ROOT identifier (`obs`), because that
                        // is what the expression engine sees once dotted keys
                        // are nested — so a change to `obs.scene` must wake a
                        // condition that depends on `obs`.
                        let relevant = deps.is_empty()
                            || deps.iter().any(|d| {
                                key == *d
                                    || (key.len() > d.len()
                                        && key.starts_with(d.as_str())
                                        && key.as_bytes()[d.len()] == b'.')
                            });
                        if relevant && predicate() {
                            return true;
                        }
                    }
                    // Buffer overflow: be conservative and re-check.
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if predicate() {
                            return true;
                        }
                    }
                    // Sender gone: no more changes can arrive.
                    Err(broadcast::error::RecvError::Closed) => return predicate(),
                }
            }
        };
        match timeout {
            Some(d) => tokio::time::timeout(d, inner).await.unwrap_or(false),
            None => inner.await,
        }
    }
}

impl Default for StateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn set_get_snapshot() {
        let s = StateStore::new();
        s.set("a", json!(1));
        s.set("b", json!("x"));
        assert_eq!(s.get("a"), Some(json!(1)));
        assert_eq!(s.snapshot().len(), 2);
        assert_eq!(s.get("missing"), None);
    }

    #[tokio::test]
    async fn wait_until_is_immediate_when_already_true() {
        let s = StateStore::new();
        s.set("ready", json!(true));
        let ready = s
            .wait_until(&["ready".into()], || s.get("ready") == Some(json!(true)), None)
            .await;
        assert!(ready);
    }

    #[tokio::test]
    async fn wait_until_fires_on_relevant_change() {
        let store = Arc::new(StateStore::new());
        store.set("a", json!(1));

        let waiter_store = store.clone();
        let handle = tokio::spawn(async move {
            let pred_store = waiter_store.clone();
            waiter_store
                .wait_until(
                    &["a".into()],
                    move || pred_store.get("a") == Some(json!(5)),
                    Some(Duration::from_secs(5)),
                )
                .await
        });

        // Let the waiter subscribe and park on the initial (false) check.
        tokio::task::yield_now().await;
        store.set("a", json!(5));
        assert!(handle.await.unwrap());
    }

    #[tokio::test]
    async fn wait_until_times_out() {
        let s = StateStore::new();
        let ready = s
            .wait_until(&["x".into()], || false, Some(Duration::from_millis(30)))
            .await;
        assert!(!ready);
    }

    #[tokio::test]
    async fn irrelevant_change_does_not_satisfy() {
        // A change to a key NOT in deps must not end the wait early; the timeout
        // does. (Predicate only true for "a", but we only touch "b".)
        let store = Arc::new(StateStore::new());
        let waiter = store.clone();
        let ready = tokio::spawn(async move {
            let p = waiter.clone();
            waiter
                .wait_until(
                    &["a".into()],
                    move || p.get("a") == Some(json!(1)),
                    Some(Duration::from_millis(40)),
                )
                .await
        });
        tokio::task::yield_now().await;
        store.set("b", json!(99)); // irrelevant to deps ["a"]
        assert!(!ready.await.unwrap());
    }
}

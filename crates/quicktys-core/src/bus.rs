//! The trigger event bus.
//!
//! Plugins fire **triggers** (`emit-trigger`); profiles match them and run the
//! bound automation (see [`crate::profile`]). The bus is also how the runner's
//! `WaitFor::Event` parks until a specific trigger fires.
//!
//! Built on a `tokio::sync::broadcast` of [`TriggerEvent`]s — transient
//! fan-out (unlike [`crate::reactivity`], which holds durable state).

use serde_json::Value;
use std::time::Duration;
use tokio::sync::broadcast;

/// Buffer of in-flight events per subscriber. A subscriber that overflows it
/// sees `Lagged` and skips the missed events (acceptable for fire-and-forget
/// triggers).
const EVENT_BUFFER: usize = 256;

/// A fired trigger: its id and its context payload (user, message, ...).
#[derive(Clone, Debug)]
pub struct TriggerEvent {
    pub id: String,
    pub context: serde_json::Map<String, Value>,
}

impl TriggerEvent {
    pub fn new(id: impl Into<String>, context: serde_json::Map<String, Value>) -> Self {
        TriggerEvent {
            id: id.into(),
            context,
        }
    }
}

/// Fan-out bus for trigger events.
pub struct TriggerBus {
    events: broadcast::Sender<TriggerEvent>,
}

impl TriggerBus {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(EVENT_BUFFER);
        TriggerBus { events }
    }

    /// Fire a trigger to all current subscribers.
    pub fn emit(&self, event: TriggerEvent) {
        // Err only means there are no subscribers — fine.
        let _ = self.events.send(event);
    }

    /// Subscribe to the raw event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<TriggerEvent> {
        self.events.subscribe()
    }

    /// Wait for the next event with id `trigger_id` that passes `filter`.
    ///
    /// Returns the matching event, or `None` if `timeout` elapses first.
    /// Subscribes before returning, so an event racing the call is not missed.
    pub async fn wait_for_event<F>(
        &self,
        trigger_id: &str,
        mut filter: F,
        timeout: Option<Duration>,
    ) -> Option<TriggerEvent>
    where
        F: FnMut(&TriggerEvent) -> bool,
    {
        let mut rx = self.events.subscribe();
        let inner = async {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if event.id == trigger_id && filter(&event) {
                            return Some(event);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return None,
                }
            }
        };
        match timeout {
            Some(d) => tokio::time::timeout(d, inner).await.unwrap_or(None),
            None => inner.await,
        }
    }
}

impl Default for TriggerBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    fn ctx(v: Value) -> serde_json::Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[tokio::test]
    async fn wait_for_event_matches_id_and_filter() {
        let bus = Arc::new(TriggerBus::new());
        let waiter_bus = bus.clone();
        let handle = tokio::spawn(async move {
            waiter_bus
                .wait_for_event(
                    "twitch.chat",
                    |e| e.context.get("user") == Some(&json!("ada")),
                    Some(Duration::from_secs(5)),
                )
                .await
        });

        tokio::task::yield_now().await;
        // Wrong id, then wrong user, then the match.
        bus.emit(TriggerEvent::new("other", ctx(json!({ "user": "ada" }))));
        bus.emit(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "bob" }))));
        bus.emit(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "ada" }))));

        let got = handle.await.unwrap().expect("should have matched");
        assert_eq!(got.context.get("user"), Some(&json!("ada")));
    }

    #[tokio::test]
    async fn wait_for_event_times_out() {
        let bus = TriggerBus::new();
        let got = bus
            .wait_for_event("never", |_| true, Some(Duration::from_millis(30)))
            .await;
        assert!(got.is_none());
    }
}

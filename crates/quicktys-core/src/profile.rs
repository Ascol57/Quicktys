//! Profiles, trigger bindings, and the dispatcher that turns a fired trigger
//! into running automations (ARCHITECTURE §6).
//!
//! A [`Profile`] is active when its (optional) activation condition holds over
//! the current state. An active profile's [`TriggerBinding`]s map a trigger id
//! to an [`Automation`]; when a matching trigger fires and the binding's filter
//! passes, the bound automation runs with the trigger's payload as its base
//! context.

use crate::bus::{TriggerBus, TriggerEvent};
use crate::reactivity::StateStore;
use crate::resources::{Resource, ResourceStore};
use crate::sequence::{truthy, RunError, Runner};
use quicktys_expr::{Context, ExprEngine};
use quicktys_schema::{Expr, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};

/// A set of trigger→automation bindings, active when its condition holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Boolean expression over state; `None` means always active.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation: Option<Expr>,
    #[serde(default)]
    pub triggers: Vec<TriggerBinding>,
}

impl Resource for Profile {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Binds a trigger id to an inline sequence, with an optional filter expression.
/// CastMate-style: a profile owns its triggers' sequences directly (the sequence
/// is edited in place in the profile, not referenced by id).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerBinding {
    /// Trigger id to match (e.g. `twitch.chat`).
    pub trigger: String,
    /// The node tree to run on a match.
    #[serde(default = "empty_sequence")]
    pub sequence: Node,
    /// Boolean expression over `state + the event payload`; `None` = always run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<Expr>,
}

/// An empty sequence (a block with no steps) — the default for a fresh trigger.
fn empty_sequence() -> Node {
    Node::Block { body: Vec::new() }
}

/// A named automation: an id and the root of its node tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub root: Node,
}

impl Resource for Automation {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Wires the trigger bus to profiles + automations + the runner.
#[derive(Clone)]
pub struct Dispatcher {
    runner: Runner,
    engine: Arc<ExprEngine>,
    state: Arc<StateStore>,
    profiles: Arc<RwLock<ResourceStore<Profile>>>,
}

impl Dispatcher {
    pub fn new(
        runner: Runner,
        engine: Arc<ExprEngine>,
        state: Arc<StateStore>,
        profiles: Arc<RwLock<ResourceStore<Profile>>>,
    ) -> Self {
        Dispatcher {
            runner,
            engine,
            state,
            profiles,
        }
    }

    /// Run every automation bound to `event` by an active, matching, passing
    /// binding, and collect their results. Bindings are resolved under a short
    /// read lock that is released before any automation runs.
    pub async fn dispatch(&self, event: TriggerEvent) -> Vec<Result<Option<Value>, RunError>> {
        let jobs = self.resolve(&event);
        let mut results = Vec::with_capacity(jobs.len());
        for (root, base) in jobs {
            results.push(self.runner.run(&root, base).await);
        }
        results
    }

    /// Subscribe to `bus` and dispatch every event, each on its own task, until
    /// the bus is dropped.
    pub async fn run_forever(self: Arc<Self>, bus: Arc<TriggerBus>) {
        let mut rx = bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let me = self.clone();
                    tokio::spawn(async move {
                        me.dispatch(event).await;
                    });
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    /// Resolve the `(sequence root, base context)` jobs an event triggers.
    fn resolve(&self, event: &TriggerEvent) -> Vec<(Node, Context)> {
        let profiles = self.profiles.read().unwrap();
        let state = self.state.snapshot();

        let mut jobs = Vec::new();
        for profile in profiles.iter() {
            // Profile activation (over state only).
            if let Some(activation) = &profile.activation {
                if !self.eval_truthy(activation, &state) {
                    continue;
                }
            }
            for binding in &profile.triggers {
                if binding.trigger != event.id {
                    continue;
                }
                // Filter (over state + the event payload).
                if let Some(filter) = &binding.filter {
                    let mut ctx = state.clone();
                    for (k, v) in &event.context {
                        ctx.insert(k.clone(), v.clone());
                    }
                    if !self.eval_truthy(filter, &ctx) {
                        continue;
                    }
                }
                jobs.push((binding.sequence.clone(), event.context.clone()));
            }
        }
        jobs
    }

    /// Evaluate an expression as a boolean; an eval error counts as `false`.
    fn eval_truthy(&self, expr: &Expr, ctx: &Context) -> bool {
        self.engine
            .eval_source(expr.source(), ctx)
            .map(|v| truthy(&v))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoker::{ActionInvoker, ActionResult, BoxFut};
    use crate::test_support::TempDir;
    use serde_json::json;
    use std::sync::Mutex;

    struct Recorder {
        calls: Mutex<Vec<(String, Value)>>,
    }
    impl Recorder {
        fn new() -> Arc<Self> {
            Arc::new(Recorder {
                calls: Mutex::new(Vec::new()),
            })
        }
        fn actions(&self) -> Vec<(String, Value)> {
            self.calls.lock().unwrap().clone()
        }
    }
    impl ActionInvoker for Recorder {
        fn invoke<'a>(&'a self, _p: &'a str, action: &'a str, config: Value) -> BoxFut<'a, ActionResult> {
            Box::pin(async move {
                self.calls
                    .lock()
                    .unwrap()
                    .push((action.to_string(), config.clone()));
                Ok(Value::Null)
            })
        }
    }

    fn ctx(v: Value) -> serde_json::Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    /// Build a dispatcher over a single configurable profile.
    fn setup(profile: Profile, recorder: Arc<Recorder>) -> Dispatcher {
        // Temp dir backs the store on disk; dispatch reads the in-memory copy,
        // so it can drop at the end of setup.
        let tmp_p = TempDir::new();
        let engine = Arc::new(ExprEngine::new());
        let state = Arc::new(StateStore::new());
        let runner = Runner::with_state(engine.clone(), recorder, state.clone());

        let mut profiles = ResourceStore::<Profile>::new(tmp_p.path());
        profiles.upsert(profile).unwrap();

        Dispatcher::new(runner, engine, state, Arc::new(RwLock::new(profiles)))
    }

    fn binding(filter: Option<&str>) -> TriggerBinding {
        TriggerBinding {
            trigger: "twitch.chat".into(),
            // Inline sequence: echo the chat user.
            sequence: Node::Action(quicktys_schema::node::ActionNode {
                plugin: "twitch".into(),
                action: "say".into(),
                config: json!("hi {{ user }}"),
                bind: None,
            }),
            filter: filter.map(Expr::new),
        }
    }

    #[tokio::test]
    async fn dispatch_runs_bound_automation_with_payload() {
        let rec = Recorder::new();
        let profile = Profile {
            id: "main".into(),
            name: None,
            activation: None,
            triggers: vec![binding(None)],
        };
        let d = setup(profile, rec.clone());
        d.dispatch(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "ada" }))))
            .await;
        assert_eq!(rec.actions(), vec![("say".into(), json!("hi ada"))]);
    }

    #[tokio::test]
    async fn inactive_profile_does_not_run() {
        let rec = Recorder::new();
        let profile = Profile {
            id: "main".into(),
            name: None,
            activation: Some(Expr::new("false")),
            triggers: vec![binding(None)],
        };
        let d = setup(profile, rec.clone());
        d.dispatch(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "ada" }))))
            .await;
        assert!(rec.actions().is_empty());
    }

    #[tokio::test]
    async fn filter_gates_the_binding() {
        let rec = Recorder::new();
        let profile = Profile {
            id: "main".into(),
            name: None,
            activation: None,
            triggers: vec![binding(Some(r#"user == "ada""#))],
        };
        let d = setup(profile, rec.clone());
        // Non-matching user: skipped.
        d.dispatch(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "bob" }))))
            .await;
        assert!(rec.actions().is_empty());
        // Matching user: runs.
        d.dispatch(TriggerEvent::new("twitch.chat", ctx(json!({ "user": "ada" }))))
            .await;
        assert_eq!(rec.actions().len(), 1);
    }

    #[tokio::test]
    async fn unmatched_trigger_id_is_ignored() {
        let rec = Recorder::new();
        let profile = Profile {
            id: "main".into(),
            name: None,
            activation: None,
            triggers: vec![binding(None)],
        };
        let d = setup(profile, rec.clone());
        d.dispatch(TriggerEvent::new("obs.scene", ctx(json!({}))))
            .await;
        assert!(rec.actions().is_empty());
    }
}

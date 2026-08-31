//! The automation runner — a tree-walking interpreter over the [`Node`] model.
//!
//! Executes an automation with structured concurrency, cooperative
//! cancellation, and bounded loops (design: `docs/automation-engine.md` §2.4).
//! All conditions/values/iterables are expressions evaluated by `quicktys-expr`
//! against the merged context `{ base (trigger + state) + automation locals }`.
//!
//! ## M1 scope & simplifications (documented, to refine later)
//! * **Flat local scope.** `let`/`set` mutate one variable map threaded through
//!   sequential execution; blocks do not yet open isolated lexical scopes.
//!   Proper lexical scoping is deferred. `Parallel` branches *do* get isolated
//!   (cloned) scopes — required for concurrency — and their mutations are
//!   discarded.
//! * **Branch control flow.** `break`/`continue`/`return` inside a `parallel`
//!   branch are ignored (only errors propagate). They work normally in
//!   sequential flow and loops.
//! * **`WaitFor::Until` / `WaitFor::Event`** need host reactivity / the trigger
//!   bus (not built yet) and currently return [`RunError::Unsupported`]. Plain
//!   `WaitFor::Delay` works.

use crate::bus::{TriggerBus, TriggerEvent};
use crate::cancel::Cancel;
use crate::invoker::{ActionInvoker, BoxFut};
use crate::reactivity::StateStore;
use crate::template;
use quicktys_expr::{Context, ExprEngine};
use quicktys_schema::node::{JoinPolicy, LoopNode, Node, Track, WaitNode};
use quicktys_schema::Expr;
use futures::future::{join_all, select_all};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

/// Local variables of a running automation (name → value).
type Vars = serde_json::Map<String, Value>;

/// A failure during a run.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RunError {
    /// The run was cancelled.
    #[error("automation aborted")]
    Aborted,
    /// An expression failed to compile or evaluate.
    #[error("expression error: {0}")]
    Expr(String),
    /// A plugin action returned an error.
    #[error("action {plugin}::{action} failed: {message}")]
    Action {
        plugin: String,
        action: String,
        message: String,
    },
    /// A loop asked for more iterations than its `max` safety cap allows.
    #[error("loop exceeded its max of {max} iterations")]
    LoopBudgetExceeded { max: u32 },
    /// A value had the wrong shape for the operation (e.g. for-each over a
    /// non-array).
    #[error("type error: {0}")]
    Type(String),
    /// A `WaitFor` reached its timeout before its condition/event occurred.
    #[error("wait-for timed out")]
    WaitTimeout,
    /// A node kind not yet supported in this milestone.
    #[error("unsupported in M1: {0}")]
    Unsupported(String),
}

/// How a node finished: normally, or via a control-flow jump.
#[derive(Debug, Clone, PartialEq)]
enum Flow {
    Normal,
    Break,
    Continue,
    Return(Option<Value>),
}

/// Receives live per-node execution telemetry during a run, so a UI can light
/// up the node that is actually executing (and for how long it really runs).
/// `path` addresses a node by its index route from the root sequence
/// (currently only top-level nodes, `[i]`).
pub trait ProgressSink: Send + Sync {
    /// `running = true` when the node starts, `false` when it finishes.
    fn node(&self, path: &[usize], running: bool);
}

/// Runs automations. Holds the shared expression engine, action invoker, and
/// (optionally) the reactive state store that backs `WaitFor::Until` and feeds
/// live state into the evaluation context.
#[derive(Clone)]
pub struct Runner {
    engine: Arc<ExprEngine>,
    invoker: Arc<dyn ActionInvoker>,
    state: Option<Arc<StateStore>>,
    bus: Option<Arc<TriggerBus>>,
}

impl Runner {
    /// A runner without reactive state: `WaitFor::Until` is unavailable and the
    /// context is just `base + locals`.
    pub fn new(engine: Arc<ExprEngine>, invoker: Arc<dyn ActionInvoker>) -> Self {
        Runner {
            engine,
            invoker,
            state: None,
            bus: None,
        }
    }

    /// A runner wired to a [`StateStore`]: live state is merged into the
    /// evaluation context and `WaitFor::Until` works.
    pub fn with_state(
        engine: Arc<ExprEngine>,
        invoker: Arc<dyn ActionInvoker>,
        state: Arc<StateStore>,
    ) -> Self {
        Runner {
            engine,
            invoker,
            state: Some(state),
            bus: None,
        }
    }

    /// Attach a [`TriggerBus`] so `WaitFor::Event` can park on incoming
    /// triggers. Chainable: `Runner::with_state(..).with_bus(bus)`.
    pub fn with_bus(mut self, bus: Arc<TriggerBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Run `root` over `base` context, returning the automation's return value
    /// (if any). Creates and owns the cancellation token.
    pub async fn run(&self, root: &Node, base: Context) -> Result<Option<Value>, RunError> {
        self.run_with_cancel(root, base, Cancel::new()).await
    }

    /// Like [`run`](Self::run) but driven by a caller-supplied [`Cancel`], so
    /// the run can be aborted from elsewhere.
    pub async fn run_with_cancel(
        &self,
        root: &Node,
        base: Context,
        token: Cancel,
    ) -> Result<Option<Value>, RunError> {
        let run = Run {
            engine: &self.engine,
            invoker: self.invoker.as_ref(),
            base: &base,
            state: self.state.as_deref(),
            bus: self.bus.as_deref(),
            token,
        };
        let mut vars = Vars::new();
        match run.exec(root, &mut vars).await? {
            Flow::Return(v) => Ok(v),
            _ => Ok(None),
        }
    }

    /// Like [`run`](Self::run) but reports live per-node telemetry to
    /// `progress`. When `root` is a sequence, each top-level node is bracketed
    /// with `node([i], true/false)`, so the sink observes the *real* start and
    /// end (hence real duration) of every step as it executes.
    pub async fn run_with_progress(
        &self,
        root: &Node,
        base: Context,
        progress: Arc<dyn ProgressSink>,
    ) -> Result<Option<Value>, RunError> {
        let run = Run {
            engine: &self.engine,
            invoker: self.invoker.as_ref(),
            base: &base,
            state: self.state.as_deref(),
            bus: self.bus.as_deref(),
            token: Cancel::new(),
        };
        let mut vars = Vars::new();
        let Node::Block { body } = root else {
            // Not a sequence: report the whole run as node [0].
            progress.node(&[0], true);
            let out = run.exec(root, &mut vars).await;
            progress.node(&[0], false);
            return match out? {
                Flow::Return(v) => Ok(v),
                _ => Ok(None),
            };
        };
        for (i, child) in body.iter().enumerate() {
            progress.node(&[i], true);
            let flow = run.exec(child, &mut vars).await;
            progress.node(&[i], false);
            match flow? {
                Flow::Normal => {}
                Flow::Return(v) => return Ok(v),
                Flow::Break | Flow::Continue => return Ok(None),
            }
        }
        Ok(None)
    }
}

/// One live run: borrows the engine/invoker/base and owns the run's token.
struct Run<'a> {
    engine: &'a ExprEngine,
    invoker: &'a dyn ActionInvoker,
    base: &'a Context,
    state: Option<&'a StateStore>,
    bus: Option<&'a TriggerBus>,
    token: Cancel,
}

impl<'a> Run<'a> {
    /// Execute one node. Recursive, so it returns a boxed future.
    fn exec<'s>(&'s self, node: &'s Node, vars: &'s mut Vars) -> BoxFut<'s, Result<Flow, RunError>> {
        Box::pin(async move {
            if self.token.is_cancelled() {
                return Err(RunError::Aborted);
            }
            match node {
                Node::Action(a) => {
                    let config = self.resolve_config(&a.config, vars)?;
                    let result = self
                        .guard(self.invoker.invoke(&a.plugin, &a.action, config))
                        .await?
                        .map_err(|message| RunError::Action {
                            plugin: a.plugin.clone(),
                            action: a.action.clone(),
                            message,
                        })?;
                    if let Some(bind) = &a.bind {
                        vars.insert(bind.clone(), result);
                    }
                    Ok(Flow::Normal)
                }

                Node::Block { body } => {
                    for child in body {
                        match self.exec(child, vars).await? {
                            Flow::Normal => {}
                            other => return Ok(other),
                        }
                    }
                    Ok(Flow::Normal)
                }

                Node::Parallel { join, branches } => self.run_parallel(*join, branches, vars).await,

                Node::Timeline { tracks } => self.run_timeline(tracks, vars).await,

                Node::If {
                    cond,
                    then,
                    otherwise,
                } => {
                    if truthy(&self.eval(cond, vars)?) {
                        self.exec(then, vars).await
                    } else if let Some(other) = otherwise {
                        self.exec(other, vars).await
                    } else {
                        Ok(Flow::Normal)
                    }
                }

                Node::Switch {
                    value,
                    cases,
                    default,
                } => {
                    let v = self.eval(value, vars)?;
                    for case in cases {
                        if case.equals == v {
                            return self.exec(&case.body, vars).await;
                        }
                    }
                    if let Some(d) = default {
                        self.exec(d, vars).await
                    } else {
                        Ok(Flow::Normal)
                    }
                }

                Node::Loop(loop_node) => self.run_loop(loop_node, vars).await,

                Node::Let { name, value } => {
                    let v = self.eval(value, vars)?;
                    vars.insert(name.clone(), v);
                    Ok(Flow::Normal)
                }
                Node::Set { target, value } => {
                    let v = self.eval(value, vars)?;
                    vars.insert(target.clone(), v);
                    Ok(Flow::Normal)
                }

                Node::Try { body, catch, retry } => self.run_try(body, catch, retry, vars).await,

                Node::WaitFor(wait) => self.run_wait(wait, vars).await,

                Node::Return { value } => {
                    let v = match value {
                        Some(e) => Some(self.eval(e, vars)?),
                        None => None,
                    };
                    Ok(Flow::Return(v))
                }
                Node::Break => Ok(Flow::Break),
                Node::Continue => Ok(Flow::Continue),
            }
        })
    }

    async fn run_parallel(
        &self,
        join: JoinPolicy,
        branches: &[Node],
        vars: &Vars,
    ) -> Result<Flow, RunError> {
        // Each branch gets an isolated (cloned) scope; mutations are discarded.
        let futs: Vec<BoxFut<Result<Flow, RunError>>> = branches
            .iter()
            .map(|b| {
                let mut v = vars.clone();
                Box::pin(async move { self.exec(b, &mut v).await }) as BoxFut<_>
            })
            .collect();

        match join {
            JoinPolicy::All => {
                // Wait for every branch; first error wins. Branch control flow
                // is ignored (M1).
                for r in join_all(futs).await {
                    r?;
                }
                Ok(Flow::Normal)
            }
            JoinPolicy::Any | JoinPolicy::Race => {
                // First branch to finish wins; the rest are dropped (cancelled
                // at their next await point).
                if futs.is_empty() {
                    return Ok(Flow::Normal);
                }
                let (result, _idx, _rest) = select_all(futs).await;
                result.map(|_| Flow::Normal)
            }
        }
    }

    async fn run_timeline(&self, tracks: &[Track], vars: &Vars) -> Result<Flow, RunError> {
        // Tracks start together, each after its own offset, and the timeline
        // joins when all have finished.
        let futs: Vec<BoxFut<Result<Flow, RunError>>> = tracks
            .iter()
            .map(|t| {
                let mut v = vars.clone();
                Box::pin(async move {
                    if t.offset_ms > 0 {
                        self.guard(tokio::time::sleep(Duration::from_millis(t.offset_ms)))
                            .await?;
                    }
                    self.exec(&t.body, &mut v).await
                }) as BoxFut<_>
            })
            .collect();
        for r in join_all(futs).await {
            r?;
        }
        Ok(Flow::Normal)
    }

    async fn run_loop(&self, loop_node: &LoopNode, vars: &mut Vars) -> Result<Flow, RunError> {
        match loop_node {
            LoopNode::Repeat { count, body, max } => {
                let requested = to_count(&self.eval(count, vars)?);
                if requested > *max as u64 {
                    return Err(RunError::LoopBudgetExceeded { max: *max });
                }
                for _ in 0..requested {
                    match self.exec(body, vars).await? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        ret @ Flow::Return(_) => return Ok(ret),
                    }
                }
                Ok(Flow::Normal)
            }
            LoopNode::While { cond, body, max } => {
                let mut iters = 0u32;
                while truthy(&self.eval(cond, vars)?) {
                    if iters >= *max {
                        return Err(RunError::LoopBudgetExceeded { max: *max });
                    }
                    iters += 1;
                    match self.exec(body, vars).await? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        ret @ Flow::Return(_) => return Ok(ret),
                    }
                }
                Ok(Flow::Normal)
            }
            LoopNode::ForEach {
                var,
                list,
                body,
                max,
            } => {
                let value = self.eval(list, vars)?;
                let items = value
                    .as_array()
                    .ok_or_else(|| RunError::Type("for-each expects an array".into()))?;
                if items.len() as u64 > *max as u64 {
                    return Err(RunError::LoopBudgetExceeded { max: *max });
                }
                for item in items.clone() {
                    vars.insert(var.clone(), item);
                    match self.exec(body, vars).await? {
                        Flow::Normal | Flow::Continue => {}
                        Flow::Break => break,
                        ret @ Flow::Return(_) => return Ok(ret),
                    }
                }
                Ok(Flow::Normal)
            }
        }
    }

    async fn run_try(
        &self,
        body: &Node,
        catch: &Option<Box<Node>>,
        retry: &Option<quicktys_schema::node::Retry>,
        vars: &mut Vars,
    ) -> Result<Flow, RunError> {
        let max_retries = retry.map(|r| r.max).unwrap_or(0);
        let delay = retry.map(|r| r.delay_ms).unwrap_or(0);

        let mut attempt = 0u32;
        loop {
            match self.exec(body, vars).await {
                Ok(flow) => return Ok(flow),
                // Abort is not a recoverable failure: never retry or catch it.
                Err(RunError::Aborted) => return Err(RunError::Aborted),
                Err(e) => {
                    if attempt < max_retries {
                        attempt += 1;
                        if delay > 0 {
                            self.guard(tokio::time::sleep(Duration::from_millis(delay)))
                                .await?;
                        }
                        continue;
                    }
                    // Retries exhausted: run the catch handler, or surface the error.
                    return match catch {
                        Some(handler) => self.exec(handler, vars).await,
                        None => Err(e),
                    };
                }
            }
        }
    }

    async fn run_wait(&self, wait: &WaitNode, vars: &Vars) -> Result<Flow, RunError> {
        match wait {
            WaitNode::Delay { ms } => {
                let ms = to_count(&self.eval(ms, vars)?);
                self.guard(tokio::time::sleep(Duration::from_millis(ms)))
                    .await?;
                Ok(Flow::Normal)
            }
            WaitNode::Until { cond, timeout_ms } => {
                let Some(store) = self.state else {
                    return Err(RunError::Unsupported(
                        "wait-for until (runner has no state store)".into(),
                    ));
                };
                // Compile once: gives us the dependency set for targeted re-eval.
                let compiled = self
                    .engine
                    .compile(cond.source())
                    .map_err(|e| RunError::Expr(e.to_string()))?;
                let deps = compiled.deps().to_vec();
                let timeout = timeout_ms.map(Duration::from_millis);
                // Re-evaluate the condition against live state on each relevant
                // change. An eval error counts as "not yet true" (keeps waiting
                // until the timeout) rather than aborting the run.
                let predicate = || {
                    let ctx = self.context(vars);
                    self.engine
                        .eval(&compiled, &ctx)
                        .map(|v| truthy(&v))
                        .unwrap_or(false)
                };
                let ready = self.guard(store.wait_until(&deps, predicate, timeout)).await?;
                if ready {
                    Ok(Flow::Normal)
                } else {
                    Err(RunError::WaitTimeout)
                }
            }
            WaitNode::Event {
                trigger,
                filter,
                timeout_ms,
            } => {
                let Some(bus) = self.bus else {
                    return Err(RunError::Unsupported(
                        "wait-for event (runner has no trigger bus)".into(),
                    ));
                };
                // Compile the optional filter once.
                let compiled = match filter {
                    Some(f) => Some(
                        self.engine
                            .compile(f.source())
                            .map_err(|e| RunError::Expr(e.to_string()))?,
                    ),
                    None => None,
                };
                let timeout = timeout_ms.map(Duration::from_millis);
                // Filter runs against `context + the event's own payload`.
                let predicate = |event: &TriggerEvent| match &compiled {
                    None => true,
                    Some(c) => {
                        let mut ctx = self.context(vars);
                        for (k, v) in &event.context {
                            ctx.insert(k.clone(), v.clone());
                        }
                        self.engine.eval(c, &ctx).map(|v| truthy(&v)).unwrap_or(false)
                    }
                };
                let got = self
                    .guard(bus.wait_for_event(trigger, predicate, timeout))
                    .await?;
                if got.is_some() {
                    Ok(Flow::Normal)
                } else {
                    Err(RunError::WaitTimeout)
                }
            }
        }
    }

    /// Race a future against cancellation.
    async fn guard<T>(&self, fut: impl std::future::Future<Output = T>) -> Result<T, RunError> {
        tokio::select! {
            biased;
            _ = self.token.cancelled() => Err(RunError::Aborted),
            out = fut => Ok(out),
        }
    }

    /// Evaluate an expression against `base + vars`.
    fn eval(&self, expr: &Expr, vars: &Vars) -> Result<Value, RunError> {
        let ctx = self.context(vars);
        self.engine
            .eval_source(expr.source(), &ctx)
            .map_err(|e| RunError::Expr(e.to_string()))
    }

    /// Resolve any `@variable` and `{{ }}` regions inside an action config,
    /// recursively (see [`crate::template`]). `@name.path` looks up a flat
    /// context/state key; `{{ }}` is Handlebars. `@` runs first.
    fn resolve_config(&self, config: &Value, vars: &Vars) -> Result<Value, RunError> {
        match config {
            Value::String(s) => {
                if !s.contains('@') && !s.contains("{{") && !s.contains('\\') {
                    return Ok(config.clone());
                }
                let ctx = self.context(vars);
                template::resolve(s, &ctx).map_err(RunError::Expr)
            }
            Value::Array(items) => Ok(Value::Array(
                items
                    .iter()
                    .map(|v| self.resolve_config(v, vars))
                    .collect::<Result<_, _>>()?,
            )),
            Value::Object(obj) => {
                let mut out = serde_json::Map::with_capacity(obj.len());
                for (k, v) in obj {
                    out.insert(k.clone(), self.resolve_config(v, vars)?);
                }
                Ok(Value::Object(out))
            }
            other => Ok(other.clone()),
        }
    }

    /// Build the evaluation context: base (trigger), then live reactive state,
    /// then locals (locals win on conflict).
    fn context(&self, vars: &Vars) -> Context {
        let mut ctx = self.base.clone();
        if let Some(state) = self.state {
            for (k, v) in state.snapshot() {
                ctx.insert(k, v);
            }
        }
        for (k, v) in vars {
            ctx.insert(k.clone(), v.clone());
        }
        ctx
    }
}

/// Truthiness for conditions: only `false`, `null`, `0`, `""`, `"false"`, and
/// empty collections are falsey.
pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty() && s != "false",
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// Interpret a value as a non-negative iteration/millisecond count.
fn to_count(value: &Value) -> u64 {
    match value.as_f64() {
        Some(f) if f.is_finite() && f > 0.0 => f as u64,
        _ => 0,
    }
}

#[cfg(test)]
#[path = "sequence_tests.rs"]
mod tests;

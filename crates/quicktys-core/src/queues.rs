//! Action queues — sequential, non-overlapping automation execution
//! (ARCHITECTURE §6, kept from CastMate).
//!
//! A queue runs **one automation at a time**, with a configurable `gap` between
//! them, so stream alerts don't talk over each other. It supports pause/resume,
//! skip (abort the running job), clear (drop pending), a completion history, and
//! replay of a past job.
//!
//! [`QueueConfig`] is the persisted resource (id + gap); [`ActionQueue`] is the
//! runtime: a background worker task pulling jobs off the queue and running them
//! through the [`Runner`].

use crate::cancel::Cancel;
use crate::resources::Resource;
use crate::sequence::{RunError, Runner};
use quicktys_expr::Context;
use quicktys_schema::Node;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Persisted queue configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Delay inserted *after* each job completes, before the next starts.
    #[serde(default)]
    pub gap_ms: u64,
}

impl Resource for QueueConfig {
    fn id(&self) -> &str {
        &self.id
    }
}

/// A unit of work for a queue: an automation root + its base context.
#[derive(Debug, Clone)]
pub struct QueuedJob {
    pub id: String,
    pub automation: Node,
    pub context: Context,
}

impl QueuedJob {
    pub fn new(id: impl Into<String>, automation: Node, context: Context) -> Self {
        QueuedJob {
            id: id.into(),
            automation,
            context,
        }
    }
}

/// How a job finished.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Completed(Option<Value>),
    Failed(String),
    /// Skipped / cancelled before completing.
    Aborted,
}

/// A completed job, retained for history and replay.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    job_id: String,
    status: JobStatus,
    job: QueuedJob,
}

impl HistoryEntry {
    pub fn job_id(&self) -> &str {
        &self.job_id
    }
    pub fn status(&self) -> &JobStatus {
        &self.status
    }
}

struct QueueInner {
    gap: Duration,
    runner: Runner,
    pending: Mutex<VecDeque<QueuedJob>>,
    history: Mutex<Vec<HistoryEntry>>,
    /// Cancellation token of the job currently running (for `skip`).
    current: Mutex<Option<Cancel>>,
    paused: AtomicBool,
    /// True from the moment a job is dequeued until the worker next goes idle.
    busy: AtomicBool,
    /// Wakes the worker on enqueue / resume.
    wake: Notify,
    /// Fires when the worker becomes idle (for [`ActionQueue::wait_idle`]).
    idle: Notify,
}

/// A running action queue. Drop stops its worker.
pub struct ActionQueue {
    inner: Arc<QueueInner>,
    worker: JoinHandle<()>,
}

impl ActionQueue {
    /// Start a queue with the given inter-job `gap`, spawning its worker on the
    /// current Tokio runtime.
    pub fn start(runner: Runner, gap: Duration) -> Self {
        let inner = Arc::new(QueueInner {
            gap,
            runner,
            pending: Mutex::new(VecDeque::new()),
            history: Mutex::new(Vec::new()),
            current: Mutex::new(None),
            paused: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            wake: Notify::new(),
            idle: Notify::new(),
        });
        let worker = tokio::spawn(run_worker(inner.clone()));
        ActionQueue { inner, worker }
    }

    /// Start a queue from a persisted [`QueueConfig`].
    pub fn from_config(runner: Runner, config: &QueueConfig) -> Self {
        Self::start(runner, Duration::from_millis(config.gap_ms))
    }

    /// Append a job to the back of the queue.
    pub fn enqueue(&self, job: QueuedJob) {
        self.inner.pending.lock().unwrap().push_back(job);
        self.inner.wake.notify_one();
    }

    /// Stop dequeuing new jobs (a job already running continues).
    pub fn pause(&self) {
        self.inner.paused.store(true, Ordering::SeqCst);
    }

    /// Resume dequeuing.
    pub fn resume(&self) {
        self.inner.paused.store(false, Ordering::SeqCst);
        self.inner.wake.notify_one();
    }

    pub fn is_paused(&self) -> bool {
        self.inner.paused.load(Ordering::SeqCst)
    }

    /// Abort the currently running job (it lands in history as `Aborted`).
    pub fn skip(&self) {
        if let Some(token) = self.inner.current.lock().unwrap().as_ref() {
            token.cancel();
        }
    }

    /// Drop all pending (not-yet-started) jobs.
    pub fn clear(&self) {
        self.inner.pending.lock().unwrap().clear();
    }

    /// Re-enqueue the most recent history job with the given id. Returns `false`
    /// if no such job is in history.
    pub fn replay(&self, job_id: &str) -> bool {
        let job = self
            .inner
            .history
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|e| e.job_id == job_id)
            .map(|e| e.job.clone());
        match job {
            Some(job) => {
                self.enqueue(job);
                true
            }
            None => false,
        }
    }

    pub fn pending_len(&self) -> usize {
        self.inner.pending.lock().unwrap().len()
    }

    pub fn history(&self) -> Vec<HistoryEntry> {
        self.inner.history.lock().unwrap().clone()
    }

    /// Resolve once the queue is idle: nothing pending and no job running.
    pub async fn wait_idle(&self) {
        loop {
            // Register before checking, so a become-idle notification between the
            // check and the await is not lost.
            let notified = self.inner.idle.notified();
            if self.inner.pending.lock().unwrap().is_empty()
                && !self.inner.busy.load(Ordering::SeqCst)
            {
                return;
            }
            notified.await;
        }
    }
}

impl Drop for ActionQueue {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

/// Pull the next job, blocking while paused or empty. Marks the queue idle each
/// time it has to wait.
async fn next_job(inner: &Arc<QueueInner>) -> QueuedJob {
    loop {
        let notified = inner.wake.notified();
        if !inner.paused.load(Ordering::SeqCst) {
            if let Some(job) = inner.pending.lock().unwrap().pop_front() {
                inner.busy.store(true, Ordering::SeqCst);
                return job;
            }
        }
        inner.busy.store(false, Ordering::SeqCst);
        inner.idle.notify_waiters();
        notified.await;
    }
}

async fn run_worker(inner: Arc<QueueInner>) {
    loop {
        let job = next_job(&inner).await;

        let token = Cancel::new();
        *inner.current.lock().unwrap() = Some(token.clone());

        let status = match inner
            .runner
            .run_with_cancel(&job.automation, job.context.clone(), token)
            .await
        {
            Ok(value) => JobStatus::Completed(value),
            Err(RunError::Aborted) => JobStatus::Aborted,
            Err(e) => JobStatus::Failed(e.to_string()),
        };

        *inner.current.lock().unwrap() = None;
        inner.history.lock().unwrap().push(HistoryEntry {
            job_id: job.id.clone(),
            status,
            job,
        });

        if inner.gap > Duration::ZERO {
            tokio::time::sleep(inner.gap).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invoker::{ActionInvoker, ActionResult, BoxFut};
    use quicktys_expr::ExprEngine;
    use quicktys_schema::node::{ActionNode, WaitNode};
    use quicktys_schema::Expr;
    use serde_json::json;
    use std::sync::Mutex as StdMutex;

    struct Recorder {
        actions: Arc<StdMutex<Vec<String>>>,
    }
    impl ActionInvoker for Recorder {
        fn invoke<'a>(&'a self, _p: &'a str, action: &'a str, _c: Value) -> BoxFut<'a, ActionResult> {
            let actions = self.actions.clone();
            let action = action.to_string();
            Box::pin(async move {
                actions.lock().unwrap().push(action);
                Ok(Value::Null)
            })
        }
    }

    fn queue_with(gap: Duration) -> (ActionQueue, Arc<StdMutex<Vec<String>>>) {
        let actions = Arc::new(StdMutex::new(Vec::new()));
        let runner = Runner::new(
            Arc::new(ExprEngine::new()),
            Arc::new(Recorder {
                actions: actions.clone(),
            }),
        );
        (ActionQueue::start(runner, gap), actions)
    }

    fn act(action: &str) -> Node {
        Node::Action(ActionNode {
            plugin: "p".into(),
            action: action.into(),
            config: json!({}),
            bind: None,
        })
    }

    #[tokio::test]
    async fn runs_jobs_in_order() {
        let (q, actions) = queue_with(Duration::ZERO);
        q.enqueue(QueuedJob::new("1", act("first"), Context::new()));
        q.enqueue(QueuedJob::new("2", act("second"), Context::new()));
        q.wait_idle().await;
        assert_eq!(*actions.lock().unwrap(), vec!["first", "second"]);
        assert_eq!(q.history().len(), 2);
        assert_eq!(q.history()[0].status(), &JobStatus::Completed(None));
    }

    #[tokio::test]
    async fn pause_holds_jobs_until_resume() {
        let (q, actions) = queue_with(Duration::ZERO);
        q.pause();
        q.enqueue(QueuedJob::new("1", act("held"), Context::new()));
        // Give the worker a chance; it must stay paused.
        for _ in 0..5 {
            tokio::task::yield_now().await;
        }
        assert!(actions.lock().unwrap().is_empty());
        assert_eq!(q.pending_len(), 1);

        q.resume();
        q.wait_idle().await;
        assert_eq!(*actions.lock().unwrap(), vec!["held"]);
    }

    #[tokio::test]
    async fn skip_aborts_the_running_job() {
        let (q, _actions) = queue_with(Duration::ZERO);
        // A job that parks on a long delay.
        let long = Node::WaitFor(WaitNode::Delay {
            ms: Expr::new("600000"),
        });
        q.enqueue(QueuedJob::new("slow", long, Context::new()));

        // Wait until it is actually running.
        loop {
            if q.inner.current.lock().unwrap().is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
        q.skip();
        q.wait_idle().await;

        let history = q.history();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status(), &JobStatus::Aborted);
    }

    #[tokio::test]
    async fn replay_reenqueues_a_history_job() {
        let (q, actions) = queue_with(Duration::ZERO);
        q.enqueue(QueuedJob::new("j", act("once"), Context::new()));
        q.wait_idle().await;
        assert!(q.replay("j"));
        q.wait_idle().await;
        assert_eq!(*actions.lock().unwrap(), vec!["once", "once"]);
        assert!(!q.replay("nonexistent"));
    }
}

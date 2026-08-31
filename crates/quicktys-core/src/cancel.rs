//! A lightweight, cheaply-cloneable cancellation token.
//!
//! Stands in for `tokio_util::sync::CancellationToken` (not vendored). Built on
//! a `watch` channel: cloning shares the same cancellation state, so cancelling
//! any handle aborts every awaiter. The automation runner threads one token
//! through a run; aborting it unwinds delays, action invokes and parallel
//! branches at their next await point.

use std::sync::Arc;
use tokio::sync::watch;

/// A cancellation signal. Clone freely; all clones observe the same state.
#[derive(Clone)]
pub struct Cancel {
    tx: Arc<watch::Sender<bool>>,
    rx: watch::Receiver<bool>,
}

impl Cancel {
    /// Create a fresh, un-cancelled token.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(false);
        Cancel {
            tx: Arc::new(tx),
            rx,
        }
    }

    /// Request cancellation. Idempotent.
    pub fn cancel(&self) {
        let _ = self.tx.send(true);
    }

    /// Whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        *self.rx.borrow()
    }

    /// Resolve once cancellation is requested (never, if it is not).
    pub async fn cancelled(&self) {
        let mut rx = self.rx.clone();
        if *rx.borrow() {
            return;
        }
        while rx.changed().await.is_ok() {
            if *rx.borrow() {
                return;
            }
        }
        // Sender dropped without cancelling: cannot be cancelled anymore. The
        // Arc<Sender> is held by every clone, so in practice this is unreachable
        // while a run is live; park so this future never falsely resolves.
        std::future::pending::<()>().await;
    }
}

impl Default for Cancel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_resolves_awaiters() {
        let token = Cancel::new();
        assert!(!token.is_cancelled());
        let clone = token.clone();
        // Resolves because we cancel before awaiting.
        clone.cancel();
        assert!(token.is_cancelled());
        token.cancelled().await; // returns immediately
    }

    #[tokio::test]
    async fn cancelled_waits_then_fires() {
        let token = Cancel::new();
        let waiter = token.clone();
        let fired = tokio::select! {
            _ = waiter.cancelled() => true,
            _ = async { token.cancel(); std::future::pending::<()>().await } => false,
        };
        assert!(fired);
    }
}

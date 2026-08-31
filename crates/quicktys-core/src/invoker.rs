//! The action-invocation boundary.
//!
//! The runner is pure control flow; it does not know how to *run* a plugin
//! action. That is delegated to an [`ActionInvoker`] — implemented by
//! `quicktys-host` over the WASM ABI in a later milestone, and by a mock in
//! tests. This keeps the runner fully testable without the WASM stack.
//!
//! The trait returns a boxed future (rather than using `async fn` in trait or
//! `async-trait`) so it stays object-safe: the runner holds `&dyn ActionInvoker`.

use serde_json::Value;
use std::future::Future;
use std::pin::Pin;

/// A boxed, `Send` future of `T`.
pub type BoxFut<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The result of invoking an action: a JSON value, or an error message.
pub type ActionResult = Result<Value, String>;

/// Executes a plugin action with a (already template-resolved) config.
pub trait ActionInvoker: Send + Sync {
    /// Invoke `plugin::action` with `config`, returning its result value.
    fn invoke<'a>(
        &'a self,
        plugin: &'a str,
        action: &'a str,
        config: Value,
    ) -> BoxFut<'a, ActionResult>;
}

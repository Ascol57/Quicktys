//! Quicktys expression engine — the unified replacement for CastMate's JS eval.
//!
//! One sandboxed engine ([Rhai]) powers BOTH:
//!   * templating  — `{{ expr }}` regions inside schema fields, and
//!   * automation  — conditions, loop counts/iterables, dynamic delays, etc.
//!
//! Runs HOST-side over an already-materialised context (trigger ctx + plugin/
//! module state + automation locals), so evaluation is synchronous, fast and
//! deterministic — no I/O during eval. Sandboxed: no OS/network access, with
//! operation + size limits (anti runaway-loop).
//!
//! Design decision vs CastMate: expressions may ONLY call a core-provided
//! standard library (math/string/list/object). No plugin-injected functions —
//! those would require an async ABI round-trip per call. Plugins/modules expose
//! **values** through state instead.
//!
//! Key capability the AST gives us (impossible with opaque JS): **static
//! analysis** — [`CompiledExpr::deps`] lists the context variables an expression
//! reads, enabling targeted reactive re-evaluation.
//!
//! ## Pipeline
//! ```text
//! source ──compile──▶ CompiledExpr{ ast, deps } ──eval(context)──▶ Value
//!                                                          └─coerce(schema)─▶ typed Value
//! ```
//! [`ExprEngine::resolve_field`] ties it together for a schema field.
//!
//! See `docs/automation-engine.md` for the full design.
//!
//! [Rhai]: https://rhai.rs

mod coerce;
mod compile;
mod engine;
mod error;
mod eval;
mod resolve;
mod value;

pub use coerce::coerce;
pub use compile::CompiledExpr;
pub use engine::ExprEngine;
pub use error::ExprError;
pub use eval::{nest_context, Context};
pub use value::{dynamic_to_json, json_to_dynamic};

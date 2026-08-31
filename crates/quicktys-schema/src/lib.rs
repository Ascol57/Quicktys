//! Quicktys schema system.
//!
//! The self-describing, type-safe definition of config/context/state/settings
//! fields. Schemas drive (ARCHITECTURE.md §5):
//!   * automatic form generation in the Vue UI,
//!   * validation of JSON payloads crossing the WASM ABI,
//!   * serialization/deserialization to/from YAML on disk,
//!   * `{{ ... }}` template resolution at action-invoke time.
//!
//! This crate also hosts the shared, serialized data models consumed by both the
//! UI and the runner:
//!   * the automation **node tree** ([`node::Node`] — see
//!     `docs/automation-engine.md`),
//!   * the **template-region** splitting of `{{ }}` strings ([`template`]) — the
//!     parsing half; evaluation lives in `quicktys-expr`.
//!
//! Expressions are stored as **source** ([`expr::Expr`]); compilation to a Rhai
//! AST and dependency extraction happen in `quicktys-expr` at load time, so this
//! crate carries no expression-engine dependency.

pub mod expr;
pub mod node;
pub mod schema;
pub mod template;
pub mod validate;

pub use expr::Expr;
pub use node::Node;
pub use schema::{Common, RangeValue, Schema};
pub use template::{TemplateRegion, TemplateString};
pub use validate::{validate, ValidationError};

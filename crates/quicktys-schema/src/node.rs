//! The automation node tree (serialized model, shared with the UI).
//!
//! An automation is a tree of typed nodes, persisted as YAML and executed by the
//! tree-walking runner in `quicktys-core`. This crate owns only the **data
//! model**; execution semantics (cancellation, structured concurrency, bounded
//! loops) live in the runner. See `docs/automation-engine.md` §2.
//!
//! All conditions / values / iterables are [`Expr`]s — stored as source here and
//! compiled by `quicktys-expr` at load time. Control flow and templating share
//! the one expression engine.

use crate::expr::Expr;
use serde::{Deserialize, Serialize};

/// One node of an automation flow. Serializes with a `kind` discriminator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Node {
    /// Invoke a plugin action; optionally bind its result to a local.
    Action(ActionNode),
    /// Ordered children sharing a lexical scope of local variables.
    Block { body: Vec<Node> },
    /// Concurrent children, joined per [`JoinPolicy`].
    Parallel {
        #[serde(default)]
        join: JoinPolicy,
        branches: Vec<Node>,
    },
    /// CastMate's visual timeline: time-offset tracks.
    Timeline { tracks: Vec<Track> },
    /// Inline conditional (replaces CastMate's `FlowAction`).
    If {
        cond: Expr,
        then: Box<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        otherwise: Option<Box<Node>>,
    },
    /// Multi-way branch on the value of an expression.
    Switch {
        value: Expr,
        cases: Vec<SwitchCase>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        default: Option<Box<Node>>,
    },
    /// Bounded loop (repeat / while / for-each).
    Loop(LoopNode),
    /// Declare a local variable from an expression.
    Let { name: String, value: Expr },
    /// Reassign an existing local.
    Set { target: String, value: Expr },
    /// Error handling: optional catch handler and/or retry policy.
    Try {
        body: Box<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        catch: Option<Box<Node>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        retry: Option<Retry>,
    },
    /// Wait for an event / condition / delay mid-sequence (with timeout).
    WaitFor(WaitNode),
    /// Return from the flow, optionally with a value.
    Return {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Expr>,
    },
    /// Break out of the nearest loop.
    Break,
    /// Continue the nearest loop.
    Continue,
}

/// Invoke `plugin::action` with a (possibly templated) config.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionNode {
    pub plugin: String,
    pub action: String,
    /// Config payload; may contain `{{ }}` regions resolved at run time.
    #[serde(default)]
    pub config: serde_json::Value,
    /// Bind the action's result to this local (replaces CastMate's
    /// `resultMapping`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
}

/// How a [`Node::Parallel`] waits on its branches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JoinPolicy {
    /// Wait for every branch to finish before continuing.
    #[default]
    All,
    /// Continue when the first branch finishes; cancel the rest.
    Any,
    /// Like `Any`, race semantics (first to complete wins).
    Race,
}

/// A time-offset track of a [`Node::Timeline`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub offset_ms: u64,
    pub body: Box<Node>,
}

/// The three bounded-loop forms. Each carries a hard `max` iteration cap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "loop", rename_all = "kebab-case")]
pub enum LoopNode {
    Repeat {
        count: Expr,
        body: Box<Node>,
        max: u32,
    },
    While {
        cond: Expr,
        body: Box<Node>,
        max: u32,
    },
    ForEach {
        var: String,
        list: Expr,
        body: Box<Node>,
        max: u32,
    },
}

/// The three mid-sequence wait forms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "wait", rename_all = "kebab-case")]
pub enum WaitNode {
    /// Wait until a trigger fires (optional filter) or the timeout elapses.
    Event {
        trigger: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filter: Option<Expr>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Wait until a condition over reactive state becomes true.
    Until {
        cond: Expr,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
    },
    /// Simple delay; the duration is an expression (dynamic delays).
    Delay { ms: Expr },
}

/// Retry policy for [`Node::Try`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Retry {
    pub max: u32,
    pub delay_ms: u64,
}

/// One case of a [`Node::Switch`]: run `body` when the switch value equals
/// `equals`. (A richer pattern language can replace the literal later.)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SwitchCase {
    pub equals: serde_json::Value,
    pub body: Node,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(node: &Node) -> Node {
        let yaml = serde_yaml::to_string(node).unwrap();
        serde_yaml::from_str(&yaml).unwrap_or_else(|e| panic!("reparse failed: {e}\n{yaml}"))
    }

    #[test]
    fn action_node_roundtrips() {
        let n = Node::Action(ActionNode {
            plugin: "obs".into(),
            action: "start_recording".into(),
            config: serde_json::json!({ "scene": "Main" }),
            bind: Some("rec".into()),
        });
        assert_eq!(roundtrip(&n), n);
    }

    #[test]
    fn parallel_join_all_is_the_default() {
        let yaml = "\
kind: parallel
branches:
  - { kind: action, plugin: a, action: x }
  - { kind: action, plugin: b, action: y }
";
        let n: Node = serde_yaml::from_str(yaml).unwrap();
        match n {
            Node::Parallel { join, branches } => {
                assert_eq!(join, JoinPolicy::All);
                assert_eq!(branches.len(), 2);
            }
            _ => panic!("expected parallel"),
        }
    }

    #[test]
    fn nested_if_with_loop_roundtrips() {
        let n = Node::If {
            cond: Expr::new("points > 100"),
            then: Box::new(Node::Loop(LoopNode::Repeat {
                count: Expr::new("3"),
                body: Box::new(Node::Break),
                max: 10,
            })),
            otherwise: Some(Box::new(Node::Continue)),
        };
        assert_eq!(roundtrip(&n), n);
    }

    #[test]
    fn wait_variants_roundtrip() {
        for n in [
            Node::WaitFor(WaitNode::Delay { ms: Expr::new("1000") }),
            Node::WaitFor(WaitNode::Until {
                cond: Expr::new("ready"),
                timeout_ms: Some(5000),
            }),
            Node::WaitFor(WaitNode::Event {
                trigger: "chat".into(),
                filter: None,
                timeout_ms: None,
            }),
        ] {
            assert_eq!(roundtrip(&n), n);
        }
    }

    #[test]
    fn switch_roundtrips() {
        let n = Node::Switch {
            value: Expr::new("cmd"),
            cases: vec![SwitchCase {
                equals: serde_json::json!("start"),
                body: Node::Break,
            }],
            default: Some(Box::new(Node::Continue)),
        };
        assert_eq!(roundtrip(&n), n);
    }
}

//! **Best-effort CastMate → Quicktys migrator** (M8, `docs/ROADMAP.md`).
//!
//! Reads a user's *CastMate data* — their YAML resource files (profiles,
//! automations) — and emits the equivalent **Quicktys** resources plus a
//! **migration report** (Markdown) listing everything that mapped, was skipped,
//! or needs manual review. Reading the user's data is explicitly allowed under
//! the clean-room rule (it is *their data*, not CastMate code); this crate
//! contains zero CastMate code.
//!
//! It is **best-effort by design**: CastMate's on-disk schema is not a stable
//! contract, so extraction is *defensive* (it accepts several field spellings)
//! and anything ambiguous is reported rather than guessed. The assumed input
//! shapes are documented on [`classify`] and the `map_*` helpers; tune them
//! against real data and re-run — the report tells you what to fix.
//!
//! The Quicktys structural types ([`Profile`], [`Automation`], [`Node`]) are the
//! output contract, so whatever this emits is always valid Quicktys YAML.

use anyhow::{Context, Result};
use quicktys_core::{Automation, Profile, TriggerBinding};
use quicktys_schema::node::ActionNode;
use quicktys_schema::Node;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// What a source file was recognised as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A CastMate *profile* — has `triggers`.
    Profile,
    /// A standalone *automation*/sequence — a list of actions or a `sequence`.
    Automation,
    /// Unrecognised shape — reported, not migrated.
    Unknown,
}

/// One migrated (or skipped) source file.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Source path, relative to the CastMate root.
    pub source: String,
    /// Derived resource id.
    pub id: String,
    /// What it was classified as.
    pub kind: Kind,
    /// Where it was written (relative to the out dir), or `None` if skipped.
    pub written: Option<String>,
    /// Number of actions mapped in this resource.
    pub actions: usize,
    /// Per-file review notes (unmapped fields, template expressions, …).
    pub warnings: Vec<String>,
}

/// The outcome of a migration run — renders to a Markdown report.
#[derive(Debug, Default, Clone)]
pub struct Report {
    /// One entry per source file seen.
    pub entries: Vec<Entry>,
    /// Files that failed to parse as YAML (path, error).
    pub errors: Vec<(String, String)>,
}

impl Report {
    /// Total actions mapped across every migrated resource.
    pub fn action_count(&self) -> usize {
        self.entries.iter().map(|e| e.actions).sum()
    }

    /// How many files of a given kind were successfully written.
    pub fn migrated(&self, kind: Kind) -> usize {
        self.entries
            .iter()
            .filter(|e| e.kind == kind && e.written.is_some())
            .count()
    }

    /// Whether anything needs a human — a warning or a parse error.
    pub fn needs_review(&self) -> bool {
        !self.errors.is_empty() || self.entries.iter().any(|e| !e.warnings.is_empty())
    }

    /// Render the human-facing migration report.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str("# Quicktys migration report\n\n");
        out.push_str(&format!(
            "- profiles migrated: **{}**\n- automations migrated: **{}**\n- sources seen: **{}**\n- parse errors: **{}**\n\n",
            self.migrated(Kind::Profile),
            self.migrated(Kind::Automation),
            self.entries.len(),
            self.errors.len(),
        ));

        if !self.errors.is_empty() {
            out.push_str("## Could not read\n\n");
            for (path, err) in &self.errors {
                out.push_str(&format!("- `{path}` — {err}\n"));
            }
            out.push('\n');
        }

        out.push_str("## Resources\n\n");
        for e in &self.entries {
            let status = match &e.written {
                Some(w) => format!("→ `{w}`"),
                None => "**skipped**".to_string(),
            };
            out.push_str(&format!("### `{}` ({:?}) {status}\n\n", e.source, e.kind));
            if e.warnings.is_empty() {
                out.push_str("_no review needed._\n\n");
            } else {
                for w in &e.warnings {
                    out.push_str(&format!("- ⚠️ {w}\n"));
                }
                out.push('\n');
            }
        }
        out
    }
}

/// Migrate every YAML resource under `castmate_dir` into Quicktys resources in
/// `out_dir` (`out_dir/profiles/*.yaml`, `out_dir/automations/*.yaml`), and
/// return the [`Report`]. Also writes `out_dir/migration-report.md`.
pub fn migrate(castmate_dir: &Path, out_dir: &Path) -> Result<Report> {
    let mut report = Report::default();
    let mut files = Vec::new();
    collect_yaml(castmate_dir, &mut files)?;
    files.sort();

    for path in files {
        let rel = path
            .strip_prefix(castmate_dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                report.errors.push((rel, e.to_string()));
                continue;
            }
        };
        // serde_yaml deserialises straight into a serde_json::Value, so the whole
        // pipeline works in JSON (which is also ActionNode::config's type).
        let value: Value = match serde_yaml::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                report.errors.push((rel, e.to_string()));
                continue;
            }
        };

        let id = resource_id(&value, &path);
        let entry = migrate_one(&value, &rel, &id, out_dir);
        report.entries.push(entry);
    }

    let md = report.to_markdown();
    fs::create_dir_all(out_dir).ok();
    fs::write(out_dir.join("migration-report.md"), md)
        .with_context(|| "writing migration-report.md")?;
    Ok(report)
}

/// Migrate a single parsed resource; writes it and returns its [`Entry`].
fn migrate_one(value: &Value, rel: &str, id: &str, out_dir: &Path) -> Entry {
    let mut warnings = Vec::new();
    let kind = classify(value);
    let (subdir, yaml, actions) = match kind {
        Kind::Profile => {
            let (profile, mut w, actions) = map_profile(value, id);
            warnings.append(&mut w);
            let yaml = serde_yaml::to_string(&profile).unwrap_or_default();
            ("profiles", Some(yaml), actions)
        }
        Kind::Automation => {
            let (root, mut w, actions) = map_sequence(sequence_source(value));
            warnings.append(&mut w);
            let auto = Automation {
                id: id.to_string(),
                root,
            };
            let yaml = serde_yaml::to_string(&auto).unwrap_or_default();
            ("automations", Some(yaml), actions)
        }
        Kind::Unknown => {
            warnings.push(
                "unrecognised shape (no `triggers` and no `sequence`/action list) — migrate by hand".to_string(),
            );
            ("", None, 0)
        }
    };

    let written = yaml.and_then(|y| {
        let dir = out_dir.join(subdir);
        fs::create_dir_all(&dir).ok()?;
        fs::write(dir.join(format!("{id}.yaml")), y).ok()?;
        Some(format!("{subdir}/{id}.yaml"))
    });

    Entry {
        source: rel.to_string(),
        id: id.to_string(),
        kind,
        written,
        actions,
        warnings,
    }
}

/// Classify a parsed resource by its shape.
///
/// - has a `triggers` array → [`Kind::Profile`]
/// - a bare list, or an object with `sequence`/`actions` → [`Kind::Automation`]
/// - anything else → [`Kind::Unknown`]
pub fn classify(v: &Value) -> Kind {
    if v.get("triggers").is_some() {
        return Kind::Profile;
    }
    if v.is_array() || v.get("sequence").is_some() || v.get("actions").is_some() {
        return Kind::Automation;
    }
    Kind::Unknown
}

/// The sequence/action-list inside an automation-shaped value.
fn sequence_source(v: &Value) -> &Value {
    v.get("sequence").or_else(|| v.get("actions")).unwrap_or(v)
}

/// Map a CastMate profile → a Quicktys [`Profile`]. Returns the profile, review
/// warnings, and the number of actions mapped.
///
/// Assumed input shape (defensive — several spellings accepted):
/// ```yaml
/// name: My Profile
/// activation: ...            # optional Rhai-ish expr → left None, flagged
/// triggers:
///   - trigger: twitch.chat   # or `id:`
///     filter: ...            # optional → left None, flagged
///     sequence: [ ...actions... ]   # or `actions:`
/// ```
pub fn map_profile(v: &Value, id: &str) -> (Profile, Vec<String>, usize) {
    let mut warnings = Vec::new();
    let mut actions = 0usize;

    if v.get("activation").is_some() {
        warnings.push(
            "`activation` expression not ported — re-author it as a Rhai activation".to_string(),
        );
    }

    let mut triggers = Vec::new();
    if let Some(list) = v.get("triggers").and_then(Value::as_array) {
        for (i, t) in list.iter().enumerate() {
            let trigger = t
                .get("trigger")
                .or_else(|| t.get("id"))
                .and_then(Value::as_str);
            let Some(trigger) = trigger else {
                warnings.push(format!("trigger #{i} has no `trigger`/`id` — skipped"));
                continue;
            };
            if t.get("filter").is_some() || t.get("condition").is_some() {
                warnings.push(format!(
                    "trigger `{trigger}` has a filter/condition — re-author it as a Rhai filter"
                ));
            }
            let (sequence, mut w, a) = map_sequence(sequence_source(t));
            actions += a;
            warnings.append(&mut w);
            triggers.push(TriggerBinding {
                trigger: trigger.to_string(),
                sequence,
                filter: None,
            });
        }
    }

    let name = v
        .get("name")
        .and_then(Value::as_str)
        .map(str::to_string);

    let profile = Profile {
        id: id.to_string(),
        name,
        activation: None,
        triggers,
    };
    (profile, warnings, actions)
}

/// Map a CastMate sequence (a list of actions, or a single action) → a Quicktys
/// [`Node`] tree. Returns the root node, warnings, and the action count.
pub fn map_sequence(v: &Value) -> (Node, Vec<String>, usize) {
    let mut warnings = Vec::new();
    let mut body = Vec::new();
    let mut actions = 0usize;

    let items: Vec<&Value> = match v {
        Value::Array(a) => a.iter().collect(),
        Value::Null => Vec::new(),
        single => vec![single],
    };

    for (i, item) in items.into_iter().enumerate() {
        match map_action(item) {
            Some((node, mut w)) => {
                actions += 1;
                warnings.append(&mut w);
                body.push(node);
            }
            None => warnings.push(format!(
                "action #{i} could not be mapped (no `plugin`/`action` and not a `plugin.action` key) — migrate by hand"
            )),
        }
    }

    (Node::Block { body }, warnings, actions)
}

/// Map one CastMate action → a Quicktys [`ActionNode`], applying the template
/// codemod to its string config. Accepts two spellings:
/// - `{ plugin: obs, action: scene, config: {…} }` (config, or the leftover keys)
/// - `{ "obs.scene": {…} }` (a single `plugin.action` key)
pub fn map_action(v: &Value) -> Option<(Node, Vec<String>)> {
    let mut warnings = Vec::new();
    let obj = v.as_object()?;

    let (plugin, action, mut config) = if let (Some(p), Some(a)) = (
        obj.get("plugin").and_then(Value::as_str),
        obj.get("action").and_then(Value::as_str),
    ) {
        let config = obj.get("config").cloned().unwrap_or_else(|| {
            // No explicit `config` → treat the leftover keys as the config.
            let rest: serde_json::Map<String, Value> = obj
                .iter()
                .filter(|(k, _)| !matches!(k.as_str(), "plugin" | "action" | "config" | "bind" | "resultMapping"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Value::Object(rest)
        });
        (p.to_string(), a.to_string(), config)
    } else if obj.len() == 1 {
        // Shorthand `{ "plugin.action": config }`.
        let (key, cfg) = obj.iter().next()?;
        let (p, a) = key.split_once('.')?;
        (p.to_string(), a.to_string(), cfg.clone())
    } else {
        return None;
    };

    // resultMapping → bind (renamed in Quicktys).
    let bind = match obj.get("bind").or_else(|| obj.get("resultMapping")) {
        Some(Value::String(s)) => {
            if obj.get("resultMapping").is_some() && obj.get("bind").is_none() {
                warnings.push(format!("`resultMapping` renamed to `bind` (→ `{s}`)"));
            }
            Some(s.clone())
        }
        _ => None,
    };

    codemod_value(&mut config, &mut warnings);

    Some((
        Node::Action(ActionNode {
            plugin,
            action,
            config,
            bind,
        }),
        warnings,
    ))
}

/// Recursively apply the template codemod to every string in a config value.
fn codemod_value(v: &mut Value, warnings: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            let (new, mut w) = codemod_template(s);
            *s = new;
            warnings.append(&mut w);
        }
        Value::Array(a) => a.iter_mut().for_each(|x| codemod_value(x, warnings)),
        Value::Object(o) => o.values_mut().for_each(|x| codemod_value(x, warnings)),
        _ => {}
    }
}

/// Adapt a CastMate template string to Quicktys templating.
///
/// Both engines use `{{ … }}`, but Quicktys `{{ }}` is **Handlebars** (not JS)
/// and additionally offers `@path` for a *typed* single-variable reference.
///
/// - A string that is exactly one `{{ simple.path }}` becomes `@simple.path`
///   (preserves the typed value instead of stringifying it).
/// - Other `{{ … }}` regions are kept verbatim; any that hold an *expression*
///   (operators, calls, filters) get a warning to review by hand.
pub fn codemod_template(s: &str) -> (String, Vec<String>) {
    let mut warnings = Vec::new();
    if !s.contains("{{") {
        return (s.to_string(), warnings);
    }

    // Whole-string lone `{{ path }}` → typed `@path`.
    if let Some(inner) = lone_mustache(s.trim()) {
        if is_simple_path(inner.trim()) {
            return (format!("@{}", inner.trim()), warnings);
        }
    }

    for inner in mustache_regions(s) {
        if !is_simple_path(inner.trim()) {
            warnings.push(format!(
                "template `{{{{{inner}}}}}` is an expression — Quicktys `{{{{ }}}}` is Handlebars; move logic to a Rhai condition/value"
            ));
        }
    }
    (s.to_string(), warnings)
}

/// If `s` is exactly one `{{ … }}` region spanning the whole string, return its
/// inner text.
fn lone_mustache(s: &str) -> Option<&str> {
    let inner = s.strip_prefix("{{")?.strip_suffix("}}")?;
    if inner.contains("{{") || inner.contains("}}") {
        return None;
    }
    Some(inner)
}

/// Collect the inner text of every `{{ … }}` region in `s`.
fn mustache_regions(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find("}}") {
                out.push(&s[i + 2..i + 2 + end]);
                i = i + 2 + end + 2;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// A simple dotted variable path: `a`, `obs.scene`, `pear.title` — letters,
/// digits, `_`, `-`, `.`; no leading/trailing dot, no operators or spaces.
fn is_simple_path(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('.')
        && !s.ends_with('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

/// Derive a resource id: the `id` field if present, else the file stem.
fn resource_id(v: &Value, path: &Path) -> String {
    v.get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "migrated".to_string())
        })
}

/// Recursively collect `*.yaml`/`*.yml` files under `dir`.
fn collect_yaml(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.is_dir() {
            collect_yaml(&path, out)?;
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("yaml") | Some("yml")
        ) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lone_simple_path_becomes_typed_at_var() {
        let (out, w) = codemod_template("{{ pear.title }}");
        assert_eq!(out, "@pear.title");
        assert!(w.is_empty());
    }

    #[test]
    fn interpolated_path_is_kept_verbatim() {
        let (out, w) = codemod_template("Now playing: {{ pear.title }}!");
        assert_eq!(out, "Now playing: {{ pear.title }}!");
        assert!(w.is_empty(), "a plain path needs no review");
    }

    #[test]
    fn expression_region_is_flagged() {
        let (out, w) = codemod_template("{{ a + b }}");
        assert_eq!(out, "{{ a + b }}", "kept verbatim");
        assert_eq!(w.len(), 1, "flagged for manual review");
        assert!(w[0].contains("Handlebars"));
    }

    #[test]
    fn plain_text_is_untouched() {
        let (out, w) = codemod_template("no templates here");
        assert_eq!(out, "no templates here");
        assert!(w.is_empty());
    }

    #[test]
    fn action_shorthand_key_maps() {
        let v: Value = serde_json::json!({ "obs.scene": { "scene": "Main" } });
        let (node, w) = map_action(&v).expect("shorthand maps");
        assert!(w.is_empty());
        match node {
            Node::Action(a) => {
                assert_eq!(a.plugin, "obs");
                assert_eq!(a.action, "scene");
                assert_eq!(a.config, serde_json::json!({ "scene": "Main" }));
            }
            other => panic!("expected Action, got {other:?}"),
        }
    }

    #[test]
    fn action_explicit_fields_and_result_mapping() {
        let v: Value = serde_json::json!({
            "plugin": "twitch",
            "action": "say",
            "message": "{{ user }}",
            "resultMapping": "msg",
        });
        let (node, w) = map_action(&v).expect("explicit maps");
        // resultMapping → bind is flagged.
        assert!(w.iter().any(|x| x.contains("resultMapping")));
        match node {
            Node::Action(a) => {
                assert_eq!(a.plugin, "twitch");
                assert_eq!(a.bind.as_deref(), Some("msg"));
                // `{{ user }}` is a lone simple path → typed `@user`.
                assert_eq!(a.config, serde_json::json!({ "message": "@user" }));
            }
            other => panic!("expected Action, got {other:?}"),
        }
    }

    #[test]
    fn classify_recognises_shapes() {
        assert_eq!(
            classify(&serde_json::json!({ "triggers": [] })),
            Kind::Profile
        );
        assert_eq!(
            classify(&serde_json::json!({ "sequence": [] })),
            Kind::Automation
        );
        assert_eq!(classify(&serde_json::json!([])), Kind::Automation);
        assert_eq!(classify(&serde_json::json!({ "foo": 1 })), Kind::Unknown);
    }

    #[test]
    fn profile_maps_triggers_and_flags_activation() {
        let v: Value = serde_json::json!({
            "name": "My Profile",
            "activation": "twitch.live",
            "triggers": [
                { "trigger": "twitch.chat", "sequence": [ { "obs.scene": { "scene": "BRB" } } ] },
                { "id": "twitch.follow", "filter": "x > 1", "actions": [ { "notify.toast": { "text": "hi" } } ] },
            ],
        });
        let (profile, warnings, actions) = map_profile(&v, "my-profile");
        assert_eq!(profile.id, "my-profile");
        assert_eq!(profile.name.as_deref(), Some("My Profile"));
        assert_eq!(profile.triggers.len(), 2);
        assert_eq!(profile.triggers[0].trigger, "twitch.chat");
        assert_eq!(profile.triggers[1].trigger, "twitch.follow");
        assert_eq!(actions, 2);
        assert!(warnings.iter().any(|w| w.contains("activation")));
        assert!(warnings.iter().any(|w| w.contains("filter")));
    }
}

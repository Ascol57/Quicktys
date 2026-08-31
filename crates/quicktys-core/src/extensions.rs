//! Extension registry — decoded manifests of installed plugins and modules,
//! plus dependency resolution and certification gating (ARCHITECTURE §6, §4).
//!
//! Manifests are the JSON a guest returns from `init()` over the WASM ABI
//! (`quicktys-host` decodes them in M2/M4). This crate owns the in-memory model:
//! index actions/services, resolve plugin → required modules, and decide whether
//! a module may be granted a system capability (only if certified AND it
//! declared the capability).
//!
//! Semver here is intentionally minimal (caret-major / exact); a full semver
//! implementation lands with the real dependency resolver.

use quicktys_schema::Schema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Manifests (decoded from the guest `init()` JSON)
// ---------------------------------------------------------------------------

/// A plugin's declared contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default)]
    pub actions: Vec<ActionDecl>,
    #[serde(default)]
    pub triggers: Vec<TriggerDecl>,
    #[serde(default)]
    pub state: Vec<StateDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Schema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Schema>,
    /// Modules this plugin needs, by id + version requirement.
    #[serde(default, rename = "requires")]
    pub required_modules: Vec<ModuleRequirement>,
    /// UI contributions injected into named interface slots.
    #[serde(default)]
    pub ui: Vec<UiDecl>,
}

/// A plugin's contribution to a named UI slot (button or custom fragment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiDecl {
    pub slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDecl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<Schema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerDecl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateDecl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

/// A dependency on a module: `module@version-req` (e.g. `obs` `^1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleRequirement {
    pub module: String,
    #[serde(default = "any_version")]
    pub version: String,
}

fn any_version() -> String {
    "*".to_string()
}

/// A module's declared contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub services: Vec<ServiceDecl>,
    #[serde(default)]
    pub events: Vec<EventDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings: Option<Schema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Schema>,
    /// System capabilities the module requests.
    #[serde(default)]
    pub capabilities: Vec<Capability>,
    /// A full app **view/tab** the module contributes (its own page, an HTML
    /// `entry` shipped in the bundle, served over `moduleui://<id>/…`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewDecl>,
}

/// A navigation tab + page a module contributes to the app shell. The app adds
/// it to the sidebar and renders `entry` (an HTML file in the module bundle) in
/// a sandboxed frame, bridged so the page can call **its own** module services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewDecl {
    /// Sidebar icon (e.g. `pi pi-desktop`).
    pub icon: String,
    /// i18n key for the tab label (falls back to the module name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// HTML file in the module bundle to render (e.g. `index.html`).
    pub entry: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDecl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<Schema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventDecl {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Schema>,
}

/// The system capabilities a module may request (granted only when certified).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Net,
    Input,
    Audio,
    Hid,
    Files,
    Process,
    Timers,
}

impl Capability {
    /// The lowercase name used in manifests and the consent store.
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::Net => "net",
            Capability::Input => "input",
            Capability::Audio => "audio",
            Capability::Hid => "hid",
            Capability::Files => "files",
            Capability::Process => "process",
            Capability::Timers => "timers",
        }
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

struct ModuleEntry {
    manifest: ModuleManifest,
    /// Signed by the certification authority. Required to grant system caps.
    certified: bool,
}

/// In-memory registry of installed plugins and modules.
#[derive(Default)]
pub struct ExtensionRegistry {
    plugins: BTreeMap<String, PluginManifest>,
    modules: BTreeMap<String, ModuleEntry>,
}

impl ExtensionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_plugin(&mut self, manifest: PluginManifest) {
        self.plugins.insert(manifest.id.clone(), manifest);
    }

    /// Register a module, recording whether it is certified (signature verified
    /// upstream by the host).
    pub fn register_module(&mut self, manifest: ModuleManifest, certified: bool) {
        self.modules.insert(
            manifest.id.clone(),
            ModuleEntry {
                manifest,
                certified,
            },
        );
    }

    pub fn get_plugin(&self, id: &str) -> Option<&PluginManifest> {
        self.plugins.get(id)
    }

    pub fn get_module(&self, id: &str) -> Option<&ModuleManifest> {
        self.modules.get(id).map(|e| &e.manifest)
    }

    pub fn plugins(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins.values()
    }

    pub fn modules(&self) -> impl Iterator<Item = &ModuleManifest> {
        self.modules.values().map(|e| &e.manifest)
    }

    /// Look up one of a plugin's action declarations.
    pub fn resolve_action(&self, plugin_id: &str, action_id: &str) -> Option<&ActionDecl> {
        self.plugins
            .get(plugin_id)?
            .actions
            .iter()
            .find(|a| a.id == action_id)
    }

    /// The required modules of `plugin_id` that are not satisfied — either not
    /// registered, or registered at an incompatible version.
    pub fn missing_dependencies(&self, plugin_id: &str) -> Vec<ModuleRequirement> {
        let Some(plugin) = self.plugins.get(plugin_id) else {
            return Vec::new();
        };
        plugin
            .required_modules
            .iter()
            .filter(|req| match self.modules.get(&req.module) {
                Some(entry) => !version_satisfies(&req.version, &entry.manifest.version),
                None => true,
            })
            .cloned()
            .collect()
    }

    /// Whether every required module of `plugin_id` is present and compatible.
    pub fn is_satisfied(&self, plugin_id: &str) -> bool {
        self.missing_dependencies(plugin_id).is_empty()
    }

    /// Whether `plugin_id`'s specific dependency on `module_id` is satisfied: the
    /// plugin **declared** it AND the loaded module's version meets the semver
    /// requirement. Gates `modules.call` at runtime (an incompatible-version
    /// module is refused, not just a missing one).
    pub fn dependency_satisfied(&self, plugin_id: &str, module_id: &str) -> bool {
        let Some(plugin) = self.plugins.get(plugin_id) else {
            return false;
        };
        let Some(req) = plugin.required_modules.iter().find(|r| r.module == module_id) else {
            return false;
        };
        self.modules
            .get(module_id)
            .is_some_and(|entry| version_satisfies(&req.version, &entry.manifest.version))
    }

    /// Whether `plugin_id` declared a dependency on `module_id` (regardless of
    /// version) — used to distinguish "not allowed" from "incompatible".
    pub fn declares_dependency(&self, plugin_id: &str, module_id: &str) -> bool {
        self.plugins
            .get(plugin_id)
            .is_some_and(|p| p.required_modules.iter().any(|r| r.module == module_id))
    }

    /// Whether a module may be granted `cap`: it must be **certified** and have
    /// **declared** the capability in its manifest. This is the trust boundary
    /// that lets third-party code touch the network/keyboard/etc. safely. User
    /// **consent** (M6.7) is enforced separately, at the host cap dispatch.
    pub fn may_grant(&self, module_id: &str, cap: Capability) -> bool {
        self.modules
            .get(module_id)
            .is_some_and(|e| e.certified && e.manifest.capabilities.contains(&cap))
    }

    /// The system capabilities a certified module declared (for the consent UI).
    pub fn module_capabilities(&self, module_id: &str) -> Vec<Capability> {
        self.modules
            .get(module_id)
            .filter(|e| e.certified)
            .map(|e| e.manifest.capabilities.clone())
            .unwrap_or_default()
    }

    /// True if a registered module is required by no registered plugin — a
    /// candidate for removal (reference counting, ARCHITECTURE §4.4).
    pub fn is_orphan_module(&self, module_id: &str) -> bool {
        self.modules.contains_key(module_id)
            && !self.plugins.values().any(|p| {
                p.required_modules
                    .iter()
                    .any(|r| r.module == module_id)
            })
    }
}

/// Whether a module `version` satisfies a dependency `req` (real semver).
///
/// `req` is a semver requirement (`*`, `^1`, `~1.4`, `>=1.2, <2`, `1.2.3`); empty
/// and `*` accept anything. `version` is parsed leniently (a bare `1` / `1.2` is
/// coerced to a full triple), so short module versions still match. An
/// unparseable requirement rejects (fail-closed).
pub fn version_satisfies(req: &str, version: &str) -> bool {
    let req = req.trim();
    if req.is_empty() || req == "*" {
        return true;
    }
    let Some(version) = parse_lenient(version) else {
        return false;
    };
    match semver::VersionReq::parse(req) {
        Ok(reqv) => reqv.matches(&version),
        Err(_) => false,
    }
}

/// Parse a version leniently: exact semver first, else pad `1`/`1.2` to `1.0.0`/
/// `1.2.0` (a leading `v` is tolerated).
fn parse_lenient(v: &str) -> Option<semver::Version> {
    let v = v.trim().trim_start_matches('v');
    if let Ok(ver) = semver::Version::parse(v) {
        return Some(ver);
    }
    let mut parts: Vec<&str> = v.split('.').take(3).collect();
    while parts.len() < 3 {
        parts.push("0");
    }
    semver::Version::parse(&parts.join(".")).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn plugin_requiring(id: &str, module: &str, req: &str) -> PluginManifest {
        serde_json::from_value(json!({
            "id": id,
            "name": id,
            "version": "1.0.0",
            "actions": [
                { "id": "go", "config": { "type": "Object", "properties": {
                    "secs": { "type": "Duration" }
                } } }
            ],
            "requires": [ { "module": module, "version": req } ]
        }))
        .unwrap()
    }

    fn module(id: &str, version: &str, caps: Value) -> ModuleManifest {
        serde_json::from_value(json!({
            "id": id, "name": id, "version": version, "capabilities": caps
        }))
        .unwrap()
    }

    use serde_json::Value;

    #[test]
    fn missing_dependency_until_compatible_module_present() {
        let mut reg = ExtensionRegistry::new();
        reg.register_plugin(plugin_requiring("obsctl", "obs", "^1"));
        // No module yet → missing.
        assert_eq!(reg.missing_dependencies("obsctl").len(), 1);
        assert!(!reg.is_satisfied("obsctl"));

        // Incompatible major → still missing.
        reg.register_module(module("obs", "2.0.0", json!([])), true);
        assert!(!reg.is_satisfied("obsctl"));

        // Compatible major → satisfied.
        reg.register_module(module("obs", "1.4.2", json!([])), true);
        assert!(reg.is_satisfied("obsctl"));
    }

    #[test]
    fn capability_grant_requires_certification_and_declaration() {
        let mut reg = ExtensionRegistry::new();
        reg.register_module(module("obs", "1.0.0", json!(["net"])), true);
        reg.register_module(module("evil", "1.0.0", json!(["net"])), false);
        reg.register_module(module("audio", "1.0.0", json!(["audio"])), true);

        // Certified + declared.
        assert!(reg.may_grant("obs", Capability::Net));
        // Declared but NOT certified.
        assert!(!reg.may_grant("evil", Capability::Net));
        // Certified but did not declare this cap.
        assert!(!reg.may_grant("audio", Capability::Net));
        // Unknown module.
        assert!(!reg.may_grant("ghost", Capability::Net));
    }

    #[test]
    fn orphan_module_detection() {
        let mut reg = ExtensionRegistry::new();
        reg.register_module(module("obs", "1.0.0", json!([])), true);
        assert!(reg.is_orphan_module("obs")); // nobody requires it

        reg.register_plugin(plugin_requiring("obsctl", "obs", "^1"));
        assert!(!reg.is_orphan_module("obs")); // now required
    }

    #[test]
    fn resolve_action_and_decode_schema() {
        let mut reg = ExtensionRegistry::new();
        reg.register_plugin(plugin_requiring("obsctl", "obs", "^1"));
        let action = reg.resolve_action("obsctl", "go").unwrap();
        assert!(matches!(action.config, Some(Schema::Object { .. })));
        assert!(reg.resolve_action("obsctl", "missing").is_none());
    }

    #[test]
    fn version_satisfies_rules() {
        assert!(version_satisfies("*", "9.9.9"));
        assert!(version_satisfies("^1", "1.4.0"));
        assert!(!version_satisfies("^1", "2.0.0"));
        assert!(version_satisfies("1.2.3", "1.2.3"));
        assert!(version_satisfies("1", "1.9.9"));
        assert!(!version_satisfies("^x", "1.0.0"));
        // Real semver ranges (beyond the old major-only check).
        assert!(version_satisfies(">=1.2, <2", "1.5.0"));
        assert!(!version_satisfies(">=1.2, <2", "2.0.0"));
        assert!(version_satisfies("~1.4", "1.4.9"));
        assert!(!version_satisfies("~1.4", "1.5.0"));
        // `^0` spans the whole 0.x line; `^0.1` is pinned to 0.1.x (stricter).
        assert!(version_satisfies("^0", "0.1.0"));
        assert!(!version_satisfies("^0.1", "0.2.0"));
        // Lenient version parsing (bare major / leading v).
        assert!(version_satisfies("^1.2", "v1.3"));
    }

    #[test]
    fn dependency_satisfied_checks_declaration_and_version() {
        let mut reg = ExtensionRegistry::new();
        reg.register_plugin(plugin_requiring("obsctl", "obs", "^1"));
        // Declared but no module → not satisfied, but declared.
        assert!(!reg.dependency_satisfied("obsctl", "obs"));
        assert!(reg.declares_dependency("obsctl", "obs"));
        // Incompatible version → declared but not satisfied.
        reg.register_module(module("obs", "2.0.0", json!([])), true);
        assert!(!reg.dependency_satisfied("obsctl", "obs"));
        // Compatible version → satisfied.
        reg.register_module(module("obs", "1.9.0", json!([])), true);
        assert!(reg.dependency_satisfied("obsctl", "obs"));
        // Undeclared module → neither.
        assert!(!reg.declares_dependency("obsctl", "ghost"));
        assert!(!reg.dependency_satisfied("obsctl", "ghost"));
    }
}

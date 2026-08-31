//! Decentralised i18n registry (ARCHITECTURE §10).
//!
//! Every "thing" — the core, each module, each plugin, each widget — ships its
//! own `lang/` folder of per-locale JSON (`en.json`, `fr.json`, ...). The host
//! discovers each folder and **namespaces** its bundles by id:
//!
//! - core → `core.*`
//! - module `<id>` → `module.<id>.*`
//! - plugin `<id>` → `plugin.<id>.*`
//! - widget `<id>` → `widget.<id>.*`
//!
//! Manifests emit **keys**, never translated strings; the merged per-locale
//! bundle is what gets pushed to the WebView (resolved there by `vue-i18n`).
//! `en` is the reference locale: a missing key falls back to it.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;
use thiserror::Error;

/// Which "thing" a locale bundle belongs to. Determines its key prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Namespace {
    Core,
    Plugin(String),
    Module(String),
    Widget(String),
}

impl Namespace {
    /// The key-path segments this namespace nests under.
    fn segments(&self) -> Vec<&str> {
        match self {
            Namespace::Core => vec!["core"],
            Namespace::Plugin(id) => vec!["plugin", id],
            Namespace::Module(id) => vec!["module", id],
            Namespace::Widget(id) => vec!["widget", id],
        }
    }
}

/// An i18n discovery/registration failure.
#[derive(Debug, Error)]
pub enum I18nError {
    #[error("io error at {path}: {source}")]
    Io {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid locale JSON at {path}: {source}")]
    Json {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

const FALLBACK_LOCALE: &str = "en";

/// Holds every discovered locale bundle, merged per locale and namespaced by id.
#[derive(Debug, Default)]
pub struct LocaleRegistry {
    /// locale → merged nested object (e.g. `{ plugin: { time: { ... } } }`).
    locales: BTreeMap<String, Map<String, Value>>,
}

impl LocaleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one extension's data for one locale, nested under its namespace.
    /// Replaces that namespace's subtree, leaving siblings untouched.
    pub fn register(&mut self, namespace: &Namespace, locale: &str, data: Map<String, Value>) {
        let locale_map = self.locales.entry(locale.to_string()).or_default();
        insert_nested(locale_map, &namespace.segments(), Value::Object(data));
    }

    /// Discover a `lang/` folder: load every `<locale>.json` and register it
    /// under `namespace`. A missing folder is a no-op (not an error).
    pub fn load_lang_dir(&mut self, namespace: &Namespace, dir: &Path) -> Result<(), I18nError> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(I18nError::Io {
                    path: dir.to_path_buf(),
                    source,
                })
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|source| I18nError::Io {
                    path: dir.to_path_buf(),
                    source,
                })?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(locale) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).map_err(|source| I18nError::Io {
                path: path.clone(),
                source,
            })?;
            let data: Map<String, Value> =
                serde_json::from_str(&text).map_err(|source| I18nError::Json {
                    path: path.clone(),
                    source,
                })?;
            self.register(namespace, locale, data);
        }
        Ok(())
    }

    /// The full merged bundle for a locale, ready to push to the UI.
    pub fn bundle(&self, locale: &str) -> Option<&Map<String, Value>> {
        self.locales.get(locale)
    }

    /// Every locale that has at least one bundle.
    pub fn locales(&self) -> impl Iterator<Item = &str> {
        self.locales.keys().map(String::as_str)
    }

    /// Resolve a dotted key (e.g. `plugin.time.actions.delay.name`) in `locale`,
    /// falling back to the `en` reference locale.
    pub fn translate(&self, locale: &str, key: &str) -> Option<String> {
        self.lookup(locale, key).or_else(|| {
            (locale != FALLBACK_LOCALE)
                .then(|| self.lookup(FALLBACK_LOCALE, key))
                .flatten()
        })
    }

    fn lookup(&self, locale: &str, key: &str) -> Option<String> {
        let root = self.locales.get(locale)?;
        get_nested(root, key)?.as_str().map(str::to_string)
    }
}

/// Insert `data` at `path` within `root`, creating intermediate objects and
/// only replacing the value at the final segment.
fn insert_nested(root: &mut Map<String, Value>, path: &[&str], data: Value) {
    match path {
        [] => {}
        [last] => {
            root.insert((*last).to_string(), data);
        }
        [head, rest @ ..] => {
            let entry = root
                .entry((*head).to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            if !entry.is_object() {
                *entry = Value::Object(Map::new());
            }
            insert_nested(entry.as_object_mut().unwrap(), rest, data);
        }
    }
}

/// Navigate a dotted key through nested objects.
fn get_nested<'a>(root: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    let mut parts = key.split('.');
    let mut current = root.get(parts.next()?)?;
    for part in parts {
        current = current.as_object()?.get(part)?;
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn namespaces_by_id_and_merges_siblings() {
        let mut reg = LocaleRegistry::new();
        reg.register(&Namespace::Core, "en", obj(json!({ "app": { "title": "Quicktys" } })));
        reg.register(
            &Namespace::Plugin("time".into()),
            "en",
            obj(json!({ "actions": { "delay": { "name": "Delay" } } })),
        );
        reg.register(
            &Namespace::Plugin("obs".into()),
            "en",
            obj(json!({ "name": "OBS" })),
        );

        assert_eq!(
            reg.translate("en", "plugin.time.actions.delay.name"),
            Some("Delay".into())
        );
        // Sibling plugin under the same `plugin` node is preserved.
        assert_eq!(reg.translate("en", "plugin.obs.name"), Some("OBS".into()));
        assert_eq!(reg.translate("en", "core.app.title"), Some("Quicktys".into()));
    }

    #[test]
    fn falls_back_to_english() {
        let mut reg = LocaleRegistry::new();
        reg.register(
            &Namespace::Plugin("time".into()),
            "en",
            obj(json!({ "actions": { "delay": { "name": "Delay" } } })),
        );
        reg.register(
            &Namespace::Plugin("time".into()),
            "fr",
            obj(json!({ "actions": { "delay": {} } })), // missing `name`
        );
        // fr lacks the key → falls back to en.
        assert_eq!(
            reg.translate("fr", "plugin.time.actions.delay.name"),
            Some("Delay".into())
        );
        // Unknown key returns None even after fallback.
        assert_eq!(reg.translate("fr", "plugin.time.nope"), None);
    }

    #[test]
    fn discovers_a_lang_dir() {
        let tmp = TempDir::new();
        std::fs::write(
            tmp.path().join("en.json"),
            r#"{ "actions": { "delay": { "name": "Delay" } } }"#,
        )
        .unwrap();
        std::fs::write(
            tmp.path().join("fr.json"),
            r#"{ "actions": { "delay": { "name": "Délai" } } }"#,
        )
        .unwrap();
        std::fs::write(tmp.path().join("README.md"), "ignored").unwrap();

        let mut reg = LocaleRegistry::new();
        reg.load_lang_dir(&Namespace::Plugin("time".into()), tmp.path())
            .unwrap();

        assert_eq!(
            reg.translate("fr", "plugin.time.actions.delay.name"),
            Some("Délai".into())
        );
        let mut locales: Vec<&str> = reg.locales().collect();
        locales.sort();
        assert_eq!(locales, ["en", "fr"]);
    }

    #[test]
    fn missing_dir_is_not_an_error() {
        let mut reg = LocaleRegistry::new();
        let res = reg.load_lang_dir(&Namespace::Core, Path::new("/no/such/dir/xyz"));
        assert!(res.is_ok());
        assert_eq!(reg.locales().count(), 0);
    }
}

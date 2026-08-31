//! Resource registry — typed, YAML-backed collections of domain objects.
//!
//! A *resource* is a persisted, id-addressed object (Profiles, Automations,
//! ActionQueues, Accounts, ... — ARCHITECTURE §6). [`ResourceStore`] keeps a
//! resource type in memory and mirrors it to a directory of `<id>.yaml` files,
//! one per item. UI sync (push add/update/remove to the WebView) layers on top
//! later via `quicktys-ipc`.

use crate::persist::{load_yaml, save_yaml, PersistError};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A persisted, id-addressed object.
pub trait Resource: Serialize + DeserializeOwned {
    /// Stable unique id. Also the YAML file stem (`<id>.yaml`).
    fn id(&self) -> &str;
}

/// An error from a resource operation.
#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error(transparent)]
    Persist(#[from] PersistError),
    /// An id that can't be used as a file name (path separators, empty, ...).
    #[error("invalid resource id: {0:?}")]
    InvalidId(String),
}

/// In-memory registry of one resource type, backed by a directory of YAML files.
pub struct ResourceStore<T> {
    dir: PathBuf,
    items: BTreeMap<String, T>,
}

impl<T: Resource> ResourceStore<T> {
    /// Create an (empty) store over `dir`. Call [`load`](Self::load) to populate
    /// it from disk.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        ResourceStore {
            dir: dir.into(),
            items: BTreeMap::new(),
        }
    }

    /// Load every `<id>.yaml` in the directory into memory, replacing the
    /// current contents. A missing directory loads as empty.
    pub fn load(&mut self) -> Result<(), ResourceError> {
        self.items.clear();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(source) => {
                return Err(PersistError::Io {
                    path: self.dir.clone(),
                    source,
                }
                .into())
            }
        };
        for entry in entries {
            let path = entry
                .map_err(|source| PersistError::Io {
                    path: self.dir.clone(),
                    source,
                })?
                .path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let item: T = load_yaml(&path)?;
            self.items.insert(item.id().to_string(), item);
        }
        Ok(())
    }

    /// Insert or replace a resource, persisting it to disk.
    pub fn upsert(&mut self, item: T) -> Result<(), ResourceError> {
        let id = item.id().to_string();
        let path = self.path_for(&id)?;
        save_yaml(&path, &item)?;
        self.items.insert(id, item);
        Ok(())
    }

    /// Remove a resource (and delete its file). Returns the removed item, if any.
    pub fn remove(&mut self, id: &str) -> Result<Option<T>, ResourceError> {
        let removed = self.items.remove(id);
        if removed.is_some() {
            let path = self.path_for(id)?;
            match std::fs::remove_file(&path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(PersistError::Io { path, source }.into()),
            }
        }
        Ok(removed)
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.items.get(id)
    }

    /// Iterate items in id order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.values()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The YAML path for an id, rejecting ids that aren't safe file stems.
    fn path_for(&self, id: &str) -> Result<PathBuf, ResourceError> {
        if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
            return Err(ResourceError::InvalidId(id.to_string()));
        }
        let mut path = self.dir.join(id);
        path.set_extension("yaml");
        // Guard against any traversal that survived the checks above.
        if path.parent() != Some(self.dir.as_path()) {
            return Err(ResourceError::InvalidId(id.to_string()));
        }
        Ok(path)
    }
}

impl<T> ResourceStore<T> {
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde::Deserialize;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Profile {
        id: String,
        title: String,
    }
    impl Resource for Profile {
        fn id(&self) -> &str {
            &self.id
        }
    }

    fn profile(id: &str, title: &str) -> Profile {
        Profile {
            id: id.into(),
            title: title.into(),
        }
    }

    #[test]
    fn upsert_persists_and_reload_recovers() {
        let tmp = TempDir::new();
        let mut store = ResourceStore::<Profile>::new(tmp.path());
        store.upsert(profile("main", "Main")).unwrap();
        store.upsert(profile("brb", "Be Right Back")).unwrap();
        assert_eq!(store.len(), 2);

        // A fresh store loads the same items from disk.
        let mut reloaded = ResourceStore::<Profile>::new(tmp.path());
        reloaded.load().unwrap();
        assert_eq!(reloaded.len(), 2);
        assert_eq!(reloaded.get("main"), Some(&profile("main", "Main")));
    }

    #[test]
    fn upsert_replaces_existing() {
        let tmp = TempDir::new();
        let mut store = ResourceStore::<Profile>::new(tmp.path());
        store.upsert(profile("main", "Main")).unwrap();
        store.upsert(profile("main", "Renamed")).unwrap();
        assert_eq!(store.len(), 1);
        assert_eq!(store.get("main").unwrap().title, "Renamed");
    }

    #[test]
    fn remove_deletes_file() {
        let tmp = TempDir::new();
        let mut store = ResourceStore::<Profile>::new(tmp.path());
        store.upsert(profile("main", "Main")).unwrap();
        let removed = store.remove("main").unwrap();
        assert_eq!(removed, Some(profile("main", "Main")));
        assert!(store.is_empty());
        assert!(!tmp.path().join("main.yaml").exists());
    }

    #[test]
    fn iter_is_id_ordered() {
        let tmp = TempDir::new();
        let mut store = ResourceStore::<Profile>::new(tmp.path());
        store.upsert(profile("c", "C")).unwrap();
        store.upsert(profile("a", "A")).unwrap();
        store.upsert(profile("b", "B")).unwrap();
        let ids: Vec<&str> = store.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "c"]);
    }

    #[test]
    fn rejects_unsafe_ids() {
        let tmp = TempDir::new();
        let mut store = ResourceStore::<Profile>::new(tmp.path());
        let err = store.upsert(profile("../escape", "x")).unwrap_err();
        assert!(matches!(err, ResourceError::InvalidId(_)));
    }
}

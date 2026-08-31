//! YAML persistence helpers and the user-data directory layout (ARCHITECTURE
//! §9).
//!
//! Files are the source of truth on disk; the in-memory registries
//! ([`crate::resources`]) load from and save to here. Writes go through a
//! temp-file + rename so a crash mid-write can't leave a half-written YAML file.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// A persistence failure.
#[derive(Debug, Error)]
pub enum PersistError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("YAML error at {path}: {source}")]
    Yaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}

fn io_err(path: &Path) -> impl Fn(std::io::Error) -> PersistError + '_ {
    move |source| PersistError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Load and deserialize a YAML file.
pub fn load_yaml<T: DeserializeOwned>(path: &Path) -> Result<T, PersistError> {
    let text = std::fs::read_to_string(path).map_err(io_err(path))?;
    serde_yaml::from_str(&text).map_err(|source| PersistError::Yaml {
        path: path.to_path_buf(),
        source,
    })
}

/// Serialize and write a value as YAML, creating parent directories. The write
/// is atomic: it lands via a temp file + rename in the same directory.
pub fn save_yaml<T: Serialize>(path: &Path, value: &T) -> Result<(), PersistError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(io_err(parent))?;
    }
    let text = serde_yaml::to_string(value).map_err(|source| PersistError::Yaml {
        path: path.to_path_buf(),
        source,
    })?;
    let tmp = path.with_extension("yaml.tmp");
    std::fs::write(&tmp, text).map_err(io_err(&tmp))?;
    std::fs::rename(&tmp, path).map_err(io_err(path))?;
    Ok(())
}

/// The user-data directory and its standard sub-folders (ARCHITECTURE §9).
#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

/// The standard sub-directories under the data root.
const SUBDIRS: &[&str] = &[
    "settings",
    "profiles",
    "automations",
    "secrets",
    "queues",
    "accounts",
    "state",
    "storage",
    "logs",
    "installed",
    "registry-cache",
];

impl DataDir {
    /// Use `root` directly as the data directory (already pointing at the
    /// `quicktys/` folder).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        DataDir { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create the root and every standard sub-directory if missing.
    pub fn ensure_layout(&self) -> Result<(), PersistError> {
        std::fs::create_dir_all(&self.root).map_err(io_err(&self.root))?;
        for sub in SUBDIRS {
            let dir = self.root.join(sub);
            std::fs::create_dir_all(&dir).map_err(io_err(&dir))?;
        }
        Ok(())
    }

    pub fn profiles(&self) -> PathBuf {
        self.root.join("profiles")
    }
    pub fn automations(&self) -> PathBuf {
        self.root.join("automations")
    }
    pub fn queues(&self) -> PathBuf {
        self.root.join("queues")
    }
    pub fn accounts(&self) -> PathBuf {
        self.root.join("accounts")
    }
    pub fn settings(&self) -> PathBuf {
        self.root.join("settings")
    }
    pub fn installed(&self) -> PathBuf {
        self.root.join("installed")
    }
    /// Cache of the marketplace index / CRL / downloaded blobs (M6.6).
    pub fn registry_cache(&self) -> PathBuf {
        self.root.join("registry-cache")
    }
    /// The install lockfile: resolved versions, hashes, ref counts (M6.2).
    pub fn lockfile(&self) -> PathBuf {
        self.root.join("installed.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Sample {
        name: String,
        count: u32,
    }

    #[test]
    fn yaml_roundtrips_through_disk() {
        let tmp = TempDir::new();
        let path = tmp.path().join("nested/sample.yaml");
        let value = Sample {
            name: "ada".into(),
            count: 3,
        };
        save_yaml(&path, &value).unwrap();
        assert!(path.exists());
        let loaded: Sample = load_yaml(&path).unwrap();
        assert_eq!(loaded, value);
    }

    #[test]
    fn ensure_layout_creates_subdirs() {
        let tmp = TempDir::new();
        let data = DataDir::new(tmp.path().join("quicktys"));
        data.ensure_layout().unwrap();
        assert!(data.profiles().is_dir());
        assert!(data.automations().is_dir());
        assert!(data.installed().is_dir());
    }
}

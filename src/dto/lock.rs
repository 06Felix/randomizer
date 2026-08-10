use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BuildSystem, FieldPresence, java::atomic_write};

const LOCK_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JavaContractEntry {
    pub contract: String,
    pub root_type: String,
    pub build_system: BuildSystem,
    pub field_presence: FieldPresence,
    pub input_hash: String,
    pub exporter_version: String,
    pub visited_classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JavaContractLock {
    pub version: u32,
    #[serde(default)]
    pub contracts: BTreeMap<String, JavaContractEntry>,
}

impl Default for JavaContractLock {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            contracts: BTreeMap::new(),
        }
    }
}

impl JavaContractLock {
    pub fn load(path: &Path) -> Result<Self, JavaContractLockError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).map_err(|source| JavaContractLockError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let lock: Self =
            serde_json::from_slice(&bytes).map_err(|source| JavaContractLockError::Parse {
                path: path.to_path_buf(),
                source,
            })?;
        if lock.version != LOCK_VERSION {
            return Err(JavaContractLockError::Version {
                provided: lock.version,
                supported: LOCK_VERSION,
            });
        }
        Ok(lock)
    }

    pub fn save(&self, path: &Path) -> Result<(), JavaContractLockError> {
        let mut encoded = serde_json::to_vec_pretty(self).map_err(JavaContractLockError::Encode)?;
        encoded.push(b'\n');
        atomic_write(path, &encoded).map_err(|source| JavaContractLockError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[derive(Debug, Error)]
pub enum JavaContractLockError {
    #[error("failed to read Java contract lock {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to parse Java contract lock {path}: {source}")]
    Parse {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported Java contract lock version {provided}; supported version is {supported}")]
    Version { provided: u32, supported: u32 },
    #[error("failed to encode Java contract lock: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("failed to write Java contract lock {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_round_trips() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("java.lock.json");
        let mut lock = JavaContractLock::default();
        lock.contracts.insert(
            "task-response".into(),
            JavaContractEntry {
                contract: ".randomizer/contracts/task-response.json".into(),
                root_type: "example.Envelope<example.Task>".into(),
                build_system: BuildSystem::Maven,
                field_presence: FieldPresence::All,
                input_hash: "sha256:abc".into(),
                exporter_version: "1.0.0".into(),
                visited_classes: vec!["example.Task".into()],
            },
        );

        lock.save(&path).unwrap();
        assert_eq!(JavaContractLock::load(&path).unwrap(), lock);
    }
}

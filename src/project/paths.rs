use std::{
    env,
    path::{Path, PathBuf},
};

use super::ManifestError;

#[derive(Debug, Clone)]
pub struct ProjectPaths {
    pub root: PathBuf,
    pub randomizer_dir: PathBuf,
    pub manifest: PathBuf,
    pub contracts: PathBuf,
    pub contracts_lock: PathBuf,
    pub contracts_transaction: PathBuf,
    pub contracts_transaction_lock: PathBuf,
    pub fixtures: PathBuf,
    pub runtime: PathBuf,
}

impl ProjectPaths {
    pub fn discover(start: Option<&Path>) -> Result<Self, ManifestError> {
        let start = match start {
            Some(path) => path.to_path_buf(),
            None => env::current_dir().map_err(ManifestError::CurrentDirectory)?,
        };
        let mut current = start
            .canonicalize()
            .map_err(|source| ManifestError::ProjectRoot {
                path: start.clone(),
                source,
            })?;

        loop {
            if current.join(".randomizer/randomizer.yaml").is_file() {
                return Ok(Self::at_root(current));
            }
            if !current.pop() {
                return Err(ManifestError::NotInitialized { start });
            }
        }
    }

    pub fn for_init(root: &Path) -> Result<Self, ManifestError> {
        let root = root
            .canonicalize()
            .map_err(|source| ManifestError::ProjectRoot {
                path: root.to_path_buf(),
                source,
            })?;
        Ok(Self::at_root(root))
    }

    fn at_root(root: PathBuf) -> Self {
        let randomizer_dir = root.join(".randomizer");
        Self {
            manifest: randomizer_dir.join("randomizer.yaml"),
            contracts: randomizer_dir.join("contracts"),
            contracts_lock: randomizer_dir.join("contracts.lock.json"),
            contracts_transaction: randomizer_dir.join("contracts.transaction.json"),
            contracts_transaction_lock: randomizer_dir.join("runtime/contracts.transaction.lock"),
            fixtures: randomizer_dir.join("fixtures"),
            runtime: randomizer_dir.join("runtime"),
            randomizer_dir,
            root,
        }
    }

    pub fn load_manifest(&self) -> Result<super::ProjectManifest, ManifestError> {
        let contents =
            std::fs::read_to_string(&self.manifest).map_err(|source| ManifestError::Read {
                path: self.manifest.clone(),
                source,
            })?;
        serde_yaml::from_str(&contents).map_err(|source| ManifestError::Parse {
            path: self.manifest.clone(),
            source,
        })
    }
}

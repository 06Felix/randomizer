use std::{collections::BTreeMap, fs, io, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::project::ProjectPaths;

use super::{
    CliError,
    args::{SkillArgs, SkillCommand, SkillSyncArgs},
};

const LOCK_FORMAT_VERSION: u32 = 1;
const SKILL_NAME: &str = "randomizer-mocks";
const SKILL_VERSION: u32 = 2;
const LOCK_PATH: &str = ".randomizer/skills.lock.json";
const SKILL_PATH: &str = ".agents/skills/randomizer-mocks/SKILL.md";
const METADATA_PATH: &str = ".agents/skills/randomizer-mocks/agents/openai.yaml";
const CONTRACT_REFERENCE_PATH: &str = ".agents/skills/randomizer-mocks/references/contracts.md";
const SKILL_CONTENTS: &str = include_str!("../../assets/randomizer-mocks/SKILL.md");
const METADATA_CONTENTS: &str = include_str!("../../assets/randomizer-mocks/agents/openai.yaml");
const CONTRACT_REFERENCE_CONTENTS: &str =
    include_str!("../../assets/randomizer-mocks/references/contracts.md");

const FILES: [(&str, &str); 3] = [
    (SKILL_PATH, SKILL_CONTENTS),
    (METADATA_PATH, METADATA_CONTENTS),
    (CONTRACT_REFERENCE_PATH, CONTRACT_REFERENCE_CONTENTS),
];

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("failed to read skill file {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write skill file {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid skill lock {path}: {source}")]
    ParseLock {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported skill lock format {provided}; supported format is {supported}")]
    LockVersion { provided: u32, supported: u32 },
    #[error("skill lock manages {provided:?}; expected {expected:?}")]
    LockName {
        provided: String,
        expected: &'static str,
    },
    #[error(
        "refusing to overwrite locally modified skill files: {paths}; review them or run `randomizer skill sync --force`"
    )]
    LocallyModified { paths: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillLock {
    format_version: u32,
    skill: String,
    skill_version: u32,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOutcome {
    Installed,
    Updated,
    Current,
}

impl SyncOutcome {
    pub fn message(self) -> &'static str {
        match self {
            Self::Installed => "installed repository skill $randomizer-mocks",
            Self::Updated => "updated repository skill $randomizer-mocks",
            Self::Current => "repository skill $randomizer-mocks is current",
        }
    }
}

pub fn skill(args: SkillArgs) -> Result<(), CliError> {
    match args.command {
        SkillCommand::Sync(args) => sync(args),
    }
}

fn sync(args: SkillSyncArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::for_init(&args.project.project)?;
    let outcome = sync_at_root(&paths.root, args.force)?;
    println!("{}", outcome.message());
    Ok(())
}

pub fn sync_at_root(root: &Path, force: bool) -> Result<SyncOutcome, SkillError> {
    let lock_path = root.join(LOCK_PATH);
    let existing_lock = match load_lock(&lock_path) {
        Ok(lock) => lock,
        Err(
            SkillError::ParseLock { .. }
            | SkillError::LockVersion { .. }
            | SkillError::LockName { .. },
        ) if force => None,
        Err(error) => return Err(error),
    };
    let expected_hashes = expected_hashes();
    let installed_before = FILES.iter().any(|(path, _)| root.join(path).exists());
    let current_before = FILES.iter().all(|(path, contents)| {
        read_optional(&root.join(path))
            .is_ok_and(|installed| installed.as_deref() == Some(contents.as_bytes()))
    });
    let lock_current = existing_lock
        .as_ref()
        .is_some_and(|lock| lock.skill_version == SKILL_VERSION && lock.files == expected_hashes);

    let modified = locally_modified(root, existing_lock.as_ref(), &expected_hashes)?;
    if !force && !modified.is_empty() {
        return Err(SkillError::LocallyModified {
            paths: modified.join(", "),
        });
    }
    if current_before && lock_current {
        return Ok(SyncOutcome::Current);
    }

    for (relative, contents) in FILES {
        write_file(&root.join(relative), contents.as_bytes())?;
    }
    save_lock(
        &lock_path,
        &SkillLock {
            format_version: LOCK_FORMAT_VERSION,
            skill: SKILL_NAME.into(),
            skill_version: SKILL_VERSION,
            files: expected_hashes,
        },
    )?;

    Ok(if installed_before {
        SyncOutcome::Updated
    } else {
        SyncOutcome::Installed
    })
}

fn locally_modified(
    root: &Path,
    lock: Option<&SkillLock>,
    expected: &BTreeMap<String, String>,
) -> Result<Vec<String>, SkillError> {
    let mut modified = Vec::new();
    for (relative, _) in FILES {
        let path = root.join(relative);
        let Some(contents) = read_optional(&path)? else {
            continue;
        };
        let actual = hash(&contents);
        let bundled = &expected[relative];
        let managed = lock.and_then(|lock| lock.files.get(relative));
        if actual != *bundled && managed != Some(&actual) {
            modified.push(relative.to_string());
        }
    }
    Ok(modified)
}

fn load_lock(path: &Path) -> Result<Option<SkillLock>, SkillError> {
    let Some(contents) = read_optional(path)? else {
        return Ok(None);
    };
    let lock: SkillLock =
        serde_json::from_slice(&contents).map_err(|source| SkillError::ParseLock {
            path: path.to_path_buf(),
            source,
        })?;
    if lock.format_version != LOCK_FORMAT_VERSION {
        return Err(SkillError::LockVersion {
            provided: lock.format_version,
            supported: LOCK_FORMAT_VERSION,
        });
    }
    if lock.skill != SKILL_NAME {
        return Err(SkillError::LockName {
            provided: lock.skill,
            expected: SKILL_NAME,
        });
    }
    Ok(Some(lock))
}

fn save_lock(path: &Path, lock: &SkillLock) -> Result<(), SkillError> {
    let mut encoded = serde_json::to_vec_pretty(lock).expect("skill lock is serializable");
    encoded.push(b'\n');
    write_file(path, &encoded)
}

fn write_file(path: &Path, contents: &[u8]) -> Result<(), SkillError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SkillError::Write {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, contents).map_err(|source| SkillError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, SkillError> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SkillError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn expected_hashes() -> BTreeMap<String, String> {
    FILES
        .iter()
        .map(|(path, contents)| ((*path).to_string(), hash(contents.as_bytes())))
        .collect()
}

fn hash(contents: &[u8]) -> String {
    format!("{:x}", Sha256::digest(contents))
}

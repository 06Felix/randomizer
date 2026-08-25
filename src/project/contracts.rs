use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::{
    generation::{GenerationMode, GenerationOptions, content_hash},
    provider::{
        EndpointSelector, FieldEvidence, PROVIDER_PROTOCOL_VERSION, ProviderDiagnostic,
        ProviderError, ProviderIdentity, ProviderRequest, ProviderResponse, SourceFingerprint,
        fingerprint_bytes, validate_provider_response,
    },
    schema::JsonSchemaContract,
    standard::StandardGenerationPlan,
};

use super::ProjectPaths;

pub const CONTRACT_LOCK_FORMAT_VERSION: u32 = 1;
const CONTRACT_TRANSACTION_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ContractTransactionJournal {
    format_version: u32,
    contract_name: String,
    artifact: String,
    artifact_before: FileSnapshot,
    lock_before: FileSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "state",
    content = "contents",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum FileSnapshot {
    Missing,
    Present(Vec<u8>),
}

struct ContractTransactionLock {
    file: File,
}

impl Drop for ContractTransactionLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContractRecipe {
    JsonSchema {
        source: String,
        endpoint: EndpointSelector,
    },
    Openapi {
        source: String,
        endpoint: EndpointSelector,
    },
    SerializedExample {
        source: String,
        endpoint: EndpointSelector,
    },
    External {
        program: String,
        #[serde(default)]
        arguments: Vec<String>,
        request: ProviderRequest,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManagedContract {
    pub artifact: String,
    pub contract_version: String,
    pub schema_hash: String,
    pub provider: ProviderIdentity,
    pub endpoint: EndpointSelector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_symbol: Option<String>,
    pub source_fingerprints: Vec<SourceFingerprint>,
    #[serde(default)]
    pub evidence: Vec<FieldEvidence>,
    #[serde(default)]
    pub diagnostics: Vec<ProviderDiagnostic>,
    pub recipe: ContractRecipe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContractLock {
    pub format_version: u32,
    #[serde(default)]
    pub contracts: BTreeMap<String, ManagedContract>,
}

impl Default for ContractLock {
    fn default() -> Self {
        Self {
            format_version: CONTRACT_LOCK_FORMAT_VERSION,
            contracts: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractCheckReport {
    pub names: Vec<String>,
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("contract name {0:?} must contain only lowercase letters, digits, and '-'")]
    InvalidName(String),
    #[error("contract version must not be empty")]
    EmptyVersion,
    #[error("failed to read contract state {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid contract lock {path}: {source}")]
    ParseLock {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("unsupported contract lock format {provided}; supported format is {supported}")]
    LockVersion { provided: u32, supported: u32 },
    #[error("managed contract {0:?} was not found")]
    NotManaged(String),
    #[error(
        "managed contract {name:?} changed while its provider was running; retry the refresh against the newer contract state"
    )]
    ConcurrentUpdate { name: String },
    #[error("invalid managed contract lock entry for {name:?}: {message}")]
    InvalidLockEntry { name: String, message: String },
    #[error(
        "refusing to overwrite unmanaged contract artifact {0}; move it or choose another contract name"
    )]
    UnmanagedArtifact(PathBuf),
    #[error("managed contract {name:?} has unsafe artifact path {path:?}")]
    UnsafeArtifact { name: String, path: String },
    #[error("unsafe contract state path {path}: {message}")]
    UnsafeStatePath { path: PathBuf, message: String },
    #[error("contract source path {0:?} must be a project-relative file path")]
    UnsafeSource(String),
    #[error("contract source {path:?} resolves outside project root {root}")]
    SourceOutsideProject { path: String, root: PathBuf },
    #[error(
        "contract source fingerprint is stale for {path:?}; run `randomizer contract refresh {contract}`"
    )]
    StaleSource { contract: String, path: String },
    #[error("contract artifact hash is stale for {name:?}; refresh or re-import the contract")]
    StaleArtifact { name: String },
    #[error("invalid managed contract artifact {path}: {source}")]
    ParseArtifact {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("contract {name:?} cannot generate a supported response: {source}")]
    Generation {
        name: String,
        #[source]
        source: crate::error::GenerationError,
    },
    #[error("failed to encode contract state: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("failed to write contract state {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to lock contract transactions at {path}: {source}")]
    TransactionLock {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "another contract command is updating contract state (lock: {0}); retry after it finishes"
    )]
    TransactionBusy(PathBuf),
    #[error(
        "invalid contract transaction journal {path}: {source}; inspect the artifact and lock before removing the journal manually"
    )]
    ParseTransaction {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "unsupported contract transaction journal format {provided}; supported format is {supported}; upgrade or recover the journal with a compatible Randomizer version"
    )]
    TransactionVersion { provided: u32, supported: u32 },
    #[error("contract transaction journal has unsafe artifact path {path:?} for {name:?}")]
    UnsafeTransactionArtifact { name: String, path: String },
    #[error(
        "contract transaction journal already exists at {0}; rerun the command to recover it before starting another update"
    )]
    TransactionJournalExists(PathBuf),
    #[error(
        "contract transaction failed while {operation}: {source}; prior artifact and lock state were restored, so the command can be retried"
    )]
    TransactionRolledBack {
        operation: &'static str,
        #[source]
        source: Box<ContractError>,
    },
    #[error(
        "contract transaction failed while {operation}: {failure}; automatic rollback also failed: {recovery}. Inspect {journal}, resolve the filesystem error, and rerun the command to retry recovery"
    )]
    TransactionRecoveryFailed {
        operation: &'static str,
        failure: Box<ContractError>,
        recovery: Box<ContractError>,
        journal: PathBuf,
    },
    #[error(
        "failed to recover interrupted contract transaction from {journal}: {source}; leave the journal in place, resolve the filesystem error, and rerun the command"
    )]
    TransactionRecovery {
        journal: PathBuf,
        #[source]
        source: Box<ContractError>,
    },
}

pub fn save_managed_contract(
    paths: &ProjectPaths,
    name: &str,
    contract_version: &str,
    response: ProviderResponse,
    recipe: ContractRecipe,
) -> Result<ManagedContract, ContractError> {
    save_managed_contract_inner(paths, name, contract_version, response, recipe, None)
}

/// Saves a refreshed contract only if the managed entry still matches the entry
/// that was loaded before the provider ran.
pub fn save_managed_contract_if_unchanged(
    paths: &ProjectPaths,
    name: &str,
    contract_version: &str,
    response: ProviderResponse,
    recipe: ContractRecipe,
    expected: &ManagedContract,
) -> Result<ManagedContract, ContractError> {
    save_managed_contract_inner(
        paths,
        name,
        contract_version,
        response,
        recipe,
        Some(expected),
    )
}

fn save_managed_contract_inner(
    paths: &ProjectPaths,
    name: &str,
    contract_version: &str,
    response: ProviderResponse,
    recipe: ContractRecipe,
    expected: Option<&ManagedContract>,
) -> Result<ManagedContract, ContractError> {
    let _transaction_lock = acquire_transaction_lock(paths)?;
    recover_contract_transaction_unlocked(paths)?;
    validate_name(name)?;
    if contract_version.trim().is_empty() {
        return Err(ContractError::EmptyVersion);
    }
    validate_provider_response(&response)?;
    validate_recipe(name, &recipe, &response)?;
    validate_fingerprints(paths, name, &response.source_fingerprints)?;

    let schema_hash =
        content_hash(&response.schema).map_err(|source| ContractError::Generation {
            name: name.to_string(),
            source,
        })?;
    let artifact_relative = format!(".randomizer/contracts/{name}.json");
    let artifact_path = safe_artifact_path(paths, name, &artifact_relative, true)?;
    let mut lock = load_contract_lock_unlocked(paths)?;
    if expected.is_some_and(|expected| lock.contracts.get(name) != Some(expected)) {
        return Err(ContractError::ConcurrentUpdate {
            name: name.to_string(),
        });
    }
    if artifact_path.exists() && !lock.contracts.contains_key(name) {
        return Err(ContractError::UnmanagedArtifact(artifact_path));
    }

    let contract = JsonSchemaContract {
        name: name.to_string(),
        version: contract_version.to_string(),
        source: response.source_fingerprints.first().map_or_else(
            || response.provider.name.clone(),
            |source| source.path.clone(),
        ),
        schema: response.schema.clone(),
        content_hash: Some(schema_hash.clone()),
    };
    validate_generation(name, contract.clone())?;

    let managed = ManagedContract {
        artifact: artifact_relative,
        contract_version: contract_version.to_string(),
        schema_hash,
        provider: response.provider,
        endpoint: response.endpoint,
        root_symbol: response.root_symbol,
        source_fingerprints: response.source_fingerprints,
        evidence: response.evidence,
        diagnostics: response.diagnostics,
        recipe,
    };
    lock.contracts.insert(name.to_string(), managed.clone());

    let mut artifact = serde_json::to_vec_pretty(&contract)?;
    artifact.push(b'\n');
    let mut encoded_lock = serde_json::to_vec_pretty(&lock)?;
    encoded_lock.push(b'\n');
    commit_contract_transaction(paths, name, &artifact, &encoded_lock)?;
    Ok(managed)
}

pub fn load_contract_lock(paths: &ProjectPaths) -> Result<ContractLock, ContractError> {
    let _transaction_lock = acquire_transaction_lock(paths)?;
    recover_contract_transaction_unlocked(paths)?;
    load_contract_lock_unlocked(paths)
}

fn load_contract_lock_unlocked(paths: &ProjectPaths) -> Result<ContractLock, ContractError> {
    let lock_path = safe_state_file(paths, &paths.contracts_lock, "contracts.lock.json")?;
    let contents = match fs::read(&lock_path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            return Ok(ContractLock::default());
        }
        Err(source) => {
            return Err(ContractError::Read {
                path: lock_path.clone(),
                source,
            });
        }
    };
    let lock: ContractLock =
        serde_json::from_slice(&contents).map_err(|source| ContractError::ParseLock {
            path: lock_path,
            source,
        })?;
    if lock.format_version != CONTRACT_LOCK_FORMAT_VERSION {
        return Err(ContractError::LockVersion {
            provided: lock.format_version,
            supported: CONTRACT_LOCK_FORMAT_VERSION,
        });
    }
    Ok(lock)
}

pub fn managed_contract(
    paths: &ProjectPaths,
    name: &str,
) -> Result<ManagedContract, ContractError> {
    validate_name(name)?;
    load_contract_lock(paths)?
        .contracts
        .remove(name)
        .ok_or_else(|| ContractError::NotManaged(name.to_string()))
}

pub fn check_managed_contracts(
    paths: &ProjectPaths,
    name: Option<&str>,
) -> Result<ContractCheckReport, ContractError> {
    let _transaction_lock = acquire_transaction_lock(paths)?;
    recover_contract_transaction_unlocked(paths)?;
    let lock = load_contract_lock_unlocked(paths)?;
    let selected: Vec<_> = match name {
        Some(name) => {
            validate_name(name)?;
            vec![(
                name,
                lock.contracts
                    .get(name)
                    .ok_or_else(|| ContractError::NotManaged(name.to_string()))?,
            )]
        }
        None => lock
            .contracts
            .iter()
            .map(|(name, contract)| (name.as_str(), contract))
            .collect(),
    };

    let mut names = Vec::with_capacity(selected.len());
    for (name, managed) in selected {
        validate_name(name)?;
        let artifact_path = safe_artifact_path(paths, name, &managed.artifact, false)?;
        let contents = fs::read(&artifact_path).map_err(|source| ContractError::Read {
            path: artifact_path.clone(),
            source,
        })?;
        let contract: JsonSchemaContract =
            serde_json::from_slice(&contents).map_err(|source| ContractError::ParseArtifact {
                path: artifact_path,
                source,
            })?;
        let actual_hash =
            content_hash(&contract.schema).map_err(|source| ContractError::Generation {
                name: name.to_string(),
                source,
            })?;
        if actual_hash != managed.schema_hash
            || contract.content_hash.as_deref() != Some(actual_hash.as_str())
            || contract.name != name
            || contract.version != managed.contract_version
            || contract.source
                != managed.source_fingerprints.first().map_or_else(
                    || managed.provider.name.clone(),
                    |source| source.path.clone(),
                )
        {
            return Err(ContractError::StaleArtifact {
                name: name.to_string(),
            });
        }
        let response = ProviderResponse {
            protocol_version: PROVIDER_PROTOCOL_VERSION.to_string(),
            provider: managed.provider.clone(),
            endpoint: managed.endpoint.clone(),
            root_symbol: managed.root_symbol.clone(),
            schema: contract.schema.clone(),
            source_fingerprints: managed.source_fingerprints.clone(),
            evidence: managed.evidence.clone(),
            diagnostics: managed.diagnostics.clone(),
        };
        validate_provider_response(&response)?;
        validate_recipe(name, &managed.recipe, &response)?;
        validate_generation(name, contract)?;
        validate_fingerprints(paths, name, &managed.source_fingerprints)?;
        names.push(name.to_string());
    }
    Ok(ContractCheckReport { names })
}

fn validate_recipe(
    name: &str,
    recipe: &ContractRecipe,
    response: &ProviderResponse,
) -> Result<(), ContractError> {
    let invalid = |message: String| ContractError::InvalidLockEntry {
        name: name.to_string(),
        message,
    };
    match recipe {
        ContractRecipe::JsonSchema { source, endpoint }
        | ContractRecipe::Openapi { source, endpoint }
        | ContractRecipe::SerializedExample { source, endpoint } => {
            if endpoint != &response.endpoint {
                return Err(invalid(
                    "recipe endpoint does not match provider response".into(),
                ));
            }
            if !response
                .source_fingerprints
                .iter()
                .any(|fingerprint| &fingerprint.path == source)
            {
                return Err(invalid(format!(
                    "recipe source {source:?} has no source fingerprint"
                )));
            }
        }
        ContractRecipe::External {
            program, request, ..
        } => {
            if program.trim().is_empty() {
                return Err(invalid(
                    "external provider program must not be empty".into(),
                ));
            }
            request.validate()?;
            if request.endpoint != response.endpoint {
                return Err(invalid(
                    "external provider request endpoint does not match its response".into(),
                ));
            }
            for source in &request.source_paths {
                if !response
                    .source_fingerprints
                    .iter()
                    .any(|fingerprint| &fingerprint.path == source)
                {
                    return Err(invalid(format!(
                        "external provider source {source:?} has no source fingerprint"
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_generation(name: &str, contract: JsonSchemaContract) -> Result<(), ContractError> {
    let plan = StandardGenerationPlan::compile(
        contract,
        GenerationMode::Valid,
        &GenerationOptions {
            seed: Some(0),
            ..GenerationOptions::default()
        },
    )
    .map_err(|source| ContractError::Generation {
        name: name.to_string(),
        source,
    })?;
    plan.generate(0)
        .map(|_| ())
        .map_err(|source| ContractError::Generation {
            name: name.to_string(),
            source,
        })
}

fn validate_fingerprints(
    paths: &ProjectPaths,
    contract: &str,
    fingerprints: &[SourceFingerprint],
) -> Result<(), ContractError> {
    for expected in fingerprints {
        let source = safe_source_path(paths, &expected.path)?;
        let contents = fs::read(&source).map_err(|source_error| ContractError::Read {
            path: source,
            source: source_error,
        })?;
        if fingerprint_bytes(&expected.path, &contents) != *expected {
            return Err(ContractError::StaleSource {
                contract: contract.to_string(),
                path: expected.path.clone(),
            });
        }
    }
    Ok(())
}

fn safe_source_path(paths: &ProjectPaths, relative: &str) -> Result<PathBuf, ContractError> {
    let relative_path = Path::new(relative);
    if relative.is_empty()
        || relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ContractError::UnsafeSource(relative.to_string()));
    }
    let resolved = paths
        .root
        .join(relative_path)
        .canonicalize()
        .map_err(|source| ContractError::Read {
            path: paths.root.join(relative_path),
            source,
        })?;
    if !resolved.starts_with(&paths.root) || !resolved.is_file() {
        return Err(ContractError::SourceOutsideProject {
            path: relative.to_string(),
            root: paths.root.clone(),
        });
    }
    Ok(resolved)
}

fn safe_artifact_path(
    paths: &ProjectPaths,
    name: &str,
    artifact: &str,
    create_contracts_directory: bool,
) -> Result<PathBuf, ContractError> {
    validate_name(name)?;
    let expected = format!(".randomizer/contracts/{name}.json");
    if artifact != expected {
        return Err(ContractError::UnsafeArtifact {
            name: name.to_string(),
            path: artifact.to_string(),
        });
    }
    let contracts = safe_contracts_directory(paths, create_contracts_directory)?;
    let artifact_path = contracts.join(format!("{name}.json"));
    validate_regular_leaf(paths, &artifact_path)?;
    Ok(artifact_path)
}

fn safe_randomizer_directory(paths: &ProjectPaths) -> Result<PathBuf, ContractError> {
    let expected = paths.root.join(".randomizer");
    if paths.randomizer_dir != expected {
        return Err(unsafe_state_path(
            &paths.randomizer_dir,
            format!("expected Randomizer directory {}", expected.display()),
        ));
    }
    validate_existing_directory(paths, &expected)?;
    Ok(expected)
}

fn safe_contracts_directory(paths: &ProjectPaths, create: bool) -> Result<PathBuf, ContractError> {
    let randomizer = safe_randomizer_directory(paths)?;
    let expected = randomizer.join("contracts");
    if paths.contracts != expected {
        return Err(unsafe_state_path(
            &paths.contracts,
            format!("expected contracts directory {}", expected.display()),
        ));
    }
    match fs::symlink_metadata(&expected) {
        Ok(_) => validate_existing_directory(paths, &expected)?,
        Err(source) if source.kind() == io::ErrorKind::NotFound && create => {
            fs::create_dir(&expected).map_err(|source| ContractError::Write {
                path: expected.clone(),
                source,
            })?;
            validate_existing_directory(paths, &expected)?;
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ContractError::Read {
                path: expected,
                source,
            });
        }
    }
    Ok(expected)
}

fn safe_state_file(
    paths: &ProjectPaths,
    configured: &Path,
    file_name: &str,
) -> Result<PathBuf, ContractError> {
    let randomizer = safe_randomizer_directory(paths)?;
    let expected = randomizer.join(file_name);
    if configured != expected {
        return Err(unsafe_state_path(
            configured,
            format!("expected contract state file {}", expected.display()),
        ));
    }
    validate_regular_leaf(paths, &expected)?;
    Ok(expected)
}

fn safe_transaction_lock_file(paths: &ProjectPaths) -> Result<PathBuf, ContractError> {
    let runtime = safe_runtime_directory(paths, true)?;
    let expected = runtime.join("contracts.transaction.lock");
    if paths.contracts_transaction_lock != expected {
        return Err(unsafe_state_path(
            &paths.contracts_transaction_lock,
            format!("expected contract transaction lock {}", expected.display()),
        ));
    }
    validate_regular_leaf(paths, &expected)?;
    Ok(expected)
}

fn safe_runtime_directory(paths: &ProjectPaths, create: bool) -> Result<PathBuf, ContractError> {
    let randomizer = safe_randomizer_directory(paths)?;
    let expected = randomizer.join("runtime");
    if paths.runtime != expected {
        return Err(unsafe_state_path(
            &paths.runtime,
            format!("expected runtime directory {}", expected.display()),
        ));
    }
    match fs::symlink_metadata(&expected) {
        Ok(_) => validate_existing_directory(paths, &expected)?,
        Err(source) if source.kind() == io::ErrorKind::NotFound && create => {
            fs::create_dir(&expected).map_err(|source| ContractError::Write {
                path: expected.clone(),
                source,
            })?;
            validate_existing_directory(paths, &expected)?;
        }
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(ContractError::Read {
                path: expected,
                source,
            });
        }
    }
    Ok(expected)
}

fn validate_existing_directory(paths: &ProjectPaths, path: &Path) -> Result<(), ContractError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| ContractError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(unsafe_state_path(path, "symbolic links are not allowed"));
    }
    if !metadata.is_dir() {
        return Err(unsafe_state_path(path, "expected a directory"));
    }
    let resolved = path.canonicalize().map_err(|source| ContractError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if !resolved.starts_with(&paths.root) || resolved != path {
        return Err(unsafe_state_path(
            path,
            format!(
                "resolves to {}, outside the expected project location",
                resolved.display()
            ),
        ));
    }
    Ok(())
}

fn validate_regular_leaf(paths: &ProjectPaths, path: &Path) -> Result<(), ContractError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(ContractError::Read {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(unsafe_state_path(path, "symbolic links are not allowed"));
    }
    if !metadata.is_file() {
        return Err(unsafe_state_path(path, "expected a regular file"));
    }
    let resolved = path.canonicalize().map_err(|source| ContractError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    if !resolved.starts_with(&paths.root) || resolved != path {
        return Err(unsafe_state_path(
            path,
            format!(
                "resolves to {}, outside the expected project location",
                resolved.display()
            ),
        ));
    }
    Ok(())
}

fn unsafe_state_path(path: &Path, message: impl Into<String>) -> ContractError {
    ContractError::UnsafeStatePath {
        path: path.to_path_buf(),
        message: message.into(),
    }
}

fn validate_name(name: &str) -> Result<(), ContractError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ContractError::InvalidName(name.to_string()));
    }
    Ok(())
}

fn acquire_transaction_lock(
    paths: &ProjectPaths,
) -> Result<ContractTransactionLock, ContractError> {
    safe_contracts_directory(paths, false)?;
    let lock_path = safe_transaction_lock_file(paths)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|source| ContractError::TransactionLock {
            path: lock_path.clone(),
            source,
        })?;
    validate_regular_leaf(paths, &lock_path)?;
    match file.try_lock() {
        Ok(()) => Ok(ContractTransactionLock { file }),
        Err(TryLockError::WouldBlock) => Err(ContractError::TransactionBusy(lock_path)),
        Err(TryLockError::Error(source)) => Err(ContractError::TransactionLock {
            path: lock_path,
            source,
        }),
    }
}

fn commit_contract_transaction(
    paths: &ProjectPaths,
    name: &str,
    artifact: &[u8],
    encoded_lock: &[u8],
) -> Result<(), ContractError> {
    commit_contract_transaction_with(paths, name, artifact, encoded_lock, atomic_write)
}

fn commit_contract_transaction_with<F>(
    paths: &ProjectPaths,
    name: &str,
    artifact: &[u8],
    encoded_lock: &[u8],
    mut writer: F,
) -> Result<(), ContractError>
where
    F: FnMut(&Path, &[u8]) -> Result<(), ContractError>,
{
    let journal = begin_contract_transaction(paths, name)?;
    let artifact_path = validated_transaction_artifact(paths, &journal)?;
    let lock_path = safe_state_file(paths, &paths.contracts_lock, "contracts.lock.json")?;

    if let Err(source) = writer(&artifact_path, artifact) {
        return rollback_after_transaction_failure(
            paths,
            &journal,
            "writing the contract artifact",
            source,
        );
    }
    if let Err(source) = writer(&lock_path, encoded_lock) {
        return rollback_after_transaction_failure(
            paths,
            &journal,
            "writing the contract lock",
            source,
        );
    }
    if let Err(source) = remove_transaction_journal(paths) {
        return rollback_after_transaction_failure(
            paths,
            &journal,
            "acknowledging the completed update",
            source,
        );
    }
    Ok(())
}

fn begin_contract_transaction(
    paths: &ProjectPaths,
    name: &str,
) -> Result<ContractTransactionJournal, ContractError> {
    validate_name(name)?;
    let artifact = format!(".randomizer/contracts/{name}.json");
    let artifact_path = safe_artifact_path(paths, name, &artifact, true)?;
    let lock_path = safe_state_file(paths, &paths.contracts_lock, "contracts.lock.json")?;
    let journal = ContractTransactionJournal {
        format_version: CONTRACT_TRANSACTION_FORMAT_VERSION,
        contract_name: name.to_string(),
        artifact,
        artifact_before: read_file_snapshot(&artifact_path)?,
        lock_before: read_file_snapshot(&lock_path)?,
    };
    persist_transaction_journal(paths, &journal)?;
    Ok(journal)
}

fn persist_transaction_journal(
    paths: &ProjectPaths,
    journal: &ContractTransactionJournal,
) -> Result<(), ContractError> {
    let mut contents = serde_json::to_vec_pretty(journal)?;
    contents.push(b'\n');
    let journal_path = safe_state_file(
        paths,
        &paths.contracts_transaction,
        "contracts.transaction.json",
    )?;
    atomic_create(&journal_path, &contents)
}

fn read_file_snapshot(path: &Path) -> Result<FileSnapshot, ContractError> {
    match fs::read(path) {
        Ok(contents) => Ok(FileSnapshot::Present(contents)),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(FileSnapshot::Missing),
        Err(source) => Err(ContractError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn recover_contract_transaction_unlocked(paths: &ProjectPaths) -> Result<(), ContractError> {
    let Some(journal) = load_transaction_journal(paths)? else {
        return Ok(());
    };
    restore_transaction(paths, &journal)
}

fn load_transaction_journal(
    paths: &ProjectPaths,
) -> Result<Option<ContractTransactionJournal>, ContractError> {
    let journal_path = safe_state_file(
        paths,
        &paths.contracts_transaction,
        "contracts.transaction.json",
    )?;
    let contents = match fs::read(&journal_path) {
        Ok(contents) => contents,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(ContractError::Read {
                path: journal_path.clone(),
                source,
            });
        }
    };
    let journal: ContractTransactionJournal =
        serde_json::from_slice(&contents).map_err(|source| ContractError::ParseTransaction {
            path: journal_path,
            source,
        })?;
    validated_transaction_artifact(paths, &journal)?;
    Ok(Some(journal))
}

fn validated_transaction_artifact(
    paths: &ProjectPaths,
    journal: &ContractTransactionJournal,
) -> Result<PathBuf, ContractError> {
    if journal.format_version != CONTRACT_TRANSACTION_FORMAT_VERSION {
        return Err(ContractError::TransactionVersion {
            provided: journal.format_version,
            supported: CONTRACT_TRANSACTION_FORMAT_VERSION,
        });
    }
    validate_name(&journal.contract_name)?;
    let expected = format!(".randomizer/contracts/{}.json", journal.contract_name);
    if journal.artifact != expected {
        return Err(ContractError::UnsafeTransactionArtifact {
            name: journal.contract_name.clone(),
            path: journal.artifact.clone(),
        });
    }
    safe_artifact_path(paths, &journal.contract_name, &journal.artifact, false)
}

fn restore_transaction(
    paths: &ProjectPaths,
    journal: &ContractTransactionJournal,
) -> Result<(), ContractError> {
    let artifact_path = validated_transaction_artifact(paths, journal)?;
    let lock_path = safe_state_file(paths, &paths.contracts_lock, "contracts.lock.json")?;
    restore_file(&artifact_path, &journal.artifact_before)
        .map_err(|source| transaction_recovery_error(paths, source))?;
    restore_file(&lock_path, &journal.lock_before)
        .map_err(|source| transaction_recovery_error(paths, source))?;
    remove_transaction_journal(paths)
        .map_err(|source| transaction_recovery_error(paths, source))?;
    Ok(())
}

fn restore_file(path: &Path, snapshot: &FileSnapshot) -> Result<(), ContractError> {
    match snapshot {
        FileSnapshot::Present(contents) => atomic_write(path, contents),
        FileSnapshot::Missing => match fs::remove_file(path) {
            Ok(()) => sync_parent_directory(path),
            Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ContractError::Write {
                path: path.to_path_buf(),
                source,
            }),
        },
    }
}

fn rollback_after_transaction_failure(
    paths: &ProjectPaths,
    journal: &ContractTransactionJournal,
    operation: &'static str,
    failure: ContractError,
) -> Result<(), ContractError> {
    let recovery = ensure_transaction_journal(paths, journal)
        .and_then(|()| restore_transaction(paths, journal));
    match recovery {
        Ok(()) => Err(ContractError::TransactionRolledBack {
            operation,
            source: Box::new(failure),
        }),
        Err(recovery) => Err(ContractError::TransactionRecoveryFailed {
            operation,
            failure: Box::new(failure),
            recovery: Box::new(recovery),
            journal: paths.contracts_transaction.clone(),
        }),
    }
}

fn ensure_transaction_journal(
    paths: &ProjectPaths,
    journal: &ContractTransactionJournal,
) -> Result<(), ContractError> {
    let journal_path = safe_state_file(
        paths,
        &paths.contracts_transaction,
        "contracts.transaction.json",
    )?;
    match fs::metadata(&journal_path) {
        Ok(_) => Ok(()),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            persist_transaction_journal(paths, journal)
        }
        Err(source) => Err(ContractError::Read {
            path: journal_path,
            source,
        }),
    }
}

fn remove_transaction_journal(paths: &ProjectPaths) -> Result<(), ContractError> {
    let journal_path = safe_state_file(
        paths,
        &paths.contracts_transaction,
        "contracts.transaction.json",
    )?;
    match fs::remove_file(&journal_path) {
        Ok(()) => sync_parent_directory(&journal_path),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(ContractError::Write {
            path: journal_path,
            source,
        }),
    }
}

fn transaction_recovery_error(paths: &ProjectPaths, source: ContractError) -> ContractError {
    ContractError::TransactionRecovery {
        journal: paths.contracts_transaction.clone(),
        source: Box::new(source),
    }
}

fn atomic_write(path: &Path, contents: &[u8]) -> Result<(), ContractError> {
    let parent = path.parent().ok_or_else(|| ContractError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory"),
    })?;
    fs::create_dir_all(parent).map_err(|source| ContractError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| ContractError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    temporary
        .write_all(contents)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| ContractError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| ContractError::Write {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    sync_parent_directory(path)
}

fn atomic_create(path: &Path, contents: &[u8]) -> Result<(), ContractError> {
    let parent = path.parent().ok_or_else(|| ContractError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory"),
    })?;
    fs::create_dir_all(parent).map_err(|source| ContractError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| ContractError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    temporary
        .write_all(contents)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| ContractError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == io::ErrorKind::AlreadyExists {
            ContractError::TransactionJournalExists(path.to_path_buf())
        } else {
            ContractError::Write {
                path: path.to_path_buf(),
                source: error.error,
            }
        }
    })?;
    sync_parent_directory(path)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), ContractError> {
    let parent = path.parent().ok_or_else(|| ContractError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory"),
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ContractError::Write {
            path: parent.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), ContractError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::provider::{
        EVIDENCE_KIND_CONSTRAINT, EVIDENCE_KIND_ENUM, EVIDENCE_KIND_FORMAT,
        EVIDENCE_KIND_PROPERTY_NAME, EVIDENCE_KIND_REQUIREDNESS, EVIDENCE_KIND_RESPONSE_WRAPPER,
        EVIDENCE_KIND_TYPE, JSON_SCHEMA_DRAFT_2020_12, ProviderIdentity,
    };

    fn paths() -> (tempfile::TempDir, ProjectPaths) {
        let directory = tempdir().unwrap();
        fs::create_dir(directory.path().join(".randomizer")).unwrap();
        let paths = ProjectPaths::for_init(directory.path()).unwrap();
        (directory, paths)
    }

    fn response(root: &Path, endpoint: &EndpointSelector) -> ProviderResponse {
        let source_path = "api/user.schema.json";
        let contents = br#"{"source":"types"}"#;
        fs::create_dir_all(root.join("api")).unwrap();
        fs::write(root.join(source_path), contents).unwrap();
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test.provider", "1"),
            endpoint.clone(),
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "type": "object",
                "required": ["state", "created_at"],
                "properties": {
                    "state": {"type": "string", "enum": ["ACTIVE", "PAUSED"]},
                    "created_at": {"type": "string", "format": "date-time"}
                },
                "additionalProperties": false
            }),
            vec![fingerprint_bytes(source_path, contents)],
        );
        response.evidence = [
            ("#", EVIDENCE_KIND_RESPONSE_WRAPPER),
            ("#/type", EVIDENCE_KIND_TYPE),
            ("#/required/0", EVIDENCE_KIND_REQUIREDNESS),
            ("#/required/1", EVIDENCE_KIND_REQUIREDNESS),
            ("#/properties/state", EVIDENCE_KIND_PROPERTY_NAME),
            ("#/properties/state/type", EVIDENCE_KIND_TYPE),
            ("#/properties/state/enum", EVIDENCE_KIND_ENUM),
            ("#/properties/created_at", EVIDENCE_KIND_PROPERTY_NAME),
            ("#/properties/created_at/type", EVIDENCE_KIND_TYPE),
            ("#/properties/created_at/format", EVIDENCE_KIND_FORMAT),
            ("#/additionalProperties", EVIDENCE_KIND_CONSTRAINT),
        ]
        .into_iter()
        .map(|(schema_path, kind)| FieldEvidence {
            schema_path: schema_path.to_string(),
            source_path: source_path.to_string(),
            source_location: "User".to_string(),
            kind: kind.to_string(),
        })
        .collect();
        response
    }

    #[test]
    fn saves_and_checks_managed_contract_with_precise_wire_types() {
        let (directory, paths) = paths();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        let response = response(directory.path(), &endpoint);

        let saved = save_managed_contract(
            &paths,
            "get-user",
            "1",
            response,
            ContractRecipe::JsonSchema {
                source: "api/user.schema.json".into(),
                endpoint,
            },
        )
        .unwrap();

        assert_eq!(saved.provider.name, "test.provider");
        assert!(!paths.contracts_transaction.exists());
        assert_eq!(
            check_managed_contracts(&paths, None).unwrap().names,
            ["get-user"]
        );
        let artifact: JsonSchemaContract =
            serde_json::from_slice(&fs::read(paths.contracts.join("get-user.json")).unwrap())
                .unwrap();
        assert_eq!(artifact.schema["properties"]["state"]["enum"][0], "ACTIVE");
        assert_eq!(
            artifact.schema["properties"]["created_at"]["format"],
            "date-time"
        );
    }

    #[test]
    fn conditional_save_does_not_overwrite_a_newer_managed_contract() {
        let (directory, paths) = paths();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        let recipe = ContractRecipe::JsonSchema {
            source: "api/user.schema.json".into(),
            endpoint: endpoint.clone(),
        };
        let original = save_managed_contract(
            &paths,
            "get-user",
            "1",
            response(directory.path(), &endpoint),
            recipe.clone(),
        )
        .unwrap();

        let mut newer_response = response(directory.path(), &endpoint);
        newer_response.provider.version = "2".into();
        let newer =
            save_managed_contract(&paths, "get-user", "1", newer_response, recipe.clone()).unwrap();

        let error = save_managed_contract_if_unchanged(
            &paths,
            "get-user",
            "1",
            response(directory.path(), &endpoint),
            recipe,
            &original,
        )
        .unwrap_err();

        assert!(matches!(error, ContractError::ConcurrentUpdate { .. }));
        assert_eq!(managed_contract(&paths, "get-user").unwrap(), newer);
    }

    #[test]
    fn reports_stale_sources_and_unmanaged_artifacts() {
        let (directory, paths) = paths();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        save_managed_contract(
            &paths,
            "get-user",
            "1",
            response(directory.path(), &endpoint),
            ContractRecipe::JsonSchema {
                source: "api/user.schema.json".into(),
                endpoint,
            },
        )
        .unwrap();
        fs::write(directory.path().join("api/user.schema.json"), "changed").unwrap();
        assert!(matches!(
            check_managed_contracts(&paths, Some("get-user")),
            Err(ContractError::StaleSource { .. })
        ));

        fs::write(paths.contracts.join("other.json"), "{}").unwrap();
        let endpoint = EndpointSelector::new("GET", "/other", 200);
        let source = response(directory.path(), &endpoint);
        assert!(matches!(
            save_managed_contract(
                &paths,
                "other",
                "1",
                source,
                ContractRecipe::JsonSchema {
                    source: "api/user.schema.json".into(),
                    endpoint,
                }
            ),
            Err(ContractError::UnmanagedArtifact(_))
        ));
    }

    #[test]
    fn failed_lock_write_rolls_back_both_raw_files() {
        let (_directory, paths) = paths();
        fs::create_dir_all(&paths.contracts).unwrap();
        let artifact_path = paths.contracts.join("get-user.json");
        let old_artifact = b"old artifact bytes\n";
        let old_lock = b"old lock bytes\n";
        fs::write(&artifact_path, old_artifact).unwrap();
        fs::write(&paths.contracts_lock, old_lock).unwrap();

        let mut write_number = 0;
        let error = commit_contract_transaction_with(
            &paths,
            "get-user",
            b"new artifact bytes\n",
            b"new lock bytes\n",
            |path, contents| {
                write_number += 1;
                if write_number == 2 {
                    return Err(ContractError::Write {
                        path: path.to_path_buf(),
                        source: io::Error::other("simulated lock write failure"),
                    });
                }
                atomic_write(path, contents)
            },
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ContractError::TransactionRolledBack {
                operation: "writing the contract lock",
                ..
            }
        ));
        assert_eq!(fs::read(artifact_path).unwrap(), old_artifact);
        assert_eq!(fs::read(&paths.contracts_lock).unwrap(), old_lock);
        assert!(!paths.contracts_transaction.exists());
    }

    #[test]
    fn load_recovers_a_crash_after_the_artifact_write() {
        let (_directory, paths) = paths();
        fs::create_dir_all(&paths.contracts).unwrap();
        let artifact_path = paths.contracts.join("get-user.json");
        let old_artifact = b"old artifact bytes\n";
        fs::write(&artifact_path, old_artifact).unwrap();
        let mut old_lock = serde_json::to_vec_pretty(&ContractLock::default()).unwrap();
        old_lock.push(b'\n');
        fs::write(&paths.contracts_lock, &old_lock).unwrap();

        {
            let _transaction_lock = acquire_transaction_lock(&paths).unwrap();
            let _journal = begin_contract_transaction(&paths, "get-user").unwrap();
            atomic_write(&artifact_path, b"partially committed artifact\n").unwrap();
        }
        assert!(paths.contracts_transaction.exists());

        assert_eq!(load_contract_lock(&paths).unwrap(), ContractLock::default());
        assert_eq!(fs::read(artifact_path).unwrap(), old_artifact);
        assert_eq!(fs::read(&paths.contracts_lock).unwrap(), old_lock);
        assert!(!paths.contracts_transaction.exists());
    }

    #[test]
    fn crash_created_artifact_does_not_wedge_unmanaged_import() {
        let (directory, paths) = paths();
        let artifact_path = paths.contracts.join("get-user.json");
        {
            let _transaction_lock = acquire_transaction_lock(&paths).unwrap();
            let _journal = begin_contract_transaction(&paths, "get-user").unwrap();
            atomic_write(&artifact_path, b"partial artifact\n").unwrap();
        }
        assert!(artifact_path.exists());
        assert!(paths.contracts_transaction.exists());

        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        save_managed_contract(
            &paths,
            "get-user",
            "1",
            response(directory.path(), &endpoint),
            ContractRecipe::JsonSchema {
                source: "api/user.schema.json".into(),
                endpoint,
            },
        )
        .unwrap();

        assert!(artifact_path.exists());
        assert!(
            load_contract_lock(&paths)
                .unwrap()
                .contracts
                .contains_key("get-user")
        );
        assert!(!paths.contracts_transaction.exists());
    }

    #[test]
    fn tampered_journal_cannot_traverse_outside_the_project() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir_all(root.join(".randomizer")).unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let victim = directory.path().join("victim.json");
        fs::write(&victim, b"do not replace\n").unwrap();
        let journal = ContractTransactionJournal {
            format_version: CONTRACT_TRANSACTION_FORMAT_VERSION,
            contract_name: "get-user".into(),
            artifact: "../victim.json".into(),
            artifact_before: FileSnapshot::Present(b"attacker controlled\n".to_vec()),
            lock_before: FileSnapshot::Missing,
        };
        persist_transaction_journal(&paths, &journal).unwrap();

        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeTransactionArtifact { .. })
        ));
        assert_eq!(fs::read(victim).unwrap(), b"do not replace\n");
        assert!(paths.contracts_transaction.exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_contracts_directory_symlink_without_touching_outside_artifact() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        let randomizer = root.join(".randomizer");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&randomizer).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, randomizer.join("contracts")).unwrap();
        let victim = outside.join("get-user.json");
        fs::write(&victim, b"outside artifact must remain unchanged\n").unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);

        assert!(matches!(
            save_managed_contract(
                &paths,
                "get-user",
                "1",
                response(&root, &endpoint),
                ContractRecipe::JsonSchema {
                    source: "api/user.schema.json".into(),
                    endpoint,
                },
            ),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        assert_eq!(
            fs::read(victim).unwrap(),
            b"outside artifact must remain unchanged\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn recovery_rejects_contracts_directory_symlink_without_touching_outside_artifact() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        let randomizer = root.join(".randomizer");
        let outside = directory.path().join("outside");
        fs::create_dir_all(&randomizer).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, randomizer.join("contracts")).unwrap();
        let victim = outside.join("get-user.json");
        fs::write(&victim, b"partially written outside artifact\n").unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let journal = ContractTransactionJournal {
            format_version: CONTRACT_TRANSACTION_FORMAT_VERSION,
            contract_name: "get-user".into(),
            artifact: ".randomizer/contracts/get-user.json".into(),
            artifact_before: FileSnapshot::Present(b"attacker supplied rollback\n".to_vec()),
            lock_before: FileSnapshot::Missing,
        };
        fs::write(
            &paths.contracts_transaction,
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        assert_eq!(
            fs::read(victim).unwrap(),
            b"partially written outside artifact\n"
        );
        assert!(paths.contracts_transaction.exists());
    }

    #[cfg(unix)]
    #[test]
    fn import_and_recovery_reject_artifact_symlink_without_touching_victim() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir_all(root.join(".randomizer/contracts")).unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let victim = directory.path().join("victim.json");
        fs::write(&victim, b"outside artifact must remain unchanged\n").unwrap();
        symlink(&victim, paths.contracts.join("get-user.json")).unwrap();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);

        assert!(matches!(
            save_managed_contract(
                &paths,
                "get-user",
                "1",
                response(&root, &endpoint),
                ContractRecipe::JsonSchema {
                    source: "api/user.schema.json".into(),
                    endpoint,
                },
            ),
            Err(ContractError::UnsafeStatePath { .. })
        ));

        let journal = ContractTransactionJournal {
            format_version: CONTRACT_TRANSACTION_FORMAT_VERSION,
            contract_name: "get-user".into(),
            artifact: ".randomizer/contracts/get-user.json".into(),
            artifact_before: FileSnapshot::Present(b"attacker supplied rollback\n".to_vec()),
            lock_before: FileSnapshot::Missing,
        };
        fs::write(
            &paths.contracts_transaction,
            serde_json::to_vec_pretty(&journal).unwrap(),
        )
        .unwrap();
        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        assert_eq!(
            fs::read(victim).unwrap(),
            b"outside artifact must remain unchanged\n"
        );
        assert!(paths.contracts_transaction.exists());
    }

    #[test]
    fn check_rejects_unsafe_contract_names_from_a_tampered_lock() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir_all(root.join(".randomizer")).unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let endpoint = EndpointSelector::new("GET", "/users/{id}", 200);
        save_managed_contract(
            &paths,
            "get-user",
            "1",
            response(&root, &endpoint),
            ContractRecipe::JsonSchema {
                source: "api/user.schema.json".into(),
                endpoint,
            },
        )
        .unwrap();
        let victim = directory.path().join("victim.json");
        fs::write(&victim, b"outside lock target\n").unwrap();
        let mut lock = load_contract_lock(&paths).unwrap();
        let mut managed = lock.contracts.remove("get-user").unwrap();
        managed.artifact = ".randomizer/contracts/../../../victim.json".into();
        lock.contracts.insert("../../../victim".into(), managed);
        fs::write(
            &paths.contracts_lock,
            serde_json::to_vec_pretty(&lock).unwrap(),
        )
        .unwrap();

        assert!(matches!(
            check_managed_contracts(&paths, None),
            Err(ContractError::InvalidName(_))
        ));
        assert_eq!(fs::read(victim).unwrap(), b"outside lock target\n");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_lock_journal_and_transaction_lock_files() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project");
        fs::create_dir_all(root.join(".randomizer")).unwrap();
        let paths = ProjectPaths::for_init(&root).unwrap();
        let victim = directory.path().join("victim.json");
        fs::write(&victim, b"outside state must remain unchanged\n").unwrap();

        symlink(&victim, &paths.contracts_lock).unwrap();
        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        fs::remove_file(&paths.contracts_lock).unwrap();

        symlink(&victim, &paths.contracts_transaction).unwrap();
        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        fs::remove_file(&paths.contracts_transaction).unwrap();

        fs::remove_file(&paths.contracts_transaction_lock).unwrap();
        symlink(&victim, &paths.contracts_transaction_lock).unwrap();
        assert!(matches!(
            load_contract_lock(&paths),
            Err(ContractError::UnsafeStatePath { .. })
        ));
        assert_eq!(
            fs::read(victim).unwrap(),
            b"outside state must remain unchanged\n"
        );
    }
}

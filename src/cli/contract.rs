use std::{collections::BTreeMap, fs, path::PathBuf};

use thiserror::Error;

use crate::{
    dto::{
        BuildSystem, FieldPresence, JavaContractEntry, JavaContractLock, JavaDtoExtractor,
        JavaExportResult,
    },
    generation::{GenerationMode, GenerationOptions, content_hash},
    project::ProjectPaths,
    schema::JsonSchemaContract,
    standard::StandardGenerationPlan,
};

use super::{
    CliError,
    args::{ContractArgs, ContractCommand, JavaContractArgs, ProjectArgs},
};

pub fn contract(args: ContractArgs) -> Result<(), CliError> {
    match args.command {
        ContractCommand::ImportJava(args) => import_java(args),
        ContractCommand::Refresh(args) => refresh(args),
        ContractCommand::Check(args) => check(args),
    }
}

fn import_java(args: JavaContractArgs) -> Result<(), CliError> {
    validate_contract_name(&args.name)?;
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let mut lock = JavaContractLock::load(&paths.java_contract_lock)?;
    let contract_path = paths.contracts.join(format!("{}.json", args.name));
    if let Some(existing) = lock.contracts.get(&args.name) {
        let changed_owner = existing.root_type != args.root_type
            || existing.field_presence != args.field_presence
            || existing.build_system != BuildSystem::Maven;
        if changed_owner && !args.force {
            return Err(ContractError::OwnedByDifferentType {
                name: args.name,
                root_type: existing.root_type.clone(),
            }
            .into());
        }
    } else if contract_path.exists() && !args.force {
        return Err(ContractError::UnmanagedContract(contract_path).into());
    }

    let manifest = paths.load_manifest()?;
    let extractor = JavaDtoExtractor::prepare(&paths, args.build_system)?;
    let export = extractor.export(&args.root_type, args.field_presence)?;
    let generated = build_contract(
        &args.name,
        &args.root_type,
        args.field_presence,
        export,
        manifest.project.seed,
    )?;
    write_contract(&contract_path, &generated.contract)?;
    lock.contracts.insert(args.name.clone(), generated.entry);
    lock.save(&paths.java_contract_lock)?;
    print_warnings(&generated.warnings);
    println!("imported Java DTO contract {}", contract_path.display());
    println!(
        "reference it as .randomizer/contracts/{}.json in a route response",
        args.name
    );
    Ok(())
}

fn refresh(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let mut lock = JavaContractLock::load(&paths.java_contract_lock)?;
    if lock.contracts.is_empty() {
        println!("no Java DTO contracts are registered");
        return Ok(());
    }
    ensure_single_build_system(&lock)?;
    let manifest = paths.load_manifest()?;
    let extractor = JavaDtoExtractor::prepare(&paths, BuildSystem::Maven)?;
    let mut generated = BTreeMap::new();
    for (name, entry) in &lock.contracts {
        let export = extractor.export(&entry.root_type, entry.field_presence)?;
        generated.insert(
            name.clone(),
            build_contract(
                name,
                &entry.root_type,
                entry.field_presence,
                export,
                manifest.project.seed,
            )?,
        );
    }

    for (name, output) in &generated {
        let destination = paths.contracts.join(format!("{name}.json"));
        write_contract(&destination, &output.contract)?;
        print_warnings(&output.warnings);
    }
    for (name, output) in generated {
        lock.contracts.insert(name, output.entry);
    }
    lock.save(&paths.java_contract_lock)?;
    println!("refreshed {} Java DTO contracts", lock.contracts.len());
    Ok(())
}

fn check(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let lock = JavaContractLock::load(&paths.java_contract_lock)?;
    if lock.contracts.is_empty() {
        println!("no Java DTO contracts are registered");
        return Ok(());
    }
    ensure_single_build_system(&lock)?;
    let manifest = paths.load_manifest()?;
    let extractor = JavaDtoExtractor::prepare(&paths, BuildSystem::Maven)?;
    let mut stale = Vec::new();
    for (name, entry) in &lock.contracts {
        let export = extractor.export(&entry.root_type, entry.field_presence)?;
        let generated = build_contract(
            name,
            &entry.root_type,
            entry.field_presence,
            export,
            manifest.project.seed,
        )?;
        let path = paths.contracts.join(format!("{name}.json"));
        let stored = read_contract(&path).ok();
        let expected_path = format!(".randomizer/contracts/{name}.json");
        if entry.input_hash != generated.entry.input_hash
            || entry.exporter_version != generated.entry.exporter_version
            || entry.contract != expected_path
            || stored.as_ref().is_none_or(|stored| {
                stored.name != generated.contract.name
                    || stored.version != generated.contract.version
                    || stored.source != generated.contract.source
                    || stored.schema != generated.contract.schema
                    || stored.content_hash != generated.contract.content_hash
            })
        {
            stale.push(name.clone());
        }
    }
    if !stale.is_empty() {
        return Err(ContractError::Stale(stale).into());
    }
    println!(
        "checked {} Java DTO contracts; all are current",
        lock.contracts.len()
    );
    Ok(())
}

struct GeneratedContract {
    contract: JsonSchemaContract,
    entry: JavaContractEntry,
    warnings: Vec<String>,
}

fn build_contract(
    name: &str,
    root_type: &str,
    field_presence: FieldPresence,
    export: JavaExportResult,
    seed: u64,
) -> Result<GeneratedContract, CliError> {
    let hash = content_hash(&export.schema).map_err(ContractError::InvalidGeneratedContract)?;
    let contract = JsonSchemaContract {
        name: name.to_string(),
        version: "1".to_string(),
        source: format!("java:{root_type}"),
        schema: export.schema,
        content_hash: Some(hash),
    };
    let plan = StandardGenerationPlan::compile(
        contract.clone(),
        GenerationMode::Valid,
        &GenerationOptions {
            seed: Some(seed),
            ..GenerationOptions::default()
        },
    )
    .map_err(ContractError::InvalidGeneratedContract)?;
    plan.generate(0)
        .map_err(ContractError::UnsupportedGeneratedContract)?;

    Ok(GeneratedContract {
        entry: JavaContractEntry {
            contract: format!(".randomizer/contracts/{name}.json"),
            root_type: root_type.to_string(),
            build_system: BuildSystem::Maven,
            field_presence,
            input_hash: export.input_hash,
            exporter_version: env!("CARGO_PKG_VERSION").to_string(),
            visited_classes: export.visited_classes,
        },
        contract,
        warnings: export.warnings,
    })
}

fn write_contract(path: &std::path::Path, contract: &JsonSchemaContract) -> Result<(), CliError> {
    let mut encoded = serde_json::to_vec_pretty(contract)?;
    encoded.push(b'\n');
    crate::dto::atomic_write(path, &encoded).map_err(|source| CliError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn read_contract(path: &std::path::Path) -> Result<JsonSchemaContract, ContractError> {
    let bytes = fs::read(path).map_err(|source| ContractError::ReadContract {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ContractError::ParseContract {
        path: path.to_path_buf(),
        source,
    })
}

fn validate_contract_name(name: &str) -> Result<(), ContractError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == b'-')
    {
        return Err(ContractError::InvalidName(name.to_string()));
    }
    Ok(())
}

fn ensure_single_build_system(lock: &JavaContractLock) -> Result<(), ContractError> {
    if lock
        .contracts
        .values()
        .any(|entry| entry.build_system != BuildSystem::Maven)
    {
        return Err(ContractError::UnsupportedLockedBuildSystem);
    }
    Ok(())
}

fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        eprintln!("Java DTO exporter warning: {warning}");
    }
}

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("contract name {0:?} must contain only lowercase letters, digits, and '-'")]
    InvalidName(String),
    #[error(
        "contract {name:?} is registered for Java type {root_type:?}; use --force to replace it"
    )]
    OwnedByDifferentType { name: String, root_type: String },
    #[error("contract file {0} is not owned by the Java contract lock; use --force to replace it")]
    UnmanagedContract(PathBuf),
    #[error("Java contract lock contains a build system unsupported by this release")]
    UnsupportedLockedBuildSystem,
    #[error("generated DTO schema is invalid: {0}")]
    InvalidGeneratedContract(#[source] crate::error::GenerationError),
    #[error("Randomizer cannot generate a response from the extracted DTO schema: {0}")]
    UnsupportedGeneratedContract(#[source] crate::error::GenerationError),
    #[error("failed to read generated contract {path}: {source}")]
    ReadContract {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse generated contract {path}: {source}")]
    ParseContract {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("stale Java DTO contracts: {0:?}; run `randomizer contract refresh`")]
    Stale(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_portable_contract_names() {
        assert!(validate_contract_name("service-os-task").is_ok());
        assert!(validate_contract_name("Task_Response").is_err());
    }
}

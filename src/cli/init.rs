use std::path::Path;

use crate::project::{CURRENT_MANIFEST_VERSION, ProjectDefinition, ProjectManifest, ProjectPaths};

use super::{CliError, args::InitArgs, skill};

pub fn init(args: InitArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::for_init(&args.path)?;
    if paths.manifest.exists() {
        return Err(CliError::AlreadyInitialized(paths.manifest));
    }
    let manifest = empty_manifest(&paths.root);

    std::fs::create_dir_all(&paths.contracts).map_err(|source| CliError::Write {
        path: paths.contracts.clone(),
        source,
    })?;
    std::fs::create_dir_all(&paths.fixtures).map_err(|source| CliError::Write {
        path: paths.fixtures.clone(),
        source,
    })?;
    std::fs::create_dir_all(&paths.runtime).map_err(|source| CliError::Write {
        path: paths.runtime.clone(),
        source,
    })?;

    let skill_outcome = skill::sync_at_root(&paths.root, false)?;
    write_ignore_file(&paths)?;
    // Write the manifest last because its presence marks initialization as complete.
    write_yaml(&paths.manifest, &manifest)?;

    println!("initialized {}", paths.randomizer_dir.display());
    println!("{}", skill_outcome.message());
    println!(
        "review {} and add services and routes before starting Randomizer",
        paths.manifest.display()
    );
    Ok(())
}

fn empty_manifest(root: &Path) -> ProjectManifest {
    ProjectManifest {
        version: CURRENT_MANIFEST_VERSION,
        project: ProjectDefinition {
            name: root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("project")
                .to_string(),
            seed: 0,
            host: "127.0.0.1".into(),
            port: 7263,
            adapter: None,
        },
        services: Vec::new(),
        routes: Vec::new(),
    }
}

fn write_yaml(path: &Path, value: &impl serde::Serialize) -> Result<(), CliError> {
    let encoded = serde_yaml::to_string(value)?;
    std::fs::write(path, encoded).map_err(|source| CliError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn write_ignore_file(paths: &ProjectPaths) -> Result<(), CliError> {
    let path = paths.randomizer_dir.join(".gitignore");
    std::fs::write(&path, "runtime/\ncontracts.transaction.json\n")
        .map_err(|source| CliError::Write { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_http_project_without_external_discovery() {
        let directory = tempfile::tempdir().unwrap();

        init(InitArgs {
            path: directory.path().to_path_buf(),
        })
        .unwrap();

        let paths = ProjectPaths::discover(Some(directory.path())).unwrap();
        let manifest = paths.load_manifest().unwrap();
        assert_eq!(manifest.project.adapter, None);
        assert!(
            !std::fs::read_to_string(&paths.manifest)
                .unwrap()
                .contains("adapter:")
        );
        assert!(manifest.services.is_empty());
        assert!(manifest.routes.is_empty());
        assert!(paths.contracts.is_dir());
        assert!(paths.fixtures.is_dir());
        let installed_skill = std::fs::read_to_string(
            directory
                .path()
                .join(".agents/skills/randomizer-mocks/SKILL.md"),
        )
        .unwrap();
        assert!(installed_skill.contains("randomizer contract import"));
        assert!(installed_skill.contains("randomizer wiring apply"));
        assert!(installed_skill.contains("generic-wire-contract.md"));
        assert!(installed_skill.contains("versioned provider protocol"));
        assert!(
            directory
                .path()
                .join(".agents/skills/randomizer-mocks/references/contracts.md")
                .is_file()
        );
        assert!(
            directory
                .path()
                .join(".agents/skills/randomizer-mocks/references/runtime-capabilities.md")
                .is_file()
        );
        assert!(
            directory
                .path()
                .join(".agents/skills/randomizer-mocks/references/languages/java.md")
                .is_file()
        );
        let java_reference = std::fs::read_to_string(
            directory
                .path()
                .join(".agents/skills/randomizer-mocks/references/languages/java.md"),
        )
        .unwrap();
        assert!(java_reference.contains("protocol-v1 provider"));
        assert!(java_reference.contains("@JsonValue"));
        assert!(
            directory
                .path()
                .join(".randomizer/skills.lock.json")
                .is_file()
        );
        let skill_lock: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.path().join(".randomizer/skills.lock.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(skill_lock["skill_version"], 5);
        assert_eq!(skill_lock["files"].as_object().unwrap().len(), 10);
        assert_eq!(
            std::fs::read_to_string(paths.randomizer_dir.join(".gitignore")).unwrap(),
            "runtime/\ncontracts.transaction.json\n"
        );
    }
}

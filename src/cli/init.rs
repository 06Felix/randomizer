use std::path::Path;

use crate::{
    adapter::SpringBootAdapter,
    project::{CURRENT_MANIFEST_VERSION, ProjectDefinition, ProjectManifest, ProjectPaths},
};

use super::{CliError, args::InitArgs};

pub fn init(args: InitArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::for_init(&args.path)?;
    if paths.manifest.exists() {
        return Err(CliError::AlreadyInitialized(paths.manifest));
    }
    let manifest = empty_manifest(&paths.root, &args.adapter);

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

    if args.adapter == "spring-boot" && !args.no_apply {
        SpringBootAdapter::ensure_import(&paths)?;
    }
    write_yaml(&paths.manifest, &manifest)?;
    write_ignore_file(&paths)?;

    println!("initialized {}", paths.randomizer_dir.display());
    println!(
        "review {} and add route response contracts before starting the harness",
        paths.manifest.display()
    );
    Ok(())
}

fn empty_manifest(root: &Path, adapter: &str) -> ProjectManifest {
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
            adapter: Some(adapter.to_string()),
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
    std::fs::write(&path, "runtime/\n").map_err(|source| CliError::Write { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_http_project_without_external_discovery() {
        let directory = tempfile::tempdir().unwrap();

        init(InitArgs {
            path: directory.path().to_path_buf(),
            adapter: "spring-boot".into(),
            no_apply: true,
        })
        .unwrap();

        let paths = ProjectPaths::discover(Some(directory.path())).unwrap();
        let manifest = paths.load_manifest().unwrap();
        assert_eq!(manifest.project.adapter.as_deref(), Some("spring-boot"));
        assert!(manifest.services.is_empty());
        assert!(manifest.routes.is_empty());
        assert!(paths.contracts.is_dir());
        assert!(paths.fixtures.is_dir());
        assert_eq!(
            std::fs::read_to_string(paths.randomizer_dir.join(".gitignore")).unwrap(),
            "runtime/\n"
        );
    }
}

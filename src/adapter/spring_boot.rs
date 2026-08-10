use std::{
    io,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use thiserror::Error;

use crate::project::{ProjectManifest, ProjectPaths};

const IMPORT_VALUE: &str = "optional:file:.randomizer/runtime/application-randomizer.yaml";

pub struct SpringBootAdapter;

#[derive(Debug, Error)]
pub enum SpringBootAdapterError {
    #[error("no Spring Boot local configuration found under {root}/src/main/resources")]
    LocalConfigMissing { root: PathBuf },
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to encode generated Spring configuration: {0}")]
    Encode(#[source] serde_yaml::Error),
}

impl SpringBootAdapter {
    pub fn ensure_import(paths: &ProjectPaths) -> Result<PathBuf, SpringBootAdapterError> {
        let resources = paths.root.join("src/main/resources");
        let yaml = resources.join("application-local.yaml");
        let yml = resources.join("application-local.yml");
        let properties = resources.join("application-local.properties");
        if yaml.is_file() {
            ensure_yaml_import(&yaml)?;
            Ok(yaml)
        } else if yml.is_file() {
            ensure_yaml_import(&yml)?;
            Ok(yml)
        } else if properties.is_file() {
            ensure_properties_import(&properties)?;
            Ok(properties)
        } else {
            Err(SpringBootAdapterError::LocalConfigMissing {
                root: paths.root.clone(),
            })
        }
    }

    pub fn write_overlay(
        paths: &ProjectPaths,
        manifest: &ProjectManifest,
    ) -> Result<PathBuf, SpringBootAdapterError> {
        let mut overlay = Value::Object(Map::new());
        for service in &manifest.services {
            let Some(config_key) = service.config_key.as_deref() else {
                continue;
            };
            insert_property(
                &mut overlay,
                config_key,
                Value::String(format!(
                    "http://{}:{}/mock/{}",
                    manifest.project.host, manifest.project.port, service.id
                )),
            );
        }

        let encoded = serde_yaml::to_string(&overlay).map_err(SpringBootAdapterError::Encode)?;
        std::fs::create_dir_all(&paths.runtime).map_err(|source| {
            SpringBootAdapterError::Write {
                path: paths.runtime.clone(),
                source,
            }
        })?;
        let target = paths.runtime.join("application-randomizer.yaml");
        std::fs::write(&target, encoded).map_err(|source| SpringBootAdapterError::Write {
            path: target.clone(),
            source,
        })?;
        Ok(target)
    }
}

fn insert_property(root: &mut Value, key: &str, value: Value) {
    let mut current = root;
    let mut segments = key.split('.').peekable();
    while let Some(segment) = segments.next() {
        let object = current
            .as_object_mut()
            .expect("validated configuration paths cannot conflict");
        if segments.peek().is_none() {
            object.insert(segment.to_string(), value);
            return;
        }
        current = object
            .entry(segment)
            .or_insert_with(|| Value::Object(Map::new()));
    }
}

fn ensure_yaml_import(path: &Path) -> Result<(), SpringBootAdapterError> {
    let contents =
        std::fs::read_to_string(path).map_err(|source| SpringBootAdapterError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    if contents.contains(IMPORT_VALUE) {
        return Ok(());
    }
    let mut lines: Vec<String> = contents.lines().map(ToString::to_string).collect();
    if let Some(import_index) = lines
        .iter()
        .position(|line| line.starts_with("    import:"))
    {
        append_yaml_import(&mut lines, import_index);
    } else if let Some(config_index) = lines.iter().position(|line| line == "  config:")
        && lines[..=config_index]
            .iter()
            .rposition(|line| line == "spring:")
            .is_some()
    {
        lines.insert(config_index + 1, format!("    import: {IMPORT_VALUE}"));
    } else if let Some(spring_index) = lines.iter().position(|line| line == "spring:") {
        lines.insert(spring_index + 1, "  config:".to_string());
        lines.insert(spring_index + 2, format!("    import: {IMPORT_VALUE}"));
    } else {
        lines.splice(
            0..0,
            [
                "spring:".to_string(),
                "  config:".to_string(),
                format!("    import: {IMPORT_VALUE}"),
                String::new(),
            ],
        );
    }
    let mut output = lines.join("\n");
    output.push('\n');
    std::fs::write(path, output).map_err(|source| SpringBootAdapterError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn append_yaml_import(lines: &mut Vec<String>, import_index: usize) {
    let value = lines[import_index]
        .strip_prefix("    import:")
        .expect("caller selected an import line")
        .trim();
    if value.is_empty() {
        lines.insert(import_index + 1, format!("      - {IMPORT_VALUE}"));
    } else if let Some(list) = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        let separator = if list.trim().is_empty() { "" } else { ", " };
        lines[import_index] = format!("    import: [{list}{separator}{IMPORT_VALUE}]");
    } else {
        lines[import_index] = format!("    import: {value},{IMPORT_VALUE}");
    }
}

fn ensure_properties_import(path: &Path) -> Result<(), SpringBootAdapterError> {
    let mut contents =
        std::fs::read_to_string(path).map_err(|source| SpringBootAdapterError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    if contents.contains(IMPORT_VALUE) {
        return Ok(());
    }
    if let Some(existing) = contents
        .lines()
        .find(|line| line.starts_with("spring.config.import="))
    {
        let updated = format!("{existing},{IMPORT_VALUE}");
        contents = contents.replacen(existing, &updated, 1);
        return std::fs::write(path, contents).map_err(|source| SpringBootAdapterError::Write {
            path: path.to_path_buf(),
            source,
        });
    }
    if !contents.ends_with('\n') {
        contents.push('\n');
    }
    contents.push_str(&format!("spring.config.import={IMPORT_VALUE}\n"));
    std::fs::write(path, contents).map_err(|source| SpringBootAdapterError::Write {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{CURRENT_MANIFEST_VERSION, ProjectDefinition, ServiceDefinition};

    #[test]
    fn inserts_import_without_rewriting_existing_yaml() {
        let directory = tempfile::tempdir().unwrap();
        let resources = directory.path().join("src/main/resources");
        std::fs::create_dir_all(&resources).unwrap();
        let local = resources.join("application-local.yaml");
        std::fs::write(&local, "spring:\n  config:\n    activate:\n      on-profile: local\nurl:\n  service: original\n").unwrap();
        let paths = ProjectPaths::for_init(directory.path()).unwrap();

        SpringBootAdapter::ensure_import(&paths).unwrap();
        let updated = std::fs::read_to_string(local).unwrap();
        assert!(updated.contains(&format!("    import: {IMPORT_VALUE}")));
        assert!(updated.contains("      on-profile: local"));
        assert!(updated.contains("  service: original"));
    }

    #[test]
    fn appends_to_existing_yaml_import() {
        let directory = tempfile::tempdir().unwrap();
        let resources = directory.path().join("src/main/resources");
        std::fs::create_dir_all(&resources).unwrap();
        let local = resources.join("application-local.yaml");
        std::fs::write(
            &local,
            "spring:\n  config:\n    import: optional:configserver:http://localhost:8888\n",
        )
        .unwrap();
        let paths = ProjectPaths::for_init(directory.path()).unwrap();

        SpringBootAdapter::ensure_import(&paths).unwrap();
        let updated = std::fs::read_to_string(local).unwrap();
        assert!(updated.contains(&format!(
            "import: optional:configserver:http://localhost:8888,{IMPORT_VALUE}"
        )));
        assert_eq!(updated.matches("    import:").count(), 1);
    }

    #[test]
    fn appends_to_existing_properties_import() {
        let directory = tempfile::tempdir().unwrap();
        let resources = directory.path().join("src/main/resources");
        std::fs::create_dir_all(&resources).unwrap();
        let local = resources.join("application-local.properties");
        std::fs::write(
            &local,
            "spring.config.import=optional:classpath:base.yaml\n",
        )
        .unwrap();
        let paths = ProjectPaths::for_init(directory.path()).unwrap();

        SpringBootAdapter::ensure_import(&paths).unwrap();
        let updated = std::fs::read_to_string(local).unwrap();
        assert_eq!(
            updated,
            format!("spring.config.import=optional:classpath:base.yaml,{IMPORT_VALUE}\n")
        );
    }

    #[test]
    fn renders_http_overrides() {
        let directory = tempfile::tempdir().unwrap();
        let paths = ProjectPaths::for_init(directory.path()).unwrap();
        let manifest = ProjectManifest {
            version: CURRENT_MANIFEST_VERSION,
            project: ProjectDefinition {
                name: "app".into(),
                seed: 0,
                host: "127.0.0.1".into(),
                port: 7263,
                adapter: Some("spring-boot".into()),
            },
            services: vec![ServiceDefinition {
                id: "service-os".into(),
                config_key: Some("url.serviceOSBaseUrl".into()),
            }],
            routes: Vec::new(),
        };

        let output = SpringBootAdapter::write_overlay(&paths, &manifest).unwrap();
        let yaml = std::fs::read_to_string(output).unwrap();
        assert!(yaml.contains("serviceOSBaseUrl: http://127.0.0.1:7263/mock/service-os"));
    }
}

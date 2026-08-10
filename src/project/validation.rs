use std::{collections::HashSet, io, path::PathBuf};

use thiserror::Error;

use super::{CURRENT_MANIFEST_VERSION, ProjectManifest};

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("failed to determine the current directory: {0}")]
    CurrentDirectory(#[source] io::Error),
    #[error("failed to resolve project root {path}: {source}")]
    ProjectRoot {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("no .randomizer/randomizer.yaml found from {start}; run `randomizer init`")]
    NotInitialized { start: PathBuf },
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
    #[error("invalid manifest {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("unsupported manifest version {provided}; supported version is {supported}")]
    UnsupportedVersion { provided: u32, supported: u32 },
    #[error("invalid manifest: {0}")]
    Invalid(String),
}

pub fn validate_manifest(manifest: &ProjectManifest) -> Result<(), ManifestError> {
    if manifest.version != CURRENT_MANIFEST_VERSION {
        return Err(ManifestError::UnsupportedVersion {
            provided: manifest.version,
            supported: CURRENT_MANIFEST_VERSION,
        });
    }
    if manifest.project.name.trim().is_empty() {
        return Err(ManifestError::Invalid(
            "project.name must not be empty".into(),
        ));
    }
    if manifest.project.port == 0 {
        return Err(ManifestError::Invalid(
            "project.port must be greater than zero".into(),
        ));
    }
    if manifest
        .project
        .adapter
        .as_deref()
        .is_some_and(|adapter| adapter != "spring-boot")
    {
        return Err(ManifestError::Invalid(
            "project.adapter must be spring-boot when configured".into(),
        ));
    }
    let mut service_ids = HashSet::new();
    let mut config_keys = HashSet::new();
    for service in &manifest.services {
        validate_id("service", &service.id)?;
        if !service_ids.insert(service.id.as_str()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate service id {:?}",
                service.id
            )));
        }
        if let Some(config_key) = service.config_key.as_deref() {
            if config_key.split('.').any(|segment| {
                segment.is_empty()
                    || !segment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
            }) {
                return Err(ManifestError::Invalid(format!(
                    "service {:?} has invalid config_key {:?}",
                    service.id, config_key
                )));
            }
            if !config_keys.insert(config_key) {
                return Err(ManifestError::Invalid(format!(
                    "duplicate service config_key {config_key:?}"
                )));
            }
        }
    }
    for left in &config_keys {
        for right in &config_keys {
            if left != right
                && (left.starts_with(&format!("{right}."))
                    || right.starts_with(&format!("{left}.")))
            {
                return Err(ManifestError::Invalid(format!(
                    "service config_key paths {left:?} and {right:?} conflict"
                )));
            }
        }
    }

    let mut route_ids = HashSet::new();
    let mut matchers = HashSet::new();
    for route in &manifest.routes {
        validate_id("route", &route.id)?;
        if !route_ids.insert(route.id.as_str()) {
            return Err(ManifestError::Invalid(format!(
                "duplicate route id {:?}",
                route.id
            )));
        }
        if !service_ids.contains(route.service.as_str()) {
            return Err(ManifestError::Invalid(format!(
                "route {:?} references unknown service {:?}",
                route.id, route.service
            )));
        }
        if !route.request_match.path.starts_with('/') {
            return Err(ManifestError::Invalid(format!(
                "route {:?} path must start with '/'",
                route.id
            )));
        }
        if route.responses.is_empty() {
            return Err(ManifestError::Invalid(format!(
                "route {:?} must define a response",
                route.id
            )));
        }
        let matcher_key = format!(
            "{}|{}|{}|{:?}|{:?}|{:?}",
            route.service,
            route.request_match.method.as_deref().unwrap_or("*"),
            route.request_match.path,
            route.request_match.query,
            route.request_match.headers,
            route.request_match.body,
        );
        if !matchers.insert(matcher_key) {
            return Err(ManifestError::Invalid(format!(
                "route {:?} has an ambiguous duplicate matcher",
                route.id
            )));
        }
        for response in &route.responses {
            if !(100..=599).contains(&response.status) {
                return Err(ManifestError::Invalid(format!(
                    "route {:?} has invalid HTTP status {}",
                    route.id, response.status
                )));
            }
            let choices = usize::from(response.body.inline.is_some())
                + usize::from(response.body.fixture.is_some())
                + usize::from(response.body.contract.is_some());
            if choices > 1 {
                return Err(ManifestError::Invalid(format!(
                    "route {:?} response body must choose only one of inline, fixture, or contract",
                    route.id
                )));
            }
            if response.delay_ms > 60_000 {
                return Err(ManifestError::Invalid(format!(
                    "route {:?} delay_ms cannot exceed 60000",
                    route.id
                )));
            }
        }
    }

    Ok(())
}

fn validate_id(kind: &str, id: &str) -> Result<(), ManifestError> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ManifestError::Invalid(format!(
            "{kind} id {id:?} must contain only lowercase letters, digits, and '-'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{
        MatchDefinition, ProjectDefinition, ResponseBodyDefinition, ResponseDefinition,
        RouteDefinition, ServiceDefinition,
    };

    fn manifest() -> ProjectManifest {
        ProjectManifest {
            version: CURRENT_MANIFEST_VERSION,
            project: ProjectDefinition {
                name: "example".into(),
                seed: 0,
                host: "127.0.0.1".into(),
                port: 7263,
                adapter: None,
            },
            services: vec![ServiceDefinition {
                id: "service".into(),
                config_key: None,
            }],
            routes: vec![RouteDefinition {
                id: "get-item".into(),
                service: "service".into(),
                request_match: MatchDefinition {
                    method: Some("GET".into()),
                    path: "/items/{id}".into(),
                    ..MatchDefinition::default()
                },
                responses: vec![ResponseDefinition {
                    status: 200,
                    headers: Default::default(),
                    delay_ms: 0,
                    body: ResponseBodyDefinition {
                        inline: Some(serde_json::json!({"id": "1"})),
                        ..ResponseBodyDefinition::default()
                    },
                    bindings: Vec::new(),
                }],
            }],
        }
    }

    #[test]
    fn accepts_valid_manifest() {
        validate_manifest(&manifest()).unwrap();
    }

    #[test]
    fn rejects_duplicate_matchers_and_unsafe_delays() {
        let mut duplicate = manifest();
        duplicate.routes.push(RouteDefinition {
            id: "other-item".into(),
            ..duplicate.routes[0].clone()
        });
        assert!(validate_manifest(&duplicate).is_err());

        let mut delay = manifest();
        delay.routes[0].responses[0].delay_ms = 60_001;
        assert!(validate_manifest(&delay).is_err());
    }
}

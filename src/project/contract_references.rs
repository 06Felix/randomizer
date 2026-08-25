use std::collections::BTreeMap;

use thiserror::Error;

use super::{ContractError, ManagedContract, ProjectManifest, ProjectPaths, load_contract_lock};

#[derive(Debug, Error)]
pub enum ContractReferenceError {
    #[error(transparent)]
    Contract(#[from] ContractError),
    #[error("invalid managed contract name {0:?} in the contract lock")]
    InvalidName(String),
    #[error(
        "managed contract {name:?} has unexpected artifact path {artifact:?}; expected {expected:?}"
    )]
    InvalidArtifact {
        name: String,
        artifact: String,
        expected: String,
    },
    #[error("managed contracts {first:?} and {second:?} both claim artifact {artifact:?}")]
    DuplicateArtifact {
        first: String,
        second: String,
        artifact: String,
    },
    #[error(
        "managed contract {name:?} for {method} {path} status {status} is not referenced by any manifest response"
    )]
    Unreferenced {
        name: String,
        method: String,
        path: String,
        status: u16,
    },
    #[error(
        "route {route:?} response using managed contract {name:?} does not match its locked endpoint: {message}"
    )]
    EndpointMismatch {
        name: String,
        route: String,
        message: String,
    },
}

/// Ensures every managed response contract is actually attached to its locked HTTP endpoint.
///
/// `contract check` intentionally remains useful while a route is being assembled. Project-wide
/// `verify` calls this stricter association check before the application is started.
pub fn validate_managed_contract_references(
    paths: &ProjectPaths,
    manifest: &ProjectManifest,
) -> Result<(), ContractReferenceError> {
    let lock = load_contract_lock(paths)?;
    let mut by_artifact: BTreeMap<&str, (&str, &ManagedContract)> = BTreeMap::new();
    for (name, managed) in &lock.contracts {
        validate_name(name)?;
        let expected = format!(".randomizer/contracts/{name}.json");
        if managed.artifact != expected {
            return Err(ContractReferenceError::InvalidArtifact {
                name: name.clone(),
                artifact: managed.artifact.clone(),
                expected,
            });
        }
        if let Some((first, _)) = by_artifact.insert(&managed.artifact, (name, managed)) {
            return Err(ContractReferenceError::DuplicateArtifact {
                first: first.to_string(),
                second: name.clone(),
                artifact: managed.artifact.clone(),
            });
        }
    }

    let mut reference_counts = BTreeMap::new();
    for route in &manifest.routes {
        for response in &route.responses {
            let Some(artifact) = response.body.contract.as_deref() else {
                continue;
            };
            let Some((name, managed)) = by_artifact.get(artifact).copied() else {
                // Bare legacy contracts intentionally have no managed endpoint metadata.
                continue;
            };
            *reference_counts.entry(name).or_insert(0_usize) += 1;
            validate_reference(name, managed, route, response.status, &response.headers)?;
        }
    }

    for (name, managed) in &lock.contracts {
        if reference_counts.get(name.as_str()).copied().unwrap_or(0) == 0 {
            return Err(ContractReferenceError::Unreferenced {
                name: name.clone(),
                method: managed.endpoint.method.clone(),
                path: managed.endpoint.path.clone(),
                status: managed.endpoint.status,
            });
        }
    }
    Ok(())
}

fn validate_reference(
    name: &str,
    managed: &ManagedContract,
    route: &super::RouteDefinition,
    response_status: u16,
    response_headers: &BTreeMap<String, String>,
) -> Result<(), ContractReferenceError> {
    let actual_method = route.request_match.method.as_deref().unwrap_or("ANY");
    if actual_method != managed.endpoint.method {
        return mismatch(
            name,
            &route.id,
            format!(
                "method is {actual_method:?}, locked method is {:?}",
                managed.endpoint.method
            ),
        );
    }
    if route.request_match.path != managed.endpoint.path {
        return mismatch(
            name,
            &route.id,
            format!(
                "path is {:?}, locked path is {:?}",
                route.request_match.path, managed.endpoint.path
            ),
        );
    }
    if response_status != managed.endpoint.status {
        return mismatch(
            name,
            &route.id,
            format!(
                "status is {response_status}, locked status is {}",
                managed.endpoint.status
            ),
        );
    }
    if let Some(expected_media_type) = managed.endpoint.media_type.as_deref() {
        let actual_media_type = response_headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            .map(|(_, value)| value.as_str())
            .unwrap_or("application/json");
        if media_type_essence(actual_media_type) != media_type_essence(expected_media_type) {
            return mismatch(
                name,
                &route.id,
                format!(
                    "response media type is {actual_media_type:?}, locked media type is {expected_media_type:?}"
                ),
            );
        }
    }
    Ok(())
}

fn media_type_essence(media_type: &str) -> String {
    media_type
        .split(';')
        .next()
        .unwrap_or(media_type)
        .trim()
        .to_ascii_lowercase()
}

fn mismatch(name: &str, route: &str, message: String) -> Result<(), ContractReferenceError> {
    Err(ContractReferenceError::EndpointMismatch {
        name: name.to_string(),
        route: route.to_string(),
        message,
    })
}

fn validate_name(name: &str) -> Result<(), ContractReferenceError> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ContractReferenceError::InvalidName(name.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        project::{
            CURRENT_MANIFEST_VERSION, MatchDefinition, ProjectDefinition, ResponseBodyDefinition,
            ResponseDefinition, RouteDefinition, ServiceDefinition,
        },
        provider::{EndpointSelector, ProviderIdentity},
    };

    fn managed(endpoint: EndpointSelector) -> ManagedContract {
        ManagedContract {
            artifact: ".randomizer/contracts/get-user-200.json".into(),
            contract_version: "1".into(),
            schema_hash: "hash".into(),
            provider: ProviderIdentity::new("test", "1"),
            endpoint,
            root_symbol: None,
            source_fingerprints: Vec::new(),
            evidence: Vec::new(),
            diagnostics: Vec::new(),
            recipe: super::super::ContractRecipe::JsonSchema {
                source: "schema.json".into(),
                endpoint: EndpointSelector::new("GET", "/users/{id}", 200),
            },
        }
    }

    fn manifest(method: &str, path: &str, status: u16) -> ProjectManifest {
        ProjectManifest {
            version: CURRENT_MANIFEST_VERSION,
            project: ProjectDefinition {
                name: "test".into(),
                seed: 0,
                host: "127.0.0.1".into(),
                port: 7263,
                adapter: None,
            },
            services: vec![ServiceDefinition {
                id: "users".into(),
                config_key: None,
                wiring: Vec::new(),
            }],
            routes: vec![RouteDefinition {
                id: "get-user".into(),
                service: "users".into(),
                request_match: MatchDefinition {
                    method: Some(method.into()),
                    path: path.into(),
                    ..MatchDefinition::default()
                },
                responses: vec![ResponseDefinition {
                    status,
                    headers: BTreeMap::new(),
                    delay_ms: 0,
                    body: ResponseBodyDefinition {
                        contract: Some(".randomizer/contracts/get-user-200.json".into()),
                        ..ResponseBodyDefinition::default()
                    },
                    bindings: Vec::new(),
                }],
            }],
        }
    }

    #[test]
    fn validates_endpoint_dimensions() {
        let managed = managed(EndpointSelector::new("GET", "/users/{id}", 200));
        let matching = manifest("GET", "/users/{id}", 200);
        validate_reference(
            "get-user-200",
            &managed,
            &matching.routes[0],
            200,
            &BTreeMap::new(),
        )
        .unwrap();

        for (method, path, status, expected) in [
            ("POST", "/users/{id}", 200, "method"),
            ("GET", "/orders/{id}", 200, "path"),
            ("GET", "/users/{id}", 201, "status"),
        ] {
            let invalid = manifest(method, path, status);
            let error = validate_reference(
                "get-user-200",
                &managed,
                &invalid.routes[0],
                status,
                &BTreeMap::new(),
            )
            .unwrap_err()
            .to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn validates_locked_response_media_type() {
        let managed = managed(
            EndpointSelector::new("GET", "/users/{id}", 200)
                .with_media_type("application/problem+json"),
        );
        let manifest = manifest("GET", "/users/{id}", 200);
        let mut headers = BTreeMap::from([(
            "Content-Type".to_string(),
            "application/problem+json; charset=utf-8".to_string(),
        )]);

        validate_reference("get-user-200", &managed, &manifest.routes[0], 200, &headers).unwrap();
        headers.insert("Content-Type".into(), "application/json".into());
        let error =
            validate_reference("get-user-200", &managed, &manifest.routes[0], 200, &headers)
                .unwrap_err()
                .to_string();
        assert!(error.contains("media type"), "{error}");
    }
}

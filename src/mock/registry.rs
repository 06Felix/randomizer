use std::{
    collections::{BTreeMap, HashSet},
    fs, io,
    path::PathBuf,
};

use axum::http::{HeaderName, HeaderValue, Method};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    generation::GenerationOptions,
    project::{
        BindingDefinition, MatchDefinition, ProjectManifest, ProjectPaths, ResponseDefinition,
    },
    schema::JsonSchemaContract,
    standard::{StandardGenerationPlan, validate_standard_value},
};

use super::{ScenarioStore, bindings::apply_bindings};

#[derive(Debug, Clone)]
pub struct MockRequest {
    pub method: String,
    pub service: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<Value>,
}

#[derive(Debug)]
pub struct MockResponse {
    pub route_id: String,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub delay_ms: u64,
    pub body: Option<Value>,
}

pub struct CompiledMockRegistry {
    routes: Vec<CompiledRoute>,
}

struct CompiledRoute {
    id: String,
    service: String,
    method: Option<String>,
    path_regex: Regex,
    path_parameters: Vec<String>,
    query: BTreeMap<String, String>,
    headers: BTreeMap<String, String>,
    body: BTreeMap<String, Value>,
    responses: Vec<CompiledResponse>,
}

struct CompiledResponse {
    status: u16,
    headers: BTreeMap<String, String>,
    delay_ms: u64,
    body: CompiledBody,
    bindings: Vec<BindingDefinition>,
}

enum CompiledBody {
    Empty,
    Static(Value),
    Contract {
        plan: Box<StandardGenerationPlan>,
        contract: JsonSchemaContract,
        mode: crate::generation::GenerationMode,
    },
}

#[derive(Debug, Error)]
pub enum MockCompileError {
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid JSON in {path}: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("route {route_id:?} has invalid path template {path:?}: {reason}")]
    PathTemplate {
        route_id: String,
        path: String,
        reason: String,
    },
    #[error("route {route_id:?} has invalid contract {path}: {source}")]
    Contract {
        route_id: String,
        path: PathBuf,
        #[source]
        source: crate::error::GenerationError,
    },
    #[error("route {route_id:?} references a file outside the project: {path}")]
    UnsafePath { route_id: String, path: PathBuf },
    #[error("route {route_id:?} binding failed: {source}")]
    Binding {
        route_id: String,
        #[source]
        source: super::bindings::BindingError,
    },
    #[error("route {route_id:?} generated an invalid bound response: {violations}")]
    InvalidBoundResponse {
        route_id: String,
        violations: String,
    },
    #[error("route {route_id:?} generation failed: {source}")]
    Generation {
        route_id: String,
        #[source]
        source: crate::error::GenerationError,
    },
    #[error("request matched multiple routes: {route_ids}")]
    AmbiguousRequest { route_ids: String },
    #[error("route {route_id:?} has invalid HTTP method {method:?}")]
    InvalidMethod { route_id: String, method: String },
    #[error("route {route_id:?} has invalid response header {name:?}: {reason}")]
    InvalidHeader {
        route_id: String,
        name: String,
        reason: String,
    },
}

impl CompiledMockRegistry {
    pub fn compile(
        manifest: &ProjectManifest,
        paths: &ProjectPaths,
    ) -> Result<Self, MockCompileError> {
        let services: HashSet<_> = manifest
            .services
            .iter()
            .map(|service| service.id.as_str())
            .collect();
        let mut routes = Vec::with_capacity(manifest.routes.len());
        for route in &manifest.routes {
            if !services.contains(route.service.as_str()) {
                continue;
            }
            let (path_regex, path_parameters) = compile_path(&route.id, &route.request_match)?;
            let responses = route
                .responses
                .iter()
                .map(|response| compile_response(manifest, paths, &route.id, response))
                .collect::<Result<Vec<_>, _>>()?;
            let method = route
                .request_match
                .method
                .as_ref()
                .map(|method| {
                    Method::from_bytes(method.as_bytes())
                        .map(|method| method.as_str().to_string())
                        .map_err(|_| MockCompileError::InvalidMethod {
                            route_id: route.id.clone(),
                            method: method.clone(),
                        })
                })
                .transpose()?;
            routes.push(CompiledRoute {
                id: route.id.clone(),
                service: route.service.clone(),
                method,
                path_regex,
                path_parameters,
                query: route.request_match.query.clone(),
                headers: route
                    .request_match
                    .headers
                    .iter()
                    .map(|(key, value)| (key.to_ascii_lowercase(), value.clone()))
                    .collect(),
                body: route.request_match.body.clone(),
                responses,
            });
        }
        routes.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { routes })
    }

    pub fn route_ids(&self) -> Vec<&str> {
        self.routes.iter().map(|route| route.id.as_str()).collect()
    }

    pub fn respond(
        &self,
        request: &MockRequest,
        scenarios: &ScenarioStore,
    ) -> Result<Option<MockResponse>, MockCompileError> {
        let matches: Vec<_> = self
            .routes
            .iter()
            .filter_map(|route| route.matches(request).map(|parameters| (route, parameters)))
            .collect();
        if matches.len() > 1 {
            return Err(MockCompileError::AmbiguousRequest {
                route_ids: matches
                    .iter()
                    .map(|(route, _)| route.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
        if let Some((route, path_parameters)) = matches.into_iter().next() {
            let (response_index, sequence) = scenarios.next(&route.id, route.responses.len());
            let response = &route.responses[response_index];
            let mut body = match &response.body {
                CompiledBody::Empty => None,
                CompiledBody::Static(value) => Some(value.clone()),
                CompiledBody::Contract { plan, .. } => Some(
                    plan.generate(request_sequence(&route.id, request, sequence))
                        .map_err(|source| MockCompileError::Generation {
                            route_id: route.id.clone(),
                            source,
                        })?
                        .value,
                ),
            };
            if let Some(body) = body.as_mut() {
                apply_bindings(body, &response.bindings, request, &path_parameters).map_err(
                    |source| MockCompileError::Binding {
                        route_id: route.id.clone(),
                        source,
                    },
                )?;
                if let CompiledBody::Contract { contract, mode, .. } = &response.body
                    && *mode != crate::generation::GenerationMode::Invalid
                {
                    let report =
                        validate_standard_value(contract.clone(), body).map_err(|source| {
                            MockCompileError::Generation {
                                route_id: route.id.clone(),
                                source,
                            }
                        })?;
                    if !report.valid {
                        return Err(MockCompileError::InvalidBoundResponse {
                            route_id: route.id.clone(),
                            violations: report
                                .violations
                                .into_iter()
                                .map(|violation| violation.message)
                                .collect::<Vec<_>>()
                                .join("; "),
                        });
                    }
                }
            }
            Ok(Some(MockResponse {
                route_id: route.id.clone(),
                status: response.status,
                headers: response.headers.clone(),
                delay_ms: response.delay_ms,
                body,
            }))
        } else {
            Ok(None)
        }
    }
}

impl CompiledRoute {
    fn matches(&self, request: &MockRequest) -> Option<BTreeMap<String, String>> {
        if self.service != request.service
            || self
                .method
                .as_ref()
                .is_some_and(|method| method != &request.method)
            || self
                .query
                .iter()
                .any(|(key, value)| request.query.get(key) != Some(value))
            || self
                .headers
                .iter()
                .any(|(key, value)| request.headers.get(key) != Some(value))
            || self.body.iter().any(|(pointer, expected)| {
                request.body.as_ref().and_then(|body| body.pointer(pointer)) != Some(expected)
            })
        {
            return None;
        }
        let captures = self.path_regex.captures(&request.path)?;
        Some(
            self.path_parameters
                .iter()
                .filter_map(|name| {
                    captures
                        .name(name)
                        .map(|value| (name.clone(), value.as_str().to_string()))
                })
                .collect(),
        )
    }
}

fn compile_path(
    route_id: &str,
    definition: &MatchDefinition,
) -> Result<(Regex, Vec<String>), MockCompileError> {
    let mut pattern = String::from("^");
    let mut parameters = Vec::new();
    for segment in definition.path.split_inclusive('/') {
        let value = segment.strip_suffix('/').unwrap_or(segment);
        let has_slash = segment.ends_with('/');
        if value.starts_with('{') && value.ends_with('}') {
            let name = &value[1..value.len() - 1];
            if name.is_empty()
                || !name.as_bytes()[0].is_ascii_alphabetic()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || parameters.iter().any(|existing| existing == name)
            {
                return Err(MockCompileError::PathTemplate {
                    route_id: route_id.to_string(),
                    path: definition.path.clone(),
                    reason: format!("invalid or duplicate parameter {name:?}"),
                });
            }
            pattern.push_str(&format!("(?P<{name}>[^/]+)"));
            parameters.push(name.to_string());
        } else if value == "*" {
            pattern.push_str(".*");
        } else {
            pattern.push_str(&regex::escape(value));
        }
        if has_slash {
            pattern.push('/');
        }
    }
    pattern.push('$');
    Regex::new(&pattern)
        .map(|regex| (regex, parameters))
        .map_err(|error| MockCompileError::PathTemplate {
            route_id: route_id.to_string(),
            path: definition.path.clone(),
            reason: error.to_string(),
        })
}

fn compile_response(
    manifest: &ProjectManifest,
    paths: &ProjectPaths,
    route_id: &str,
    definition: &ResponseDefinition,
) -> Result<CompiledResponse, MockCompileError> {
    for (name, value) in &definition.headers {
        HeaderName::try_from(name).map_err(|error| MockCompileError::InvalidHeader {
            route_id: route_id.to_string(),
            name: name.clone(),
            reason: error.to_string(),
        })?;
        HeaderValue::try_from(value).map_err(|error| MockCompileError::InvalidHeader {
            route_id: route_id.to_string(),
            name: name.clone(),
            reason: error.to_string(),
        })?;
    }
    let body =
        if let Some(value) = &definition.body.inline {
            CompiledBody::Static(value.clone())
        } else if let Some(path) = &definition.body.fixture {
            CompiledBody::Static(read_json(paths, route_id, path)?)
        } else if let Some(path) = &definition.body.contract {
            let path_buf = safe_project_path(paths, route_id, path)?;
            let value = read_json(paths, route_id, path)?;
            let contract = serde_json::from_value::<JsonSchemaContract>(value.clone())
                .unwrap_or_else(|_| JsonSchemaContract {
                    name: route_id.to_string(),
                    version: "1".to_string(),
                    source: path.to_string(),
                    schema: value,
                    content_hash: None,
                });
            let plan = StandardGenerationPlan::compile(
                contract.clone(),
                definition.body.mode,
                &GenerationOptions {
                    seed: Some(manifest.project.seed),
                    ..GenerationOptions::default()
                },
            )
            .map_err(|source| MockCompileError::Contract {
                route_id: route_id.to_string(),
                path: path_buf.clone(),
                source,
            })?;
            plan.generate(0)
                .map_err(|source| MockCompileError::Contract {
                    route_id: route_id.to_string(),
                    path: path_buf,
                    source,
                })?;
            CompiledBody::Contract {
                plan: Box::new(plan),
                contract,
                mode: definition.body.mode,
            }
        } else {
            CompiledBody::Empty
        };
    Ok(CompiledResponse {
        status: definition.status,
        headers: definition.headers.clone(),
        delay_ms: definition.delay_ms,
        body,
        bindings: definition.bindings.clone(),
    })
}

fn read_json(paths: &ProjectPaths, route_id: &str, path: &str) -> Result<Value, MockCompileError> {
    let path = safe_project_path(paths, route_id, path)?;
    let bytes = fs::read(&path).map_err(|source| MockCompileError::Read {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| MockCompileError::Json { path, source })
}

fn safe_project_path(
    paths: &ProjectPaths,
    route_id: &str,
    value: &str,
) -> Result<PathBuf, MockCompileError> {
    let joined = paths.root.join(value);
    let canonical = joined
        .canonicalize()
        .map_err(|source| MockCompileError::Read {
            path: joined.clone(),
            source,
        })?;
    if !canonical.starts_with(&paths.root) {
        return Err(MockCompileError::UnsafePath {
            route_id: route_id.to_string(),
            path: canonical,
        });
    }
    Ok(canonical)
}

fn request_sequence(route_id: &str, request: &MockRequest, scenario_sequence: u64) -> u64 {
    let mut digest = Sha256::new();
    digest.update(route_id.as_bytes());
    digest.update([0]);
    digest.update(request.method.as_bytes());
    digest.update([0]);
    digest.update(request.path.as_bytes());
    digest.update([0]);
    digest.update(scenario_sequence.to_le_bytes());
    for (key, value) in &request.query {
        digest.update(key.as_bytes());
        digest.update([0]);
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    u64::from_le_bytes(digest.finalize()[..8].try_into().expect("sha256 prefix"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_path_parameters() {
        let definition = MatchDefinition {
            path: "/api/tasks/{taskId}".into(),
            ..MatchDefinition::default()
        };
        let (regex, parameters) = compile_path("task", &definition).unwrap();
        assert_eq!(parameters, vec!["taskId"]);
        assert_eq!(
            regex
                .captures("/api/tasks/42")
                .unwrap()
                .name("taskId")
                .unwrap()
                .as_str(),
            "42"
        );
        assert!(!regex.is_match("/api/tasks/42/more"));
    }
}

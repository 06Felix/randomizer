use std::{
    collections::{HashMap, HashSet},
    io,
    path::{Component, Path, PathBuf},
};

use thiserror::Error;

use super::{CURRENT_MANIFEST_VERSION, ProjectManifest, WiringFormat, WiringTarget};

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
    #[error("unsupported manifest version {provided}; supported versions are 1 and {supported}")]
    UnsupportedVersion { provided: u32, supported: u32 },
    #[error("invalid manifest: {0}")]
    Invalid(String),
}

pub fn validate_manifest(manifest: &ProjectManifest) -> Result<(), ManifestError> {
    if !matches!(manifest.version, 1 | CURRENT_MANIFEST_VERSION) {
        return Err(ManifestError::UnsupportedVersion {
            provided: manifest.version,
            supported: CURRENT_MANIFEST_VERSION,
        });
    }
    if manifest.version == 1
        && manifest
            .services
            .iter()
            .any(|service| !service.wiring.is_empty())
    {
        return Err(ManifestError::Invalid(
            "manifest version 1 does not support services[].wiring; set `version: 2` and declare service_base_safety and service_base_path_behavior for every service_base_url target".into(),
        ));
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
    if manifest.project.adapter.is_some() {
        return Err(ManifestError::Invalid(
            "project.adapter is no longer supported; remove it and configure the application's local service URLs to use http://127.0.0.1:<port>/mock/<service-id>".into(),
        ));
    }
    let mut service_ids = HashSet::new();
    let mut config_keys = HashSet::new();
    let mut wiring_selectors = HashSet::new();
    let mut wiring_file_formats = HashMap::new();
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
        for wiring in &service.wiring {
            validate_wiring_path(&service.id, &wiring.file)?;
            validate_wiring_selector(&service.id, &wiring.file, wiring.format, &wiring.selector)?;
            if let Some(existing) = wiring_file_formats.insert(&wiring.file, wiring.format)
                && existing != wiring.format
            {
                return Err(ManifestError::Invalid(format!(
                    "wiring file {:?} is declared with multiple formats",
                    wiring.file
                )));
            }
            if !wiring_selectors.insert((&wiring.file, &wiring.selector)) {
                return Err(ManifestError::Invalid(format!(
                    "duplicate wiring selector {:?} in file {:?}",
                    wiring.selector, wiring.file
                )));
            }
            match wiring.target {
                WiringTarget::ServiceBaseUrl => {
                    if wiring.route.is_some() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets service_base_url and must not set route",
                            service.id, wiring.file
                        )));
                    }
                    if wiring.service_base_safety.is_none() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets service_base_url and must set service_base_safety to dedicated_setting or all_calls_mocked",
                            service.id, wiring.file
                        )));
                    }
                    if wiring.service_base_path_behavior.is_none() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets service_base_url and must set service_base_path_behavior to preserves_prefix after verifying the application HTTP client preserves the configured base-path prefix",
                            service.id, wiring.file
                        )));
                    }
                }
                WiringTarget::RouteUrl => {
                    if wiring.route.is_none() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets route_url and must set route",
                            service.id, wiring.file
                        )));
                    }
                    if wiring.service_base_safety.is_some() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets route_url and must not set service_base_safety",
                            service.id, wiring.file
                        )));
                    }
                    if wiring.service_base_path_behavior.is_some() {
                        return Err(ManifestError::Invalid(format!(
                            "service {:?} wiring for {:?} targets route_url and must not set service_base_path_behavior",
                            service.id, wiring.file
                        )));
                    }
                }
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
        if route.request_match.path.contains('?')
            || route.request_match.path.contains('#')
            || route
                .request_match
                .path
                .bytes()
                .any(|byte| byte.is_ascii_control())
        {
            return Err(ManifestError::Invalid(format!(
                "route {:?} path must contain only the HTTP path; declare query requirements in match.query and omit fragments/control characters",
                route.id
            )));
        }
        if route.responses.is_empty() {
            return Err(ManifestError::Invalid(format!(
                "route {:?} must define a response",
                route.id
            )));
        }
        validate_unique_header_names(
            &route.id,
            "request matcher",
            route.request_match.headers.keys(),
        )?;
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
            validate_unique_header_names(&route.id, "response", response.headers.keys())?;
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

    for service in &manifest.services {
        for wiring in &service.wiring {
            let Some(route_id) = wiring.route.as_deref() else {
                continue;
            };
            let Some(route) = manifest.routes.iter().find(|route| route.id == route_id) else {
                return Err(ManifestError::Invalid(format!(
                    "service {:?} wiring for {:?} references unknown route {:?}",
                    service.id, wiring.file, route_id
                )));
            };
            if route.service != service.id {
                return Err(ManifestError::Invalid(format!(
                    "service {:?} wiring for {:?} references route {:?} owned by service {:?}",
                    service.id, wiring.file, route_id, route.service
                )));
            }
            if wiring.target == WiringTarget::RouteUrl
                && route.request_match.path.split('/').any(|segment| {
                    segment == "*" || (segment.starts_with('{') && segment.ends_with('}'))
                })
            {
                return Err(ManifestError::Invalid(format!(
                    "service {:?} route_url wiring for {:?} references dynamic route {:?} with path {:?}; route_url requires a static path, so use service_base_url or an application setting with explicit concrete substitutions",
                    service.id, wiring.file, route_id, route.request_match.path
                )));
            }
        }
    }

    Ok(())
}

fn validate_unique_header_names<'a>(
    route_id: &str,
    context: &str,
    names: impl Iterator<Item = &'a String>,
) -> Result<(), ManifestError> {
    let mut normalized = HashSet::new();
    for name in names {
        let lowercase = name.to_ascii_lowercase();
        if !normalized.insert(lowercase) {
            return Err(ManifestError::Invalid(format!(
                "route {route_id:?} {context} contains duplicate case-insensitive header name {name:?}"
            )));
        }
    }
    Ok(())
}

fn validate_wiring_path(service_id: &str, file: &str) -> Result<(), ManifestError> {
    let path = Path::new(file);
    if file.is_empty()
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ManifestError::Invalid(format!(
            "service {service_id:?} wiring file {file:?} must be a project-relative path without '.' or '..' components"
        )));
    }
    if path
        .components()
        .next()
        .is_some_and(|component| component.as_os_str() == ".randomizer")
    {
        return Err(ManifestError::Invalid(format!(
            "service {service_id:?} wiring file {file:?} must be application configuration outside .randomizer"
        )));
    }
    Ok(())
}

fn validate_wiring_selector(
    service_id: &str,
    file: &str,
    format: WiringFormat,
    selector: &str,
) -> Result<(), ManifestError> {
    let valid = match format {
        WiringFormat::Dotenv => {
            let mut bytes = selector.bytes();
            matches!(bytes.next(), Some(byte) if byte.is_ascii_alphabetic() || byte == b'_')
                && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        }
        WiringFormat::Properties => {
            !selector.is_empty()
                && selector
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        }
        WiringFormat::Json => valid_json_pointer(selector),
        WiringFormat::Yaml => {
            !selector.is_empty()
                && selector.split('.').all(|segment| {
                    !segment.is_empty()
                        && segment
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
                })
        }
    };
    if !valid {
        return Err(ManifestError::Invalid(format!(
            "service {service_id:?} wiring for {file:?} has invalid {format:?} selector {selector:?}"
        )));
    }
    Ok(())
}

fn valid_json_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') {
        return false;
    }
    let mut bytes = pointer.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'~' && !matches!(bytes.next(), Some(b'0' | b'1')) {
            return false;
        }
    }
    true
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
                wiring: Vec::new(),
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

        let mut legacy = manifest();
        legacy.version = 1;
        validate_manifest(&legacy).unwrap();
    }

    #[test]
    fn requires_v2_for_structured_wiring() {
        use crate::project::{WiringDefinition, WiringFormat, WiringTarget};

        let mut legacy = manifest();
        legacy.version = 1;
        legacy.services[0].wiring.push(WiringDefinition {
            file: ".env.local".into(),
            format: WiringFormat::Dotenv,
            selector: "SERVICE_URL".into(),
            target: WiringTarget::RouteUrl,
            route: Some("get-item".into()),
            service_base_safety: None,
            service_base_path_behavior: None,
        });

        let error = validate_manifest(&legacy).unwrap_err().to_string();

        assert!(error.contains("version 1 does not support services[].wiring"));
        assert!(error.contains("version: 2"));
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

        for path in ["/items?active=true", "/items#fragment", "/items\n"] {
            let mut invalid_path = manifest();
            invalid_path.routes[0].request_match.path = path.into();
            let error = validate_manifest(&invalid_path).unwrap_err().to_string();
            assert!(error.contains("only the HTTP path"), "{error}");
        }
    }

    #[test]
    fn rejects_case_insensitive_duplicate_header_names() {
        let mut request = manifest();
        request.routes[0]
            .request_match
            .headers
            .insert("X-Client".into(), "one".into());
        request.routes[0]
            .request_match
            .headers
            .insert("x-client".into(), "two".into());
        let error = validate_manifest(&request).unwrap_err().to_string();
        assert!(
            error.contains("duplicate case-insensitive header"),
            "{error}"
        );

        let mut response = manifest();
        response.routes[0].responses[0]
            .headers
            .insert("Content-Type".into(), "application/json".into());
        response.routes[0].responses[0]
            .headers
            .insert("content-type".into(), "text/plain".into());
        let error = validate_manifest(&response).unwrap_err().to_string();
        assert!(
            error.contains("duplicate case-insensitive header"),
            "{error}"
        );
    }

    #[test]
    fn rejects_legacy_application_adapters_with_migration_guidance() {
        let mut legacy = manifest();
        legacy.project.adapter = Some("spring-boot".into());

        let error = validate_manifest(&legacy).unwrap_err().to_string();

        assert!(error.contains("project.adapter is no longer supported"));
        assert!(error.contains("/mock/<service-id>"));
    }

    #[test]
    fn rejects_invalid_wiring_target_combinations_and_routes() {
        use crate::project::{WiringDefinition, WiringFormat, WiringTarget};

        let mut invalid = manifest();
        invalid.services[0].wiring.push(WiringDefinition {
            file: ".env.local".into(),
            format: WiringFormat::Dotenv,
            selector: "SERVICE_URL".into(),
            target: WiringTarget::RouteUrl,
            route: None,
            service_base_safety: None,
            service_base_path_behavior: None,
        });
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("must set route")
        );

        invalid.services[0].wiring[0].route = Some("missing-route".into());
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("unknown route")
        );

        invalid.services[0].wiring[0].route = Some("get-item".into());
        let error = validate_manifest(&invalid).unwrap_err().to_string();
        assert!(error.contains("route_url requires a static path"));

        invalid.routes[0].request_match.path = "/items/static".into();
        validate_manifest(&invalid).unwrap();

        invalid.services[0].wiring[0].service_base_safety =
            Some(super::super::manifest::ServiceBaseSafety::DedicatedSetting);
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("must not set service_base_safety")
        );

        invalid.services[0].wiring[0].service_base_safety = None;
        invalid.services[0].wiring[0].service_base_path_behavior =
            Some(super::super::manifest::ServiceBasePathBehavior::PreservesPrefix);
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("must not set service_base_path_behavior")
        );
    }

    #[test]
    fn service_base_url_requires_explicit_safety_and_path_assertions() {
        use crate::project::{WiringDefinition, WiringFormat, WiringTarget};

        let mut invalid = manifest();
        invalid.services[0].wiring.push(WiringDefinition {
            file: ".env.local".into(),
            format: WiringFormat::Dotenv,
            selector: "SERVICE_URL".into(),
            target: WiringTarget::ServiceBaseUrl,
            route: None,
            service_base_safety: None,
            service_base_path_behavior: None,
        });

        let error = validate_manifest(&invalid).unwrap_err().to_string();

        assert!(error.contains("must set service_base_safety"));
        assert!(error.contains("dedicated_setting"));
        assert!(error.contains("all_calls_mocked"));

        invalid.services[0].wiring[0].service_base_safety =
            Some(super::super::manifest::ServiceBaseSafety::AllCallsMocked);
        let error = validate_manifest(&invalid).unwrap_err().to_string();
        assert!(error.contains("must set service_base_path_behavior"));
        assert!(error.contains("preserves_prefix"));

        invalid.services[0].wiring[0].service_base_path_behavior =
            Some(super::super::manifest::ServiceBasePathBehavior::PreservesPrefix);
        validate_manifest(&invalid).unwrap();
    }

    #[test]
    fn rejects_unsafe_paths_invalid_selectors_and_duplicate_locations() {
        use crate::project::{WiringDefinition, WiringFormat, WiringTarget};

        let mut invalid = manifest();
        invalid.services[0].wiring.push(WiringDefinition {
            file: "../outside.env".into(),
            format: WiringFormat::Dotenv,
            selector: "SERVICE_URL".into(),
            target: WiringTarget::ServiceBaseUrl,
            route: None,
            service_base_safety: Some(super::super::manifest::ServiceBaseSafety::DedicatedSetting),
            service_base_path_behavior: Some(
                super::super::manifest::ServiceBasePathBehavior::PreservesPrefix,
            ),
        });
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("project-relative path")
        );

        invalid.services[0].wiring[0].file = ".env.local".into();
        invalid.services[0].wiring[0].selector = "invalid-key".into();
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("invalid Dotenv selector")
        );

        invalid.services[0].wiring[0].selector = "SERVICE_URL".into();
        let duplicate = invalid.services[0].wiring[0].clone();
        invalid.services[0].wiring.push(duplicate);
        assert!(
            validate_manifest(&invalid)
                .unwrap_err()
                .to_string()
                .contains("duplicate wiring selector")
        );

        invalid.services[0].wiring.truncate(1);
        invalid.services[0].wiring[0].file = ".randomizer/randomizer.yaml".into();
        let error = validate_manifest(&invalid).unwrap_err().to_string();
        assert!(error.contains("outside .randomizer"), "{error}");
    }
}

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    ops::Range,
    path::{Path, PathBuf},
};

use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use tempfile::NamedTempFile;
use thiserror::Error;

use super::{
    ManifestError, ProjectManifest, ServiceDefinition, WiringDefinition, WiringFormat,
    WiringTarget, validate_manifest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiringReport {
    pub entries: Vec<WiringEntryReport>,
}

impl WiringReport {
    pub fn changed_count(&self) -> usize {
        self.entries.iter().filter(|entry| entry.changed).count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiringEntryReport {
    pub service: String,
    pub file: String,
    pub selector: String,
    pub value: String,
    pub changed: bool,
}

#[derive(Debug, Error)]
pub enum WiringError {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("unknown service {0:?}")]
    UnknownService(String),
    #[error("failed to resolve project root {path}: {source}")]
    ProjectRoot {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to resolve wiring file {file:?}: {source}")]
    ResolveFile {
        file: String,
        #[source]
        source: io::Error,
    },
    #[error("wiring file {file:?} resolves outside project root {root}")]
    OutsideProject { file: String, root: PathBuf },
    #[error("failed to resolve Randomizer managed state directory {path}: {source}")]
    ResolveManagedState {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("wiring file {file:?} resolves inside Randomizer managed state {managed_root}")]
    ManagedState { file: String, managed_root: PathBuf },
    #[error("wiring path {file:?} is not a regular file")]
    NotAFile { file: String },
    #[error("wiring paths {first:?} and {second:?} resolve to the same file; use one exact path")]
    AliasedFile { first: String, second: String },
    #[error("failed to read wiring file {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("invalid JSON wiring file {path}: {source}")]
    ParseJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("invalid YAML wiring file {path}: {source}")]
    ParseYaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("selector {selector:?} was not found in wiring file {file:?}")]
    MissingSelector { file: String, selector: String },
    #[error("selector {selector:?} occurs {count} times in wiring file {file:?}")]
    AmbiguousSelector {
        file: String,
        selector: String,
        count: usize,
    },
    #[error("selector {selector:?} in wiring file {file:?} does not contain a string value")]
    NonStringValue { file: String, selector: String },
    #[error("invalid assignment for selector {selector:?} in wiring file {file:?}: {message}")]
    InvalidAssignment {
        file: String,
        selector: String,
        message: String,
    },
    #[error(
        "wiring mismatch for service {service:?} at {file:?} selector {selector:?}; expected {expected:?}"
    )]
    Mismatch {
        service: String,
        file: String,
        selector: String,
        expected: String,
    },
    #[error("project host {host:?} cannot form a gateway URL: {source}")]
    InvalidGatewayUrl {
        host: String,
        #[source]
        source: url::ParseError,
    },
    #[error("project host {host:?} is not an IP address: {source}")]
    InvalidGatewayHost {
        host: String,
        #[source]
        source: std::net::AddrParseError,
    },
    #[error("failed to serialize JSON wiring file {path}: {source}")]
    SerializeJson {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize YAML wiring file {path}: {source}")]
    SerializeYaml {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
    #[error("failed to write wiring file {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("wiring file {path} changed while endpoint updates were being prepared")]
    ConcurrentModification { path: PathBuf },
    #[error(
        "failed to update wiring file {path}: {source}; previously written application settings were restored"
    )]
    ApplyRolledBack {
        path: PathBuf,
        #[source]
        source: Box<WiringError>,
    },
    #[error(
        "failed to update wiring file {path}: {source}; rollback of {rollback_path} also failed: {rollback}; application settings may be partially updated"
    )]
    ApplyRollbackFailed {
        path: PathBuf,
        source: Box<WiringError>,
        rollback_path: PathBuf,
        rollback: Box<WiringError>,
    },
}

/// Applies every configured wiring entry, or only entries for `service` when supplied.
///
/// Every input file and selector is parsed and snapshotted before any file is replaced. Changed
/// files are replaced atomically; prior replacements are rolled back if a later write fails, and
/// unchanged files are not rewritten.
pub fn apply_wiring(
    project_root: &Path,
    manifest: &ProjectManifest,
    service: Option<&str>,
) -> Result<WiringReport, WiringError> {
    process_wiring(project_root, manifest, service, Mode::Apply)
}

/// Checks wiring without writing files. A stale value is returned as `WiringError::Mismatch`.
pub fn check_wiring(
    project_root: &Path,
    manifest: &ProjectManifest,
    service: Option<&str>,
) -> Result<WiringReport, WiringError> {
    process_wiring(project_root, manifest, service, Mode::Check)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Apply,
    Check,
}

#[derive(Debug)]
struct SelectedWiring<'a> {
    service: &'a ServiceDefinition,
    wiring: &'a WiringDefinition,
    expected: String,
}

#[derive(Debug)]
struct PreparedWrite {
    path: PathBuf,
    contents: String,
    original: Vec<u8>,
}

fn process_wiring(
    project_root: &Path,
    manifest: &ProjectManifest,
    service_filter: Option<&str>,
    mode: Mode,
) -> Result<WiringReport, WiringError> {
    validate_manifest(manifest)?;
    if let Some(service_id) = service_filter
        && !manifest
            .services
            .iter()
            .any(|service| service.id == service_id)
    {
        return Err(WiringError::UnknownService(service_id.to_string()));
    }

    let root = project_root
        .canonicalize()
        .map_err(|source| WiringError::ProjectRoot {
            path: project_root.to_path_buf(),
            source,
        })?;
    let has_selected_wiring = manifest.services.iter().any(|service| {
        !service.wiring.is_empty()
            && service_filter.is_none_or(|filter| filter == service.id.as_str())
    });
    if !has_selected_wiring {
        return Ok(WiringReport {
            entries: Vec::new(),
        });
    }
    let origin = gateway_origin(manifest)?;
    let mut files: BTreeMap<PathBuf, Vec<SelectedWiring<'_>>> = BTreeMap::new();
    let mut declared_paths: BTreeMap<PathBuf, String> = BTreeMap::new();

    for service in &manifest.services {
        if service_filter.is_some_and(|filter| filter != service.id) {
            continue;
        }
        for wiring in &service.wiring {
            let path = resolve_wiring_file(&root, &wiring.file)?;
            if let Some(first) = declared_paths.get(&path)
                && first != &wiring.file
            {
                return Err(WiringError::AliasedFile {
                    first: first.clone(),
                    second: wiring.file.clone(),
                });
            }
            declared_paths.insert(path.clone(), wiring.file.clone());
            files.entry(path).or_default().push(SelectedWiring {
                service,
                wiring,
                expected: expected_url(&origin, manifest, service, wiring),
            });
        }
    }

    let mut reports = Vec::new();
    let mut writes = Vec::new();
    for (path, entries) in files {
        let contents = fs::read_to_string(&path).map_err(|source| WiringError::Read {
            path: path.clone(),
            source,
        })?;
        let original = contents.as_bytes().to_vec();
        let mut document = Document::parse(entries[0].wiring.format, contents, &path)?;
        let mut file_changed = false;
        for entry in entries {
            let changed = document.update(
                &entry.wiring.file,
                &entry.wiring.selector,
                &entry.expected,
                mode,
            )?;
            if mode == Mode::Check && changed {
                return Err(WiringError::Mismatch {
                    service: entry.service.id.clone(),
                    file: entry.wiring.file.clone(),
                    selector: entry.wiring.selector.clone(),
                    expected: entry.expected,
                });
            }
            file_changed |= changed;
            reports.push(WiringEntryReport {
                service: entry.service.id.clone(),
                file: entry.wiring.file.clone(),
                selector: entry.wiring.selector.clone(),
                value: entry.expected,
                changed,
            });
        }
        if mode == Mode::Apply && file_changed {
            writes.push(PreparedWrite {
                contents: document.serialize(&path)?,
                path,
                original,
            });
        }
    }

    commit_prepared_writes(&writes)?;

    Ok(WiringReport { entries: reports })
}

fn commit_prepared_writes(writes: &[PreparedWrite]) -> Result<(), WiringError> {
    commit_prepared_writes_with(writes, atomic_replace)
}

fn commit_prepared_writes_with<F>(
    writes: &[PreparedWrite],
    mut writer: F,
) -> Result<(), WiringError>
where
    F: FnMut(&Path, &[u8]) -> Result<(), WiringError>,
{
    for (index, write) in writes.iter().enumerate() {
        let mut current_write_attempted = false;
        let result = fs::read(&write.path)
            .map_err(|source| WiringError::Read {
                path: write.path.clone(),
                source,
            })
            .and_then(|current| {
                if current == write.original {
                    current_write_attempted = true;
                    writer(&write.path, write.contents.as_bytes())
                } else {
                    Err(WiringError::ConcurrentModification {
                        path: write.path.clone(),
                    })
                }
            });
        let Err(source) = result else {
            continue;
        };

        let mut rollback_failure = None;
        let rollback_end = index + usize::from(current_write_attempted);
        for committed in writes[..rollback_end].iter().rev() {
            let rollback = fs::read(&committed.path)
                .map_err(|source| WiringError::Read {
                    path: committed.path.clone(),
                    source,
                })
                .and_then(|current| {
                    if current == committed.original {
                        Ok(())
                    } else if current == committed.contents.as_bytes() {
                        writer(&committed.path, &committed.original)
                    } else {
                        Err(WiringError::ConcurrentModification {
                            path: committed.path.clone(),
                        })
                    }
                });
            if let Err(rollback) = rollback
                && rollback_failure.is_none()
            {
                rollback_failure = Some((committed.path.clone(), rollback));
            }
        }
        return match rollback_failure {
            Some((rollback_path, rollback)) => Err(WiringError::ApplyRollbackFailed {
                path: write.path.clone(),
                source: Box::new(source),
                rollback_path,
                rollback: Box::new(rollback),
            }),
            None => Err(WiringError::ApplyRolledBack {
                path: write.path.clone(),
                source: Box::new(source),
            }),
        };
    }
    Ok(())
}

fn resolve_wiring_file(root: &Path, file: &str) -> Result<PathBuf, WiringError> {
    let candidate = root.join(file);
    let resolved = candidate
        .canonicalize()
        .map_err(|source| WiringError::ResolveFile {
            file: file.to_string(),
            source,
        })?;
    if !resolved.starts_with(root) {
        return Err(WiringError::OutsideProject {
            file: file.to_string(),
            root: root.to_path_buf(),
        });
    }
    let managed_path = root.join(".randomizer");
    match managed_path.canonicalize() {
        Ok(managed_root) if resolved.starts_with(&managed_root) => {
            return Err(WiringError::ManagedState {
                file: file.to_string(),
                managed_root,
            });
        }
        Ok(_) => {}
        Err(source) if source.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(WiringError::ResolveManagedState {
                path: managed_path,
                source,
            });
        }
    }
    if !resolved.is_file() {
        return Err(WiringError::NotAFile {
            file: file.to_string(),
        });
    }
    Ok(resolved)
}

fn gateway_origin(manifest: &ProjectManifest) -> Result<String, WiringError> {
    let host = manifest.project.host.trim();
    let bind_address =
        host.parse::<std::net::IpAddr>()
            .map_err(|source| WiringError::InvalidGatewayHost {
                host: manifest.project.host.clone(),
                source,
            })?;
    let client_address = match bind_address {
        std::net::IpAddr::V4(address) if address.is_unspecified() => {
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        }
        std::net::IpAddr::V6(address) if address.is_unspecified() => {
            std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        }
        address => address,
    };
    let authority_host = match client_address {
        std::net::IpAddr::V4(address) => address.to_string(),
        std::net::IpAddr::V6(address) => format!("[{address}]"),
    };
    let origin = format!("http://{authority_host}:{}", manifest.project.port);
    url::Url::parse(&origin).map_err(|source| WiringError::InvalidGatewayUrl {
        host: manifest.project.host.clone(),
        source,
    })?;
    Ok(origin)
}

fn expected_url(
    origin: &str,
    manifest: &ProjectManifest,
    service: &ServiceDefinition,
    wiring: &WiringDefinition,
) -> String {
    let base = format!("{origin}/mock/{}", service.id);
    match wiring.target {
        WiringTarget::ServiceBaseUrl => base,
        WiringTarget::RouteUrl => {
            let route_id = wiring
                .route
                .as_deref()
                .expect("validated route_url wiring has a route");
            let route = manifest
                .routes
                .iter()
                .find(|route| route.id == route_id)
                .expect("validated route_url wiring references an existing route");
            format!("{base}{}", route.request_match.path)
        }
    }
}

fn atomic_replace(path: &Path, contents: &[u8]) -> Result<(), WiringError> {
    let parent = path.parent().ok_or_else(|| WiringError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory"),
    })?;
    let permissions = fs::metadata(path)
        .map_err(|source| WiringError::Write {
            path: path.to_path_buf(),
            source,
        })?
        .permissions();
    let mut temporary = NamedTempFile::new_in(parent).map_err(|source| WiringError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    temporary
        .as_file()
        .set_permissions(permissions)
        .map_err(|source| WiringError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .write_all(contents)
        .and_then(|()| temporary.flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| WiringError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    temporary
        .persist(path)
        .map_err(|error| WiringError::Write {
            path: path.to_path_buf(),
            source: error.error,
        })?;
    sync_parent_directory(path)
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), WiringError> {
    let parent = path.parent().ok_or_else(|| WiringError::Write {
        path: path.to_path_buf(),
        source: io::Error::new(io::ErrorKind::InvalidInput, "file has no parent directory"),
    })?;
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| WiringError::Write {
            path: parent.to_path_buf(),
            source,
        })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), WiringError> {
    Ok(())
}

#[derive(Debug)]
enum Document {
    Lines {
        contents: String,
        format: WiringFormat,
    },
    Json(JsonValue),
    Yaml(YamlValue),
}

impl Document {
    fn parse(format: WiringFormat, contents: String, path: &Path) -> Result<Self, WiringError> {
        match format {
            WiringFormat::Dotenv | WiringFormat::Properties => Ok(Self::Lines { contents, format }),
            WiringFormat::Json => {
                serde_json::from_str(&contents)
                    .map(Self::Json)
                    .map_err(|source| WiringError::ParseJson {
                        path: path.to_path_buf(),
                        source,
                    })
            }
            WiringFormat::Yaml => {
                serde_yaml::from_str(&contents)
                    .map(Self::Yaml)
                    .map_err(|source| WiringError::ParseYaml {
                        path: path.to_path_buf(),
                        source,
                    })
            }
        }
    }

    fn update(
        &mut self,
        file: &str,
        selector: &str,
        expected: &str,
        mode: Mode,
    ) -> Result<bool, WiringError> {
        match self {
            Self::Lines { contents, format } => {
                update_line_document(contents, *format, file, selector, expected, mode)
            }
            Self::Json(value) => {
                let selected =
                    value
                        .pointer_mut(selector)
                        .ok_or_else(|| WiringError::MissingSelector {
                            file: file.to_string(),
                            selector: selector.to_string(),
                        })?;
                let current = selected
                    .as_str()
                    .ok_or_else(|| WiringError::NonStringValue {
                        file: file.to_string(),
                        selector: selector.to_string(),
                    })?;
                let changed = current != expected;
                if changed && mode == Mode::Apply {
                    *selected = JsonValue::String(expected.to_string());
                }
                Ok(changed)
            }
            Self::Yaml(value) => {
                let selected = yaml_value_mut(value, selector).ok_or_else(|| {
                    WiringError::MissingSelector {
                        file: file.to_string(),
                        selector: selector.to_string(),
                    }
                })?;
                let current = selected
                    .as_str()
                    .ok_or_else(|| WiringError::NonStringValue {
                        file: file.to_string(),
                        selector: selector.to_string(),
                    })?;
                let changed = current != expected;
                if changed && mode == Mode::Apply {
                    *selected = YamlValue::String(expected.to_string());
                }
                Ok(changed)
            }
        }
    }

    fn serialize(self, path: &Path) -> Result<String, WiringError> {
        match self {
            Self::Lines { contents, .. } => Ok(contents),
            Self::Json(value) => {
                let mut output = serde_json::to_string_pretty(&value).map_err(|source| {
                    WiringError::SerializeJson {
                        path: path.to_path_buf(),
                        source,
                    }
                })?;
                output.push('\n');
                Ok(output)
            }
            Self::Yaml(value) => {
                serde_yaml::to_string(&value).map_err(|source| WiringError::SerializeYaml {
                    path: path.to_path_buf(),
                    source,
                })
            }
        }
    }
}

fn yaml_value_mut<'a>(value: &'a mut YamlValue, selector: &str) -> Option<&'a mut YamlValue> {
    let mut current = value;
    for segment in selector.split('.') {
        let mapping = current.as_mapping_mut()?;
        current = mapping.get_mut(YamlValue::String(segment.to_string()))?;
    }
    Some(current)
}

fn update_line_document(
    contents: &mut String,
    format: WiringFormat,
    file: &str,
    selector: &str,
    expected: &str,
    mode: Mode,
) -> Result<bool, WiringError> {
    let ranges = assignment_ranges(contents, format, file, selector)?;
    let range = match ranges.len() {
        0 => {
            return Err(WiringError::MissingSelector {
                file: file.to_string(),
                selector: selector.to_string(),
            });
        }
        1 => ranges.into_iter().next().expect("one assignment range"),
        count => {
            return Err(WiringError::AmbiguousSelector {
                file: file.to_string(),
                selector: selector.to_string(),
                count,
            });
        }
    };
    let changed = &contents[range.clone()] != expected;
    if changed && mode == Mode::Apply {
        contents.replace_range(range, expected);
    }
    Ok(changed)
}

fn assignment_ranges(
    contents: &str,
    format: WiringFormat,
    file: &str,
    selector: &str,
) -> Result<Vec<Range<usize>>, WiringError> {
    let mut ranges = Vec::new();
    let mut offset = 0;
    for line_with_ending in contents.split_inclusive('\n') {
        let mut line = line_with_ending
            .strip_suffix('\n')
            .unwrap_or(line_with_ending);
        if let Some(without_carriage_return) = line.strip_suffix('\r') {
            line = without_carriage_return;
        }
        let result = match format {
            WiringFormat::Dotenv => dotenv_value_range(line, selector),
            WiringFormat::Properties => properties_value_range(line, selector),
            WiringFormat::Json | WiringFormat::Yaml => unreachable!("structured document format"),
        };
        match result {
            Ok(Some(range)) => ranges.push((range.start + offset)..(range.end + offset)),
            Ok(None) => {}
            Err(message) => {
                return Err(WiringError::InvalidAssignment {
                    file: file.to_string(),
                    selector: selector.to_string(),
                    message,
                });
            }
        }
        offset += line_with_ending.len();
    }
    Ok(ranges)
}

fn dotenv_value_range(line: &str, selector: &str) -> Result<Option<Range<usize>>, String> {
    let bytes = line.as_bytes();
    let mut cursor = skip_horizontal_whitespace(bytes, 0);
    if cursor == bytes.len() || bytes[cursor] == b'#' {
        return Ok(None);
    }
    if line[cursor..].starts_with("export")
        && bytes
            .get(cursor + "export".len())
            .is_some_and(u8::is_ascii_whitespace)
    {
        cursor = skip_horizontal_whitespace(bytes, cursor + "export".len());
    }
    let Some(relative_equals) = line[cursor..].find('=') else {
        return Ok(None);
    };
    let equals = cursor + relative_equals;
    if line[cursor..equals].trim() != selector {
        return Ok(None);
    }
    let value_start = skip_horizontal_whitespace(bytes, equals + 1);
    if let Some(&quote @ (b'\'' | b'"')) = bytes.get(value_start) {
        let mut escaped = false;
        for (index, byte) in bytes.iter().enumerate().skip(value_start + 1) {
            if quote == b'"' && *byte == b'\\' && !escaped {
                escaped = true;
                continue;
            }
            if *byte == quote && !escaped {
                return Ok(Some((value_start + 1)..index));
            }
            escaped = false;
        }
        return Err("quoted value is missing its closing quote".into());
    }

    let comment = (value_start..bytes.len()).find(|&index| {
        bytes[index] == b'#' && (index == value_start || bytes[index - 1].is_ascii_whitespace())
    });
    let value_end =
        trim_horizontal_whitespace_end(bytes, value_start, comment.unwrap_or(bytes.len()));
    Ok(Some(value_start..value_end))
}

fn properties_value_range(line: &str, selector: &str) -> Result<Option<Range<usize>>, String> {
    let bytes = line.as_bytes();
    let cursor = skip_horizontal_whitespace(bytes, 0);
    if cursor == bytes.len() || matches!(bytes[cursor], b'#' | b'!') {
        return Ok(None);
    }

    let mut escaped = false;
    let mut separator = None;
    for (index, byte) in bytes.iter().enumerate().skip(cursor) {
        if *byte == b'\\' && !escaped {
            escaped = true;
            continue;
        }
        if !escaped && (matches!(*byte, b'=' | b':') || byte.is_ascii_whitespace()) {
            separator = Some(index);
            break;
        }
        escaped = false;
    }
    let key_end = separator.unwrap_or(bytes.len());
    if line[cursor..key_end].trim_end() != selector {
        return Ok(None);
    }

    let mut value_start = separator.unwrap_or(bytes.len());
    if value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
        value_start = skip_horizontal_whitespace(bytes, value_start);
        if value_start < bytes.len() && matches!(bytes[value_start], b'=' | b':') {
            value_start += 1;
        }
    } else if value_start < bytes.len() {
        value_start += 1;
    }
    value_start = skip_horizontal_whitespace(bytes, value_start);
    let value_end = trim_horizontal_whitespace_end(bytes, value_start, bytes.len());
    if value_end > value_start && bytes[value_end - 1] == b'\\' {
        return Err("continued property values are not supported".into());
    }
    Ok(Some(value_start..value_end))
}

fn skip_horizontal_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
        index += 1;
    }
    index
}

fn trim_horizontal_whitespace_end(bytes: &[u8], start: usize, mut end: usize) -> usize {
    while end > start && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::project::{
        MatchDefinition, ProjectDefinition, ResponseBodyDefinition, ResponseDefinition,
        RouteDefinition, ServiceDefinition, WiringDefinition,
    };

    #[test]
    fn applies_dotenv_and_properties_without_changing_line_structure() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(".env.local"),
            "# local\r\nexport SERVICE_URL = \"https://real.example\" # keep\r\nOTHER=1\r\n",
        )
        .unwrap();
        fs::write(
            directory.path().join("application.properties"),
            "# local\nclient.service-url : https://real.example   \nother=true\n",
        )
        .unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring = vec![
            wiring(
                ".env.local",
                WiringFormat::Dotenv,
                "SERVICE_URL",
                WiringTarget::ServiceBaseUrl,
                None,
            ),
            wiring(
                "application.properties",
                WiringFormat::Properties,
                "client.service-url",
                WiringTarget::RouteUrl,
                Some("get-item"),
            ),
        ];

        let report = apply_wiring(directory.path(), &manifest, None).unwrap();

        assert_eq!(report.changed_count(), 2);
        assert_eq!(
            fs::read_to_string(directory.path().join(".env.local")).unwrap(),
            "# local\r\nexport SERVICE_URL = \"http://127.0.0.1:7263/mock/service\" # keep\r\nOTHER=1\r\n"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("application.properties")).unwrap(),
            "# local\nclient.service-url : http://127.0.0.1:7263/mock/service/items/static   \nother=true\n"
        );

        let second = apply_wiring(directory.path(), &manifest, None).unwrap();
        assert_eq!(second.changed_count(), 0);
        check_wiring(directory.path(), &manifest, None).unwrap();
    }

    #[test]
    fn applies_json_pointer_and_yaml_dot_path_deterministically() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("config.json"),
            r#"{"services":{"item":{"baseUrl":"https://real.example"}},"enabled":true}"#,
        )
        .unwrap();
        fs::write(
            directory.path().join("config.yaml"),
            "services:\n  item:\n    base-url: https://real.example\nenabled: true\n",
        )
        .unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring = vec![
            wiring(
                "config.json",
                WiringFormat::Json,
                "/services/item/baseUrl",
                WiringTarget::ServiceBaseUrl,
                None,
            ),
            wiring(
                "config.yaml",
                WiringFormat::Yaml,
                "services.item.base-url",
                WiringTarget::ServiceBaseUrl,
                None,
            ),
        ];

        apply_wiring(directory.path(), &manifest, None).unwrap();

        let json: JsonValue = serde_json::from_str(
            &fs::read_to_string(directory.path().join("config.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            json.pointer("/services/item/baseUrl").unwrap(),
            "http://127.0.0.1:7263/mock/service"
        );
        let yaml: YamlValue = serde_yaml::from_str(
            &fs::read_to_string(directory.path().join("config.yaml")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            yaml["services"]["item"]["base-url"],
            "http://127.0.0.1:7263/mock/service"
        );
        assert!(
            fs::read_to_string(directory.path().join("config.json"))
                .unwrap()
                .ends_with('\n')
        );
        let json_after_first_apply =
            fs::read_to_string(directory.path().join("config.json")).unwrap();
        let yaml_after_first_apply =
            fs::read_to_string(directory.path().join("config.yaml")).unwrap();

        let second = apply_wiring(directory.path(), &manifest, None).unwrap();

        assert_eq!(second.changed_count(), 0);
        assert_eq!(
            fs::read_to_string(directory.path().join("config.json")).unwrap(),
            json_after_first_apply
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("config.yaml")).unwrap(),
            yaml_after_first_apply
        );
    }

    #[test]
    fn wiring_uses_loopback_for_unspecified_bind_addresses() {
        let mut manifest = manifest();
        manifest.project.host = "0.0.0.0".into();
        assert_eq!(gateway_origin(&manifest).unwrap(), "http://127.0.0.1:7263");

        manifest.project.host = "::".into();
        assert_eq!(gateway_origin(&manifest).unwrap(), "http://[::1]:7263");
    }

    #[test]
    fn check_is_read_only_and_reports_stale_values() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(".env");
        fs::write(&path, "SERVICE_URL=https://real.example\n").unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring.push(wiring(
            ".env",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        ));

        let error = check_wiring(directory.path(), &manifest, None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("wiring mismatch"));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "SERVICE_URL=https://real.example\n"
        );
    }

    #[test]
    fn rejects_service_base_redirection_without_a_safety_assertion() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(".env");
        fs::write(&path, "SERVICE_URL=https://real.example\n").unwrap();
        let mut manifest = manifest();
        let mut unsafe_wiring = wiring(
            ".env",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        );
        unsafe_wiring.service_base_safety = None;
        manifest.services[0].wiring.push(unsafe_wiring);

        let error = apply_wiring(directory.path(), &manifest, None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("must set service_base_safety"));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "SERVICE_URL=https://real.example\n"
        );
    }

    #[test]
    fn rejects_service_base_redirection_without_verified_path_behavior() {
        let directory = tempdir().unwrap();
        let path = directory.path().join(".env");
        fs::write(&path, "SERVICE_URL=https://real.example\n").unwrap();
        let mut manifest = manifest();
        let mut unsafe_wiring = wiring(
            ".env",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        );
        unsafe_wiring.service_base_path_behavior = None;
        manifest.services[0].wiring.push(unsafe_wiring);

        let error = apply_wiring(directory.path(), &manifest, None)
            .unwrap_err()
            .to_string();

        assert!(error.contains("must set service_base_path_behavior"));
        assert_eq!(
            fs::read_to_string(path).unwrap(),
            "SERVICE_URL=https://real.example\n"
        );
    }

    #[test]
    fn rejects_missing_duplicate_and_non_string_selectors_before_writing() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(".env"),
            "SERVICE_URL=one\nSERVICE_URL=two\n",
        )
        .unwrap();
        fs::write(directory.path().join("config.json"), r#"{"url":42}"#).unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring.push(wiring(
            ".env",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        ));
        assert!(
            apply_wiring(directory.path(), &manifest, None)
                .unwrap_err()
                .to_string()
                .contains("occurs 2 times")
        );

        manifest.services[0].wiring[0].selector = "MISSING_URL".into();
        assert!(
            apply_wiring(directory.path(), &manifest, None)
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );

        manifest.services[0].wiring[0] = wiring(
            "config.json",
            WiringFormat::Json,
            "/url",
            WiringTarget::ServiceBaseUrl,
            None,
        );
        assert!(
            apply_wiring(directory.path(), &manifest, None)
                .unwrap_err()
                .to_string()
                .contains("does not contain a string")
        );
        assert_eq!(
            fs::read_to_string(directory.path().join(".env")).unwrap(),
            "SERVICE_URL=one\nSERVICE_URL=two\n"
        );
    }

    #[test]
    fn validates_every_file_before_replacing_any_file() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("a.env"), "FIRST_URL=old\n").unwrap();
        fs::write(directory.path().join("z.env"), "OTHER=value\n").unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring = vec![
            wiring(
                "a.env",
                WiringFormat::Dotenv,
                "FIRST_URL",
                WiringTarget::ServiceBaseUrl,
                None,
            ),
            wiring(
                "z.env",
                WiringFormat::Dotenv,
                "MISSING_URL",
                WiringTarget::ServiceBaseUrl,
                None,
            ),
        ];

        assert!(
            apply_wiring(directory.path(), &manifest, None)
                .unwrap_err()
                .to_string()
                .contains("was not found")
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("a.env")).unwrap(),
            "FIRST_URL=old\n"
        );
    }

    #[test]
    fn restores_earlier_files_when_a_later_wiring_write_fails() {
        let directory = tempdir().unwrap();
        let first = directory.path().join("first.env");
        let second = directory.path().join("second.env");
        fs::write(&first, "URL=first-old\n").unwrap();
        fs::write(&second, "URL=second-old\n").unwrap();
        let writes = vec![
            PreparedWrite {
                path: first.clone(),
                contents: "URL=first-new\n".into(),
                original: b"URL=first-old\n".to_vec(),
            },
            PreparedWrite {
                path: second.clone(),
                contents: "URL=second-new\n".into(),
                original: b"URL=second-old\n".to_vec(),
            },
        ];

        let error = commit_prepared_writes_with(&writes, |path, contents| {
            if path == second && contents == b"URL=second-new\n" {
                return Err(WiringError::Write {
                    path: path.to_path_buf(),
                    source: io::Error::other("simulated failure"),
                });
            }
            atomic_replace(path, contents)
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("were restored"), "{error}");
        assert_eq!(fs::read_to_string(first).unwrap(), "URL=first-old\n");
        assert_eq!(fs::read_to_string(second).unwrap(), "URL=second-old\n");
    }

    #[test]
    fn restores_the_current_file_when_a_write_reports_failure_after_replacement() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("service.env");
        fs::write(&path, "URL=old\n").unwrap();
        let writes = vec![PreparedWrite {
            path: path.clone(),
            contents: "URL=new\n".into(),
            original: b"URL=old\n".to_vec(),
        }];
        let mut first_call = true;

        let error = commit_prepared_writes_with(&writes, |path, contents| {
            atomic_replace(path, contents)?;
            if first_call {
                first_call = false;
                return Err(WiringError::Write {
                    path: path.to_path_buf(),
                    source: io::Error::other("simulated post-replacement failure"),
                });
            }
            Ok(())
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("were restored"), "{error}");
        assert_eq!(fs::read_to_string(path).unwrap(), "URL=old\n");
    }

    #[test]
    fn does_not_rollback_a_concurrent_change_that_matches_the_desired_value() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("service.env");
        fs::write(&path, "URL=new\n").unwrap();
        let writes = vec![PreparedWrite {
            path: path.clone(),
            contents: "URL=new\n".into(),
            original: b"URL=old\n".to_vec(),
        }];

        let error = commit_prepared_writes(&writes).unwrap_err().to_string();

        assert!(error.contains("changed while endpoint updates were being prepared"));
        assert_eq!(fs::read_to_string(path).unwrap(), "URL=new\n");
    }

    #[test]
    fn service_filter_updates_only_the_selected_service() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join(".env"),
            "FIRST_URL=old\nSECOND_URL=old\n",
        )
        .unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring.push(wiring(
            ".env",
            WiringFormat::Dotenv,
            "FIRST_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        ));
        manifest.services.push(ServiceDefinition {
            id: "second".into(),
            config_key: None,
            wiring: vec![wiring(
                ".env",
                WiringFormat::Dotenv,
                "SECOND_URL",
                WiringTarget::ServiceBaseUrl,
                None,
            )],
        });

        let report = apply_wiring(directory.path(), &manifest, Some("second")).unwrap();

        assert_eq!(report.entries.len(), 1);
        assert_eq!(
            fs::read_to_string(directory.path().join(".env")).unwrap(),
            "FIRST_URL=old\nSECOND_URL=http://127.0.0.1:7263/mock/second\n"
        );
        assert!(matches!(
            apply_wiring(directory.path(), &manifest, Some("missing")),
            Err(WiringError::UnknownService(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_that_escape_the_project_root() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("external.env"), "SERVICE_URL=old\n").unwrap();
        symlink(
            outside.path().join("external.env"),
            directory.path().join("linked.env"),
        )
        .unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring.push(wiring(
            "linked.env",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        ));

        assert!(matches!(
            apply_wiring(directory.path(), &manifest, None),
            Err(WiringError::OutsideProject { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinks_to_randomizer_managed_state() {
        use std::os::unix::fs::symlink;

        let directory = tempdir().unwrap();
        let managed = directory.path().join(".randomizer");
        fs::create_dir(&managed).unwrap();
        fs::write(managed.join("randomizer.yaml"), "SERVICE_URL=old\n").unwrap();
        symlink(
            managed.join("randomizer.yaml"),
            directory.path().join("config-link"),
        )
        .unwrap();
        let mut manifest = manifest();
        manifest.services[0].wiring.push(wiring(
            "config-link",
            WiringFormat::Dotenv,
            "SERVICE_URL",
            WiringTarget::ServiceBaseUrl,
            None,
        ));

        assert!(matches!(
            apply_wiring(directory.path(), &manifest, None),
            Err(WiringError::ManagedState { .. })
        ));
        assert_eq!(
            fs::read_to_string(managed.join("randomizer.yaml")).unwrap(),
            "SERVICE_URL=old\n"
        );
    }

    fn manifest() -> ProjectManifest {
        ProjectManifest {
            version: super::super::CURRENT_MANIFEST_VERSION,
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
                    path: "/items/static".into(),
                    ..MatchDefinition::default()
                },
                responses: vec![ResponseDefinition {
                    status: 200,
                    headers: Default::default(),
                    delay_ms: 0,
                    body: ResponseBodyDefinition::default(),
                    bindings: Vec::new(),
                }],
            }],
        }
    }

    fn wiring(
        file: &str,
        format: WiringFormat,
        selector: &str,
        target: WiringTarget,
        route: Option<&str>,
    ) -> WiringDefinition {
        WiringDefinition {
            file: file.into(),
            format,
            selector: selector.into(),
            target,
            route: route.map(str::to_string),
            service_base_safety: (target == WiringTarget::ServiceBaseUrl)
                .then_some(super::super::manifest::ServiceBaseSafety::DedicatedSetting),
            service_base_path_behavior: (target == WiringTarget::ServiceBaseUrl)
                .then_some(super::super::manifest::ServiceBasePathBehavior::PreservesPrefix),
        }
    }
}

use std::{
    env,
    ffi::{OsStr, OsString},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::project::ProjectPaths;

const EXPORTER_PROTOCOL_VERSION: u32 = 1;
const EXPORTER_MAIN_CLASS: &str = "com.acko.randomizer.dto.JavaDtoExporter";
const EXPORTER_JAR: &[u8] = include_bytes!(concat!(
    env!("OUT_DIR"),
    "/randomizer-java-dto-exporter.jar"
));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum BuildSystem {
    Auto,
    Maven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum FieldPresence {
    All,
    Annotated,
}

impl FieldPresence {
    fn exporter_name(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Annotated => "ANNOTATED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct JavaExportResult {
    pub schema: Value,
    pub visited_classes: Vec<String>,
    pub input_hash: String,
    pub warnings: Vec<String>,
}

pub struct JavaDtoExtractor {
    project_classes: PathBuf,
    classpath: Vec<PathBuf>,
    exporter_jar: PathBuf,
}

impl JavaDtoExtractor {
    pub fn prepare(paths: &ProjectPaths, build_system: BuildSystem) -> Result<Self, JavaDtoError> {
        let resolved = resolve_build_system(&paths.root, build_system)?;
        match resolved {
            BuildSystem::Maven => prepare_maven(paths),
            BuildSystem::Auto => unreachable!("auto is resolved before preparing the extractor"),
        }
    }

    pub fn export(
        &self,
        root_type: &str,
        field_presence: FieldPresence,
    ) -> Result<JavaExportResult, JavaDtoError> {
        let request = ExportRequest {
            protocol_version: EXPORTER_PROTOCOL_VERSION,
            root_type,
            project_classes: &self.project_classes,
            field_presence: field_presence.exporter_name(),
        };
        let encoded = serde_json::to_vec(&request).map_err(JavaDtoError::ProtocolEncode)?;
        let mut classpath = Vec::with_capacity(self.classpath.len() + 2);
        classpath.push(self.exporter_jar.clone());
        classpath.push(self.project_classes.clone());
        classpath.extend(self.classpath.iter().cloned());
        let joined = env::join_paths(classpath).map_err(JavaDtoError::ClasspathJoin)?;

        let mut command = Command::new(java_command());
        command
            .arg("-cp")
            .arg(joined)
            .arg(EXPORTER_MAIN_CLASS)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(JavaDtoError::JavaStart)?;
        child
            .stdin
            .take()
            .ok_or(JavaDtoError::JavaStdin)?
            .write_all(&encoded)
            .map_err(JavaDtoError::JavaWrite)?;
        let output = child.wait_with_output().map_err(JavaDtoError::JavaWait)?;
        if !output.status.success() {
            return Err(JavaDtoError::ExporterFailed {
                status: output.status.code(),
                message: output_message(&output.stderr, &output.stdout),
            });
        }
        let response: ExportResponse =
            serde_json::from_slice(&output.stdout).map_err(JavaDtoError::ProtocolDecode)?;
        if response.protocol_version != EXPORTER_PROTOCOL_VERSION {
            return Err(JavaDtoError::ProtocolVersion {
                provided: response.protocol_version,
                supported: EXPORTER_PROTOCOL_VERSION,
            });
        }
        if response.visited_classes.is_empty() {
            return Err(JavaDtoError::NoProjectClasses);
        }
        Ok(JavaExportResult {
            schema: response.schema,
            visited_classes: response.visited_classes,
            input_hash: response.input_hash,
            warnings: response.warnings,
        })
    }
}

fn resolve_build_system(root: &Path, requested: BuildSystem) -> Result<BuildSystem, JavaDtoError> {
    match requested {
        BuildSystem::Maven if root.join("pom.xml").is_file() => Ok(BuildSystem::Maven),
        BuildSystem::Maven => Err(JavaDtoError::MissingMavenProject(root.join("pom.xml"))),
        BuildSystem::Auto if root.join("pom.xml").is_file() => Ok(BuildSystem::Maven),
        BuildSystem::Auto => Err(JavaDtoError::UnsupportedProject(root.to_path_buf())),
    }
}

fn prepare_maven(paths: &ProjectPaths) -> Result<JavaDtoExtractor, JavaDtoError> {
    fs::create_dir_all(&paths.runtime).map_err(|source| JavaDtoError::RuntimeDirectory {
        path: paths.runtime.clone(),
        source,
    })?;
    let classpath_file = paths.runtime.join("java-classpath.txt");
    let mut command = maven_command(&paths.root)?;
    command
        .current_dir(&paths.root)
        .arg("-q")
        .arg("-DskipTests")
        .arg("compile")
        .arg("dependency:build-classpath")
        .arg(format!("-Dmdep.outputFile={}", classpath_file.display()));
    let output = command.output().map_err(JavaDtoError::MavenStart)?;
    if !output.status.success() {
        return Err(JavaDtoError::MavenFailed {
            status: output.status.code(),
            message: output_message(&output.stderr, &output.stdout),
        });
    }

    let project_classes = paths.root.join("target/classes");
    if !project_classes.is_dir() {
        return Err(JavaDtoError::MissingClasses(project_classes));
    }
    let encoded =
        fs::read_to_string(&classpath_file).map_err(|source| JavaDtoError::ClasspathRead {
            path: classpath_file.clone(),
            source,
        })?;
    let classpath = env::split_paths(OsStr::new(encoded.trim()))
        .filter(|path| !path.as_os_str().is_empty())
        .collect();
    let exporter_jar = materialize_exporter(paths)?;
    Ok(JavaDtoExtractor {
        project_classes,
        classpath,
        exporter_jar,
    })
}

fn maven_command(root: &Path) -> Result<Command, JavaDtoError> {
    let unix_wrapper = root.join("mvnw");
    if !cfg!(windows) && unix_wrapper.is_file() {
        let mut command = Command::new("sh");
        command.arg(unix_wrapper);
        return Ok(command);
    }
    let windows_wrapper = root.join("mvnw.cmd");
    if cfg!(windows) && windows_wrapper.is_file() {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(windows_wrapper);
        return Ok(command);
    }
    let mut probe = system_maven_command();
    if command_available(&mut probe) {
        return Ok(system_maven_command());
    }
    Err(JavaDtoError::MavenUnavailable)
}

fn system_maven_command() -> Command {
    if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg("mvn");
        command
    } else {
        Command::new("mvn")
    }
}

fn command_available(command: &mut Command) -> bool {
    command
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn java_command() -> OsString {
    if let Some(java_home) = env::var_os("JAVA_HOME") {
        let executable = if cfg!(windows) { "java.exe" } else { "java" };
        let candidate = PathBuf::from(java_home).join("bin").join(executable);
        if candidate.is_file() {
            return candidate.into_os_string();
        }
    }
    OsString::from("java")
}

fn materialize_exporter(paths: &ProjectPaths) -> Result<PathBuf, JavaDtoError> {
    let tools = paths.runtime.join("tools");
    fs::create_dir_all(&tools).map_err(|source| JavaDtoError::RuntimeDirectory {
        path: tools.clone(),
        source,
    })?;
    let hash = hex_digest(EXPORTER_JAR);
    let destination = tools.join(format!(
        "java-dto-exporter-{}-{}.jar",
        env!("CARGO_PKG_VERSION"),
        &hash[..12]
    ));
    if destination.is_file() {
        let existing = fs::read(&destination).map_err(|source| JavaDtoError::ExporterRead {
            path: destination.clone(),
            source,
        })?;
        if hex_digest(&existing) == hash {
            return Ok(destination);
        }
    }
    atomic_write(&destination, EXPORTER_JAR).map_err(|source| JavaDtoError::ExporterWrite {
        path: destination.clone(),
        source,
    })?;
    Ok(destination)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn output_message(stderr: &[u8], stdout: &[u8]) -> String {
    const MAX_OUTPUT: usize = 16 * 1024;
    let selected = if stderr.is_empty() { stdout } else { stderr };
    let start = selected.len().saturating_sub(MAX_OUTPUT);
    String::from_utf8_lossy(&selected[start..])
        .trim()
        .to_string()
}

#[derive(Serialize)]
struct ExportRequest<'a> {
    protocol_version: u32,
    root_type: &'a str,
    project_classes: &'a Path,
    field_presence: &'static str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportResponse {
    protocol_version: u32,
    schema: Value,
    visited_classes: Vec<String>,
    input_hash: String,
    warnings: Vec<String>,
}

#[derive(Debug, Error)]
pub enum JavaDtoError {
    #[error(
        "no supported Java build was found at {0}; Phase 1 supports Maven projects with a pom.xml"
    )]
    UnsupportedProject(PathBuf),
    #[error("Maven project descriptor not found at {0}")]
    MissingMavenProject(PathBuf),
    #[error("Maven is unavailable; add mvnw/mvnw.cmd to the project or install Maven")]
    MavenUnavailable,
    #[error("failed to start Maven: {0}")]
    MavenStart(#[source] std::io::Error),
    #[error("Maven compilation failed (exit {status:?}): {message}")]
    MavenFailed {
        status: Option<i32>,
        message: String,
    },
    #[error("Maven completed without producing compiled classes at {0}")]
    MissingClasses(PathBuf),
    #[error("failed to read Maven classpath {path}: {source}")]
    ClasspathRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to construct the Java classpath: {0}")]
    ClasspathJoin(#[source] env::JoinPathsError),
    #[error("failed to create runtime tool directory {path}: {source}")]
    RuntimeDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read embedded exporter at {path}: {source}")]
    ExporterRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to materialize embedded exporter at {path}: {source}")]
    ExporterWrite {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode the Java exporter request: {0}")]
    ProtocolEncode(#[source] serde_json::Error),
    #[error("failed to decode the Java exporter response: {0}")]
    ProtocolDecode(#[source] serde_json::Error),
    #[error("Java exporter returned protocol version {provided}; supported version is {supported}")]
    ProtocolVersion { provided: u32, supported: u32 },
    #[error("failed to start Java; install JDK 17 or newer: {0}")]
    JavaStart(#[source] std::io::Error),
    #[error("failed to open stdin for the Java exporter")]
    JavaStdin,
    #[error("failed to send the request to the Java exporter: {0}")]
    JavaWrite(#[source] std::io::Error),
    #[error("failed while waiting for the Java exporter: {0}")]
    JavaWait(#[source] std::io::Error),
    #[error("Java DTO extraction failed (exit {status:?}): {message}")]
    ExporterFailed {
        status: Option<i32>,
        message: String,
    },
    #[error("Java DTO extraction found no classes under the application's compiled output")]
    NoProjectClasses,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_message_prefers_stderr_and_is_bounded() {
        assert_eq!(output_message(b"failure\n", b"ignored"), "failure");
        let large = vec![b'x'; 20 * 1024];
        assert_eq!(output_message(&large, b"").len(), 16 * 1024);
    }
}

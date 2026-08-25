use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    process::Command,
    time::timeout,
};

use super::{ProviderError, ProviderRequest, ProviderResponse, Result, validate_provider_response};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MAX_STDERR_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone)]
pub struct ProviderCommand {
    program: PathBuf,
    args: Vec<OsString>,
    current_dir: Option<PathBuf>,
    timeout: Duration,
    max_output_bytes: usize,
}

impl ProviderCommand {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            current_dir: None,
            timeout: DEFAULT_TIMEOUT,
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub fn arg(mut self, arg: impl Into<OsString>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn current_dir(mut self, current_dir: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(current_dir.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }

    pub fn program(&self) -> &Path {
        &self.program
    }

    pub fn command_args(&self) -> impl Iterator<Item = &OsStr> {
        self.args.iter().map(OsString::as_os_str)
    }
}

pub async fn run_provider(
    command: &ProviderCommand,
    request: &ProviderRequest,
) -> Result<ProviderResponse> {
    request.validate()?;
    let mut encoded = serde_json::to_vec(request).map_err(ProviderError::EncodeRequest)?;
    encoded.push(b'\n');

    let program = command.program.display().to_string();
    let output = timeout(
        command.timeout,
        exchange_provider(command, &program, &encoded),
    )
    .await
    .map_err(|_| ProviderError::Timeout {
        program: program.clone(),
        timeout: command.timeout,
    })??;
    if !output.status.success() {
        return Err(ProviderError::Exit {
            program,
            status: output.status,
            stderr: output.stderr.render(),
        });
    }

    let response: ProviderResponse =
        serde_json::from_slice(&output.stdout).map_err(|source| ProviderError::DecodeResponse {
            program: program.clone(),
            source,
        })?;
    validate_provider_response(&response)?;
    if response.endpoint != request.endpoint {
        return Err(ProviderError::EndpointMismatch {
            program,
            expected: Box::new(request.endpoint.clone()),
            actual: Box::new(response.endpoint),
        });
    }
    for requested_path in &request.source_paths {
        if !response
            .source_fingerprints
            .iter()
            .any(|fingerprint| &fingerprint.path == requested_path)
        {
            return Err(ProviderError::InvalidResponse(format!(
                "provider omitted a source fingerprint for requested path {requested_path:?}"
            )));
        }
    }
    Ok(response)
}

struct ProviderOutput {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: BoundedStderr,
}

async fn exchange_provider(
    command: &ProviderCommand,
    program: &str,
    encoded: &[u8],
) -> Result<ProviderOutput> {
    let mut process = Command::new(&command.program);
    process
        .args(&command.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(current_dir) = &command.current_dir {
        process.current_dir(current_dir);
    }
    let mut child = process.spawn().map_err(|source| ProviderError::Spawn {
        program: program.to_string(),
        source,
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ProviderError::InvalidResponse("provider stdin was not piped".to_string())
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ProviderError::InvalidResponse("provider stdout was not piped".to_string())
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ProviderError::InvalidResponse("provider stderr was not piped".to_string())
    })?;

    let joined = async {
        tokio::try_join!(
            write_request(stdin, encoded, program),
            read_bounded_stdout(stdout, command.max_output_bytes, program),
            read_bounded_stderr(stderr, program),
            async {
                child.wait().await.map_err(|source| ProviderError::Wait {
                    program: program.to_string(),
                    source,
                })
            }
        )
    }
    .await;

    match joined {
        Ok(((), stdout, stderr, status)) => Ok(ProviderOutput {
            status,
            stdout,
            stderr,
        }),
        Err(error) => {
            let _ = child.kill().await;
            Err(error)
        }
    }
}

async fn write_request(
    mut stdin: impl AsyncWrite + Unpin,
    encoded: &[u8],
    program: &str,
) -> Result<()> {
    stdin
        .write_all(encoded)
        .await
        .map_err(|source| ProviderError::WriteRequest {
            program: program.to_string(),
            source,
        })?;
    stdin
        .shutdown()
        .await
        .map_err(|source| ProviderError::WriteRequest {
            program: program.to_string(),
            source,
        })
}

async fn read_bounded_stdout(
    mut stdout: impl AsyncRead + Unpin,
    maximum: usize,
    program: &str,
) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(maximum.min(8 * 1024));
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let size = stdout
            .read(&mut chunk)
            .await
            .map_err(|source| ProviderError::ReadOutput {
                program: program.to_string(),
                stream: "stdout",
                source,
            })?;
        if size == 0 {
            return Ok(output);
        }
        let received = output.len().saturating_add(size);
        if received > maximum {
            return Err(ProviderError::OutputTooLarge {
                program: program.to_string(),
                size: received,
                maximum,
            });
        }
        output.extend_from_slice(&chunk[..size]);
    }
}

struct BoundedStderr {
    bytes: Vec<u8>,
    truncated: bool,
}

impl BoundedStderr {
    fn render(&self) -> String {
        let rendered = String::from_utf8_lossy(&self.bytes);
        if self.truncated {
            format!("{}…", rendered.trim())
        } else {
            rendered.trim().to_string()
        }
    }
}

async fn read_bounded_stderr(
    mut stderr: impl AsyncRead + Unpin,
    program: &str,
) -> Result<BoundedStderr> {
    let mut bounded = BoundedStderr {
        bytes: Vec::with_capacity(MAX_STDERR_BYTES),
        truncated: false,
    };
    let mut chunk = [0_u8; 8 * 1024];
    loop {
        let size = stderr
            .read(&mut chunk)
            .await
            .map_err(|source| ProviderError::ReadOutput {
                program: program.to_string(),
                stream: "stderr",
                source,
            })?;
        if size == 0 {
            return Ok(bounded);
        }
        let remaining = MAX_STDERR_BYTES.saturating_sub(bounded.bytes.len());
        let retained = remaining.min(size);
        bounded.bytes.extend_from_slice(&chunk[..retained]);
        bounded.truncated |= retained < size;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::provider::{
        EVIDENCE_KIND_FORMAT, EVIDENCE_KIND_PROPERTY_NAME, EVIDENCE_KIND_REQUIREDNESS,
        EVIDENCE_KIND_RESPONSE_WRAPPER, EVIDENCE_KIND_TYPE, EndpointSelector, FieldEvidence,
        JSON_SCHEMA_DRAFT_2020_12, ProviderIdentity, SourceFingerprint, fingerprint_bytes,
    };

    fn request() -> ProviderRequest {
        let mut request = ProviderRequest::new(EndpointSelector::new("GET", "/users/{id}", 200));
        request.source_paths = vec!["schema.json".to_string()];
        request
    }

    fn valid_response() -> ProviderResponse {
        let mut response = ProviderResponse::new(
            ProviderIdentity::new("test-provider", "1.0.0"),
            request().endpoint,
            json!({
                "$schema": JSON_SCHEMA_DRAFT_2020_12,
                "type": "object",
                "properties": {"id": {"type": "string", "format": "uuid"}}
            }),
            vec![fingerprint_bytes("schema.json", b"schema")],
        );
        response.evidence = [
            ("#", EVIDENCE_KIND_RESPONSE_WRAPPER),
            ("#/type", EVIDENCE_KIND_TYPE),
            ("#/properties/id", EVIDENCE_KIND_PROPERTY_NAME),
            ("#/properties/id", EVIDENCE_KIND_REQUIREDNESS),
            ("#/properties/id/type", EVIDENCE_KIND_TYPE),
            ("#/properties/id/format", EVIDENCE_KIND_FORMAT),
        ]
        .into_iter()
        .map(|(schema_path, kind)| FieldEvidence {
            schema_path: schema_path.to_string(),
            source_path: "schema.json".to_string(),
            source_location: "User.id".to_string(),
            kind: kind.to_string(),
        })
        .collect();
        response
    }

    #[tokio::test]
    async fn exchanges_versioned_json_over_stdio() {
        let encoded = serde_json::to_string(&valid_response()).unwrap();
        let script = format!("read _request; printf '%s' '{encoded}'");
        let command = ProviderCommand::new("/bin/sh").args(["-c", &script]);

        let response = run_provider(&command, &request()).await.unwrap();

        assert_eq!(response.provider.name, "test-provider");
        assert_eq!(response.source_fingerprints.len(), 1);
    }

    #[tokio::test]
    async fn rejects_protocol_drift_and_endpoint_mismatches() {
        let mut wrong_version = valid_response();
        wrong_version.protocol_version = "2".to_string();
        let encoded = serde_json::to_string(&wrong_version).unwrap();
        let script = format!("read _request; printf '%s' '{encoded}'");
        let error = run_provider(
            &ProviderCommand::new("/bin/sh").args(["-c", &script]),
            &request(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("protocol_version"));

        let mut wrong_endpoint = valid_response();
        wrong_endpoint.endpoint.path = "/other".to_string();
        let encoded = serde_json::to_string(&wrong_endpoint).unwrap();
        let script = format!("read _request; printf '%s' '{encoded}'");
        let error = run_provider(
            &ProviderCommand::new("/bin/sh").args(["-c", &script]),
            &request(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("expected"));
    }

    #[tokio::test]
    async fn reports_nonzero_exit_and_timeout() {
        let exit = ProviderCommand::new("/bin/sh")
            .args(["-c", "read _request; echo provider-failed >&2; exit 7"]);
        let error = run_provider(&exit, &request()).await.unwrap_err();
        assert!(error.to_string().contains("provider-failed"));

        let slow = ProviderCommand::new("/bin/sh")
            .args(["-c", "read _request; sleep 1"])
            .timeout(Duration::from_millis(20));
        let error = run_provider(&slow, &request()).await.unwrap_err();
        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn enforces_stdout_limit_during_streaming() {
        let command = ProviderCommand::new("/bin/sh")
            .args([
                "-c",
                "read _request; while :; do printf '0123456789abcdef'; done",
            ])
            .max_output_bytes(64)
            .timeout(Duration::from_secs(1));

        let error = run_provider(&command, &request()).await.unwrap_err();

        assert!(matches!(
            error,
            ProviderError::OutputTooLarge { maximum: 64, .. }
        ));
    }

    #[tokio::test]
    async fn bounds_stderr_while_draining_the_process() {
        let command = ProviderCommand::new("/bin/sh").args([
            "-c",
            "read _request; i=0; while [ \"$i\" -lt 9000 ]; do printf x >&2; i=$((i + 1)); done; exit 7",
        ]);

        let error = run_provider(&command, &request()).await.unwrap_err();
        let ProviderError::Exit { stderr, .. } = error else {
            panic!("expected provider exit error");
        };

        assert!(stderr.ends_with('…'));
        assert!(stderr.len() <= MAX_STDERR_BYTES + '…'.len_utf8());
    }

    #[tokio::test]
    async fn timeout_includes_a_blocked_stdin_write() {
        let mut large_request = request();
        large_request.source_paths = vec!["x".repeat(2 * 1024 * 1024)];
        let command = ProviderCommand::new("/bin/sh")
            .args(["-c", "sleep 2"])
            .timeout(Duration::from_millis(20));

        let error = run_provider(&command, &large_request).await.unwrap_err();

        assert!(matches!(error, ProviderError::Timeout { .. }));
    }

    #[test]
    fn fingerprint_shape_used_by_runner_is_protocol_stable() {
        let fingerprint = fingerprint_bytes("source", b"value");
        assert_eq!(fingerprint.algorithm, "sha256");
        assert_eq!(fingerprint.digest.len(), 64);
        let _: SourceFingerprint = fingerprint;
    }
}

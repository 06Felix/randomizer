use std::{
    fs::OpenOptions,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Child, Command as ProcessCommand, Stdio},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    process::Command as AsyncCommand,
    sync::oneshot,
};
use tracing::info;

use crate::{
    mock::CompiledMockRegistry, project::ProjectPaths, server::build_project_router,
    state::AppState,
};

use super::{
    CliError,
    args::{ProjectArgs, StartArgs},
};

const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Serialize, Deserialize)]
struct HarnessState {
    project_root: String,
    owner_pid: u32,
    gateway_host: String,
    gateway_port: u16,
}

pub async fn start(args: StartArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    remove_stale_state(&paths).await?;

    if args.foreground {
        return run_harness(&paths).await;
    }

    super::project::verify_paths(&paths)?;
    start_background(&paths).await
}

pub async fn run_project(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    run_harness(&paths).await
}

async fn start_background(paths: &ProjectPaths) -> Result<(), CliError> {
    std::fs::create_dir_all(&paths.runtime).map_err(|source| CliError::Write {
        path: paths.runtime.clone(),
        source,
    })?;
    let log_path = paths.runtime.join("randomizer.log");
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| CliError::Write {
            path: log_path.clone(),
            source,
        })?;
    let stderr = log.try_clone().map_err(|source| CliError::Write {
        path: log_path.clone(),
        source,
    })?;

    let executable = std::env::current_exe().map_err(CliError::CurrentExecutable)?;
    let mut command = ProcessCommand::new(executable);
    command
        .arg("__run-project")
        .arg("--project")
        .arg(&paths.root)
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(stderr));
    detach(&mut command);

    let mut child = command.spawn().map_err(CliError::BackgroundStart)?;
    wait_until_ready(paths, &mut child, &log_path).await
}

async fn wait_until_ready(
    paths: &ProjectPaths,
    child: &mut Child,
    log_path: &Path,
) -> Result<(), CliError> {
    let pid = child.id();
    let state_path = state_path(paths);
    let deadline = tokio::time::Instant::now() + LIFECYCLE_TIMEOUT;

    while tokio::time::Instant::now() < deadline {
        if let Some(status) = child.try_wait().map_err(CliError::BackgroundStart)? {
            return Err(CliError::StartExit {
                status,
                log_path: log_path.to_path_buf(),
            });
        }
        if let Ok(state) = read_state(&state_path)
            && state.owner_pid == pid
            && gateway_is_ready(&state).await
        {
            println!(
                "Randomizer started at http://{}:{} (pid {})",
                state.gateway_host, state.gateway_port, state.owner_pid
            );
            println!("logs: {}", log_path.display());
            return Ok(());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }

    let _ = child.kill();
    let _ = child.wait();
    remove_state_if_owned(&state_path, pid)?;
    Err(CliError::StartTimeout {
        log_path: log_path.to_path_buf(),
    })
}

async fn run_harness(paths: &ProjectPaths) -> Result<(), CliError> {
    super::project::verify_paths(paths)?;
    let manifest = paths.load_manifest()?;
    let registry = CompiledMockRegistry::compile(&manifest, paths)?;
    std::fs::create_dir_all(&paths.runtime).map_err(|source| CliError::Write {
        path: paths.runtime.clone(),
        source,
    })?;
    let state_path = state_path(paths);
    if state_path.exists() {
        return Err(CliError::AlreadyRunning);
    }

    let host = manifest
        .project
        .host
        .parse()
        .map_err(|source| CliError::InvalidHost {
            host: manifest.project.host.clone(),
            source,
        })?;
    let address = SocketAddr::new(host, manifest.project.port);
    let listener = TcpListener::bind(address)
        .await
        .map_err(|source| CliError::Bind { address, source })?;
    let owner_pid = std::process::id();
    write_state(
        &state_path,
        &HarnessState {
            project_root: paths.root.to_string_lossy().into_owned(),
            owner_pid,
            gateway_host: manifest.project.host.clone(),
            gateway_port: manifest.project.port,
        },
    )?;

    let router = build_project_router(AppState::DEFAULT_MAX_CONCURRENT_WS_STREAMS, registry);
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let mut server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            })
            .await
    });
    info!(%address, "Randomizer is ready");
    println!("Randomizer ready at http://{address}");

    let (signal_result, early_server_result) = tokio::select! {
        result = shutdown_signal() => (result, None),
        result = &mut server => (Ok(()), Some(server_result(result))),
    };
    let _ = shutdown_tx.send(());
    let server_result = match early_server_result {
        Some(result) => result,
        None => server_result(server.await),
    };
    let cleanup_result = remove_state_if_owned(&state_path, owner_pid);

    signal_result?;
    server_result?;
    cleanup_result
}

fn server_result(
    result: Result<Result<(), std::io::Error>, tokio::task::JoinError>,
) -> Result<(), CliError> {
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(source)) => Err(CliError::Server(source)),
        Err(source) => Err(CliError::ServerTask(source)),
    }
}

pub async fn status(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let state_path = state_path(&paths);
    if !state_path.exists() {
        println!("Randomizer is stopped");
        return Ok(());
    }

    let state = read_state(&state_path)?;
    if gateway_is_ready(&state).await {
        println!(
            "Randomizer is running at http://{}:{} (pid {})",
            state.gateway_host, state.gateway_port, state.owner_pid
        );
    } else {
        println!(
            "Randomizer is stopped (stale state for pid {})",
            state.owner_pid
        );
    }
    Ok(())
}

pub async fn stop(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let state_path = state_path(&paths);
    if !state_path.exists() {
        println!("Randomizer is already stopped");
        return Ok(());
    }

    let state = read_state(&state_path)?;
    if !gateway_is_ready(&state).await {
        remove_state(&state_path)?;
        println!("Randomizer is already stopped; removed stale runtime state");
        return Ok(());
    }
    if !signal_owner(&state).await {
        return Err(CliError::StopOwnerMismatch {
            pid: state.owner_pid,
        });
    }

    let deadline = tokio::time::Instant::now() + LIFECYCLE_TIMEOUT;
    while tokio::time::Instant::now() < deadline {
        if !state_path.exists() || !gateway_is_ready(&state).await {
            remove_state_if_owned(&state_path, state.owner_pid)?;
            println!("Randomizer stopped");
            return Ok(());
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    Err(CliError::StopTimeout {
        pid: state.owner_pid,
    })
}

async fn remove_stale_state(paths: &ProjectPaths) -> Result<(), CliError> {
    let state_path = state_path(paths);
    if !state_path.exists() {
        return Ok(());
    }
    let state = read_state(&state_path)?;
    if gateway_is_ready(&state).await {
        return Err(CliError::AlreadyRunning);
    }
    remove_state(&state_path)
}

fn state_path(paths: &ProjectPaths) -> PathBuf {
    paths.runtime.join("state.json")
}

fn write_state(path: &Path, state: &HarnessState) -> Result<(), CliError> {
    let encoded = serde_json::to_vec_pretty(state)?;
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, encoded).map_err(|source| CliError::Write {
        path: temporary.clone(),
        source,
    })?;
    std::fs::rename(&temporary, path).map_err(|source| CliError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn read_state(path: &Path) -> Result<HarnessState, CliError> {
    let bytes = std::fs::read(path).map_err(|source| CliError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(Into::into)
}

fn remove_state_if_owned(path: &Path, owner_pid: u32) -> Result<(), CliError> {
    if path.exists() && read_state(path)?.owner_pid == owner_pid {
        remove_state(path)?;
    }
    Ok(())
}

fn remove_state(path: &Path) -> Result<(), CliError> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|source| CliError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

async fn gateway_is_ready(state: &HarnessState) -> bool {
    TcpStream::connect((state.gateway_host.as_str(), state.gateway_port))
        .await
        .is_ok()
}

async fn signal_owner(state: &HarnessState) -> bool {
    if state.owner_pid == std::process::id() {
        return false;
    }
    #[cfg(unix)]
    {
        let owner = AsyncCommand::new("ps")
            .args(["-p", &state.owner_pid.to_string(), "-o", "command="])
            .output()
            .await;
        if !owner.is_ok_and(|output| {
            let command = String::from_utf8_lossy(&output.stdout);
            output.status.success()
                && command.contains("randomizer")
                && command.contains("__run-project")
                && command.contains(&state.project_root)
        }) {
            return false;
        }
        return AsyncCommand::new("kill")
            .arg("-TERM")
            .arg(state.owner_pid.to_string())
            .status()
            .await
            .is_ok_and(|status| status.success());
    }
    #[cfg(windows)]
    {
        let script = format!(
            "$p = Get-CimInstance Win32_Process -Filter \"ProcessId = {}\"; \
             if ($null -eq $p -or $p.Name -ne 'randomizer.exe' -or \
             $p.CommandLine -notlike '*__run-project*') {{ exit 3 }}; \
             Stop-Process -Id {}",
            state.owner_pid, state.owner_pid
        );
        return AsyncCommand::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .status()
            .await
            .is_ok_and(|status| status.success());
    }
    #[allow(unreachable_code)]
    false
}

#[cfg(unix)]
fn detach(command: &mut ProcessCommand) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn detach(command: &mut ProcessCommand) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS);
}

async fn shutdown_signal() -> Result<(), CliError> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(CliError::Signal)?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(CliError::Signal),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.map_err(CliError::Signal)
    }
}

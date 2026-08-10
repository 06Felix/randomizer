use std::{ffi::OsString, net::SocketAddr, path::Path, process::Stdio, time::Duration};

use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    process::{Child, Command},
    sync::oneshot,
};
use tracing::{info, warn};

use crate::{
    adapter::SpringBootAdapter, mock::CompiledMockRegistry, project::ProjectPaths,
    server::build_project_router, state::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
struct HarnessState {
    project_root: String,
    owner_pid: u32,
    gateway_host: String,
    gateway_port: u16,
}

use super::{
    CliError,
    args::{DevArgs, ProjectArgs},
};

pub async fn up(args: ProjectArgs) -> Result<(), CliError> {
    run_harness(&args.project, None).await
}

pub async fn dev(args: DevArgs) -> Result<(), CliError> {
    run_harness(&args.project.project, Some(args.application_command)).await
}

async fn run_harness(
    project: &std::path::Path,
    application: Option<Vec<OsString>>,
) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(project))?;
    super::project::verify_paths(&paths)?;
    let manifest = paths.load_manifest()?;
    let registry = CompiledMockRegistry::compile(&manifest, &paths)?;
    std::fs::create_dir_all(&paths.runtime).map_err(|source| CliError::Write {
        path: paths.runtime.clone(),
        source,
    })?;
    let state_path = paths.runtime.join("state.json");
    if state_path.exists() {
        return Err(CliError::AlreadyRunning);
    }
    if manifest.project.adapter.as_deref() == Some("spring-boot")
        && let Err(error) = SpringBootAdapter::write_overlay(&paths, &manifest)
    {
        return Err(error.into());
    }

    let host = match manifest.project.host.parse() {
        Ok(host) => host,
        Err(source) => {
            return Err(CliError::InvalidHost {
                host: manifest.project.host.clone(),
                source,
            });
        }
    };
    let address = SocketAddr::new(host, manifest.project.port);
    let listener = match TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(source) => return Err(CliError::Bind { address, source }),
    };
    write_state(
        &state_path,
        &HarnessState {
            project_root: paths.root.to_string_lossy().into_owned(),
            owner_pid: std::process::id(),
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
    info!(%address, "Randomizer harness is ready");
    println!("Randomizer ready at http://{address}");

    let mut child = match application {
        Some(command) => match spawn_application(&paths, command) {
            Ok(child) => Some(child),
            Err(error) => {
                let _ = shutdown_tx.send(());
                let _ = server.await;
                remove_state(&state_path)?;
                return Err(error);
            }
        },
        None => None,
    };

    let (application_result, early_server_result) = if let Some(child) = child.as_mut() {
        tokio::select! {
            result = child.wait() => {
                let result = match result {
                    Ok(status) if status.success() => Ok(()),
                    Ok(status) => {
                        warn!(%status, "application process exited unsuccessfully");
                        Err(CliError::ApplicationExit(status))
                    }
                    Err(source) => Err(CliError::ApplicationWait(source)),
                };
                (result, None)
            }
            result = shutdown_signal() => {
                terminate_child(child).await;
                (result, None)
            }
            result = &mut server => {
                terminate_child(child).await;
                (Ok(()), Some(server_result(result)))
            }
        }
    } else {
        tokio::select! {
            result = shutdown_signal() => (result, None),
            result = &mut server => (Ok(()), Some(server_result(result))),
        }
    };

    let _ = shutdown_tx.send(());
    let server_result = match early_server_result {
        Some(result) => result,
        None => server_result(server.await),
    };
    let cleanup_result = remove_state(&state_path);

    application_result?;
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

fn spawn_application(paths: &ProjectPaths, command: Vec<OsString>) -> Result<Child, CliError> {
    let (program, arguments) = command
        .split_first()
        .expect("clap requires an application command");
    Command::new(program)
        .args(arguments)
        .current_dir(&paths.root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| CliError::ApplicationSpawn {
            program: program.to_string_lossy().into_owned(),
            source,
        })
}

async fn terminate_child(child: &mut Child) {
    if let Err(error) = child.start_kill() {
        warn!(%error, "failed to signal application child");
    }
    let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
}

pub async fn status(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let state = read_state(&paths.runtime.join("state.json"))?;
    let gateway = TcpStream::connect((state.gateway_host.as_str(), state.gateway_port))
        .await
        .is_ok();
    println!(
        "gateway: {} ({}:{})",
        if gateway { "up" } else { "down" },
        state.gateway_host,
        state.gateway_port
    );
    Ok(())
}

pub async fn down(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let state_path = paths.runtime.join("state.json");
    let state = read_state(&state_path)?;
    if signal_owner(state.owner_pid).await {
        let graceful_deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while state_path.exists() && tokio::time::Instant::now() < graceful_deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !state_path.exists() {
            return Ok(());
        }
    }
    remove_state(&state_path)
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

fn remove_state(path: &Path) -> Result<(), CliError> {
    if path.exists() {
        std::fs::remove_file(path).map_err(|source| CliError::Write {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

async fn signal_owner(pid: u32) -> bool {
    if pid == std::process::id() {
        return false;
    }
    #[cfg(unix)]
    {
        let owner = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "command="])
            .output()
            .await;
        if !owner.is_ok_and(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("randomizer")
        }) {
            return false;
        }
        return Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()
            .await
            .is_ok_and(|status| status.success());
    }
    #[cfg(windows)]
    {
        return Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T"])
            .status()
            .await
            .is_ok_and(|status| status.success());
    }
    #[allow(unreachable_code)]
    false
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

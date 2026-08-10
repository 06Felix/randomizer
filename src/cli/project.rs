use std::net::SocketAddr;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    mock::CompiledMockRegistry,
    project::{ProjectPaths, validate_manifest},
};

use super::{CliError, args::ProjectArgs};

pub fn verify(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    verify_paths(&paths)?;
    Ok(())
}

pub(crate) fn verify_paths(paths: &ProjectPaths) -> Result<(), CliError> {
    let manifest = paths.load_manifest()?;
    validate_manifest(&manifest)?;
    let registry = CompiledMockRegistry::compile(&manifest, paths)?;
    println!(
        "verified {} HTTP services and {} routes",
        manifest.services.len(),
        registry.route_ids().len(),
    );
    Ok(())
}

pub fn inspect(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let manifest = paths.load_manifest()?;
    println!("project: {}", manifest.project.name);
    for service in &manifest.services {
        println!(
            "http service: {}{}",
            service.id,
            service
                .config_key
                .as_deref()
                .map_or(String::new(), |key| format!(" ({key})"))
        );
    }
    for route in &manifest.routes {
        println!(
            "route: {} {} {}",
            route.id,
            route.request_match.method.as_deref().unwrap_or("ANY"),
            route.request_match.path
        );
    }
    Ok(())
}

pub async fn reset(args: ProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project))?;
    let manifest = paths.load_manifest()?;
    let address: SocketAddr = format!("{}:{}", manifest.project.host, manifest.project.port)
        .parse()
        .map_err(|source| CliError::InvalidHost {
            host: manifest.project.host.clone(),
            source,
        })?;
    let mut stream = TcpStream::connect(address)
        .await
        .map_err(|source| CliError::ResetConnect { address, source })?;
    let request = format!(
        "POST /__randomizer/reset HTTP/1.1\r\nHost: {address}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(CliError::ResetIo)?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .map_err(CliError::ResetIo)?;
    if !response.starts_with(b"HTTP/1.1 204") {
        return Err(CliError::ResetRejected(
            String::from_utf8_lossy(&response)
                .lines()
                .next()
                .unwrap_or("invalid response")
                .to_string(),
        ));
    }
    Ok(())
}

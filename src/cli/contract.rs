use std::{fs, path::Path, time::Duration};

use crate::{
    project::{
        ContractError, ContractRecipe, ManagedContract, ProjectPaths, check_managed_contracts,
        managed_contract, save_managed_contract, save_managed_contract_if_unchanged,
    },
    provider::{
        EndpointSelector, ProviderCommand, ProviderError, ProviderRequest, ProviderResponse,
        import_json_schema_bytes, import_openapi_response_bytes, import_serialized_example_bytes,
        run_provider,
    },
};

use super::{
    CliError,
    args::{
        ContractAnalyzeArgs, ContractArgs, ContractCheckArgs, ContractCommand, ContractImportArgs,
        ContractRefreshArgs, ContractSourceFormat,
    },
};

pub async fn contract(args: ContractArgs) -> Result<(), CliError> {
    match args.command {
        ContractCommand::Import(args) => import(args),
        ContractCommand::Analyze(args) => analyze(args).await,
        ContractCommand::Refresh(args) => refresh(args).await,
        ContractCommand::Check(args) => check(args),
    }
}

fn import(args: ContractImportArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let (source, contents) = read_source(&paths, &args.source)?;
    let endpoint = endpoint(args.method, args.endpoint, args.status, args.media_type);
    let root_symbol = args.root_symbol;
    let (response, recipe) = match args.format {
        ContractSourceFormat::JsonSchema => {
            let response = import_json_schema_bytes(&source, &contents, endpoint, root_symbol)?;
            let endpoint = response.endpoint.clone();
            (response, ContractRecipe::JsonSchema { source, endpoint })
        }
        ContractSourceFormat::Openapi => {
            let response =
                import_openapi_response_bytes(&source, &contents, endpoint, root_symbol)?;
            let endpoint = response.endpoint.clone();
            (response, ContractRecipe::Openapi { source, endpoint })
        }
        ContractSourceFormat::SerializedExample => {
            let response =
                import_serialized_example_bytes(&source, &contents, endpoint, root_symbol)?;
            let endpoint = response.endpoint.clone();
            (
                response,
                ContractRecipe::SerializedExample { source, endpoint },
            )
        }
    };
    save_and_report(
        &paths,
        &args.name,
        &args.contract_version,
        response,
        recipe,
        None,
    )
}

async fn analyze(args: ContractAnalyzeArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let source_paths = args
        .source
        .iter()
        .map(|source| project_relative(source))
        .collect::<Result<Vec<_>, _>>()?;
    for source in &source_paths {
        resolve_relative_source(&paths, source)?;
    }
    let endpoint = endpoint(args.method, args.endpoint, args.status, args.media_type);
    let mut request = ProviderRequest::new(endpoint);
    request.root_symbol = args.root_symbol;
    request.source_paths = source_paths;
    let program = args.provider.to_string_lossy().to_string();
    let command = ProviderCommand::new(&args.provider)
        .args(args.provider_args.iter())
        .current_dir(&paths.root)
        .timeout(Duration::from_secs(args.timeout_seconds));
    let response = run_provider(&command, &request).await?;
    let recipe = ContractRecipe::External {
        program,
        arguments: args.provider_args,
        request,
    };
    save_and_report(
        &paths,
        &args.name,
        &args.contract_version,
        response,
        recipe,
        None,
    )
}

async fn refresh(args: ContractRefreshArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let managed = managed_contract(&paths, &args.name)?;
    let recipe = managed.recipe.clone();
    let response = match &recipe {
        ContractRecipe::JsonSchema { source, endpoint } => {
            let contents = read_relative_source(&paths, source)?;
            import_json_schema_bytes(
                source,
                &contents,
                endpoint.clone(),
                managed.root_symbol.clone(),
            )?
        }
        ContractRecipe::Openapi { source, endpoint } => {
            let contents = read_relative_source(&paths, source)?;
            import_openapi_response_bytes(
                source,
                &contents,
                endpoint.clone(),
                managed.root_symbol.clone(),
            )?
        }
        ContractRecipe::SerializedExample { source, endpoint } => {
            let contents = read_relative_source(&paths, source)?;
            import_serialized_example_bytes(
                source,
                &contents,
                endpoint.clone(),
                managed.root_symbol.clone(),
            )?
        }
        ContractRecipe::External {
            program,
            arguments,
            request,
        } => {
            for source in &request.source_paths {
                resolve_relative_source(&paths, source)?;
            }
            let command = ProviderCommand::new(program)
                .args(arguments)
                .current_dir(&paths.root)
                .timeout(Duration::from_secs(args.timeout_seconds));
            run_provider(&command, request).await?
        }
    };
    save_and_report(
        &paths,
        &args.name,
        &managed.contract_version,
        response,
        recipe,
        Some(&managed),
    )
}

fn check(args: ContractCheckArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let report = check_managed_contracts(&paths, args.name.as_deref())?;
    println!("verified {} managed contracts", report.names.len());
    Ok(())
}

fn save_and_report(
    paths: &ProjectPaths,
    name: &str,
    contract_version: &str,
    response: ProviderResponse,
    recipe: ContractRecipe,
    expected: Option<&ManagedContract>,
) -> Result<(), CliError> {
    let diagnostics = response.diagnostics.clone();
    let managed = match expected {
        Some(expected) => save_managed_contract_if_unchanged(
            paths,
            name,
            contract_version,
            response,
            recipe,
            expected,
        )?,
        None => save_managed_contract(paths, name, contract_version, response, recipe)?,
    };
    println!(
        "saved managed contract {} from {} {} at {}",
        name, managed.provider.name, managed.provider.version, managed.artifact
    );
    for diagnostic in diagnostics {
        eprintln!(
            "{:?} [{}]: {}",
            diagnostic.severity, diagnostic.code, diagnostic.message
        );
    }
    Ok(())
}

fn endpoint(
    method: String,
    path: String,
    status: u16,
    media_type: Option<String>,
) -> EndpointSelector {
    let endpoint = EndpointSelector::new(method, path, status);
    match media_type {
        Some(media_type) => endpoint.with_media_type(media_type),
        None => endpoint,
    }
}

fn read_source(paths: &ProjectPaths, source: &Path) -> Result<(String, Vec<u8>), CliError> {
    let relative = project_relative(source)?;
    let contents = read_relative_source(paths, &relative)?;
    Ok((relative, contents))
}

fn read_relative_source(paths: &ProjectPaths, source: &str) -> Result<Vec<u8>, CliError> {
    let path = resolve_relative_source(paths, source)?;
    fs::read(&path)
        .map_err(|source| ProviderError::ReadSource { path, source })
        .map_err(CliError::from)
}

fn resolve_relative_source(
    paths: &ProjectPaths,
    source: &str,
) -> Result<std::path::PathBuf, CliError> {
    let normalized = project_relative(Path::new(source))?;
    if normalized != source {
        return Err(ContractError::UnsafeSource(source.to_string()).into());
    }
    let candidate = paths.root.join(source);
    let resolved = candidate
        .canonicalize()
        .map_err(|source| ProviderError::ReadSource {
            path: candidate,
            source,
        })?;
    if !resolved.starts_with(&paths.root) || !resolved.is_file() {
        return Err(ContractError::SourceOutsideProject {
            path: source.to_string(),
            root: paths.root.clone(),
        }
        .into());
    }
    Ok(resolved)
}

fn project_relative(path: &Path) -> Result<String, CliError> {
    let display = path.to_string_lossy().to_string();
    if display.is_empty() {
        return Err(ContractError::UnsafeSource(display).into());
    }
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(ContractError::UnsafeSource(display).into());
    }
    candidate
        .components()
        .map(|component| match component {
            std::path::Component::Normal(segment) => segment.to_str().ok_or_else(|| {
                ContractError::UnsafeSource(path.to_string_lossy().to_string()).into()
            }),
            _ => unreachable!("non-normal components were rejected"),
        })
        .collect::<Result<Vec<_>, CliError>>()
        .map(|segments| segments.join("/"))
}

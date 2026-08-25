use crate::project::{ProjectPaths, apply_wiring, check_wiring};

use super::{
    CliError,
    args::{WiringArgs, WiringCommand, WiringProjectArgs},
};

pub fn wiring(args: WiringArgs) -> Result<(), CliError> {
    match args.command {
        WiringCommand::Apply(args) => apply(args),
        WiringCommand::Check(args) => check(args),
    }
}

fn apply(args: WiringProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let manifest = paths.load_manifest()?;
    let report = apply_wiring(&paths.root, &manifest, args.service.as_deref())?;
    println!(
        "applied {} endpoint wiring entries ({} changed)",
        report.entries.len(),
        report.changed_count()
    );
    Ok(())
}

fn check(args: WiringProjectArgs) -> Result<(), CliError> {
    let paths = ProjectPaths::discover(Some(&args.project.project))?;
    let manifest = paths.load_manifest()?;
    let report = check_wiring(&paths.root, &manifest, args.service.as_deref())?;
    println!("verified {} endpoint wiring entries", report.entries.len());
    Ok(())
}

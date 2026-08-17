use std::{net::TcpListener, path::Path, process::Command};

use randomizer::project::{ProjectPaths, validate_manifest};

fn run_randomizer(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_randomizer"))
        .args(arguments)
        .output()
        .unwrap()
}

fn available_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn initialize(root: &Path) {
    let root = root.to_str().unwrap();
    let initialized = run_randomizer(&["init", root]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );

    let paths = ProjectPaths::discover(Some(Path::new(root))).unwrap();
    let mut manifest = paths.load_manifest().unwrap();
    manifest.project.port = available_port();
    validate_manifest(&manifest).unwrap();
    std::fs::write(&paths.manifest, serde_yaml::to_string(&manifest).unwrap()).unwrap();
}

struct RunningProject<'a> {
    root: &'a str,
}

impl Drop for RunningProject<'_> {
    fn drop(&mut self) {
        let _ = run_randomizer(&["stop", "--project", self.root]);
    }
}

#[test]
fn exposes_the_generic_project_commands() {
    let help = run_randomizer(&["--help"]);
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("  start"));
    assert!(stdout.contains("  stop"));
    assert!(stdout.contains("  skill"));
    assert!(!stdout.contains("  up"));
    assert!(!stdout.contains("  down"));
    assert!(!stdout.contains("  dev"));
    assert!(!stdout.contains("  contract"));

    let init_help = run_randomizer(&["init", "--help"]);
    let init_stdout = String::from_utf8_lossy(&init_help.stdout);
    assert!(!init_stdout.contains("--adapter"));
    assert!(!init_stdout.contains("--no-apply"));

    let skill_help = run_randomizer(&["skill", "sync", "--help"]);
    assert!(skill_help.status.success());
    let skill_stdout = String::from_utf8_lossy(&skill_help.stdout);
    assert!(skill_stdout.contains("--project"));
    assert!(skill_stdout.contains("--force"));
}

#[test]
fn starts_reports_and_stops_a_background_service() {
    let directory = tempfile::tempdir().unwrap();
    initialize(directory.path());
    let root = directory.path().to_str().unwrap();

    let started = run_randomizer(&["start", "--project", root]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let _running = RunningProject { root };
    assert!(String::from_utf8_lossy(&started.stdout).contains("Randomizer started"));

    let status = run_randomizer(&["status", "--project", root]);
    assert!(status.status.success());
    assert!(String::from_utf8_lossy(&status.stdout).contains("Randomizer is running"));

    let duplicate = run_randomizer(&["start", "--project", root]);
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already running"));

    let stopped = run_randomizer(&["stop", "--project", root]);
    assert!(
        stopped.status.success(),
        "{}",
        String::from_utf8_lossy(&stopped.stderr)
    );
    assert!(String::from_utf8_lossy(&stopped.stdout).contains("Randomizer stopped"));

    let stopped_status = run_randomizer(&["status", "--project", root]);
    assert!(stopped_status.status.success());
    assert!(String::from_utf8_lossy(&stopped_status.stdout).contains("Randomizer is stopped"));

    let stopped_again = run_randomizer(&["stop", "--project", root]);
    assert!(stopped_again.status.success());
    assert!(String::from_utf8_lossy(&stopped_again.stdout).contains("already stopped"));
}

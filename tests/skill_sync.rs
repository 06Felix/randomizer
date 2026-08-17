use std::{fs, path::Path, process::Command};

fn run_randomizer(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_randomizer"))
        .args(arguments)
        .output()
        .unwrap()
}

fn initialize(root: &Path) {
    let initialized = run_randomizer(&["init", root.to_str().unwrap()]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
}

#[test]
fn refuses_to_replace_local_skill_changes_without_force() {
    let directory = tempfile::tempdir().unwrap();
    initialize(directory.path());
    let root = directory.path().to_str().unwrap();
    let skill = directory
        .path()
        .join(".agents/skills/randomizer-mocks/SKILL.md");
    let bundled = fs::read_to_string(&skill).unwrap();
    fs::write(&skill, format!("{bundled}\nLocal repository guidance.\n")).unwrap();

    let refused = run_randomizer(&["skill", "sync", "--project", root]);

    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("locally modified skill files"));
    assert!(
        fs::read_to_string(&skill)
            .unwrap()
            .contains("Local repository guidance.")
    );

    let forced = run_randomizer(&["skill", "sync", "--project", root, "--force"]);

    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert_eq!(fs::read_to_string(&skill).unwrap(), bundled);
}

#[test]
fn restores_missing_files_and_reports_current_after_sync() {
    let directory = tempfile::tempdir().unwrap();
    initialize(directory.path());
    let root = directory.path().to_str().unwrap();
    let metadata = directory
        .path()
        .join(".agents/skills/randomizer-mocks/agents/openai.yaml");
    fs::remove_file(&metadata).unwrap();

    let restored = run_randomizer(&["skill", "sync", "--project", root]);

    assert!(
        restored.status.success(),
        "{}",
        String::from_utf8_lossy(&restored.stderr)
    );
    assert!(metadata.is_file());
    assert!(String::from_utf8_lossy(&restored.stdout).contains("updated"));

    let current = run_randomizer(&["skill", "sync", "--project", root]);

    assert!(current.status.success());
    assert!(String::from_utf8_lossy(&current.stdout).contains("is current"));
}

#[test]
fn force_recovers_an_invalid_lock_before_project_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().to_str().unwrap();
    let lock = directory.path().join(".randomizer/skills.lock.json");
    fs::create_dir_all(lock.parent().unwrap()).unwrap();
    fs::write(&lock, "not-json").unwrap();

    let refused = run_randomizer(&["skill", "sync", "--project", root]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("invalid skill lock"));

    let forced = run_randomizer(&["skill", "sync", "--project", root, "--force"]);

    assert!(
        forced.status.success(),
        "{}",
        String::from_utf8_lossy(&forced.stderr)
    );
    assert!(
        directory
            .path()
            .join(".agents/skills/randomizer-mocks/SKILL.md")
            .is_file()
    );
    for reference in [
        "contracts.md",
        "runtime-capabilities.md",
        "generic-wire-contract.md",
        "languages/java.md",
        "languages/typescript.md",
        "languages/python.md",
        "languages/go.md",
        "languages/rust.md",
    ] {
        assert!(
            directory
                .path()
                .join(format!(
                    ".agents/skills/randomizer-mocks/references/{reference}"
                ))
                .is_file(),
            "missing bundled reference {reference}"
        );
    }
    assert!(serde_json::from_slice::<serde_json::Value>(&fs::read(lock).unwrap()).is_ok());
}

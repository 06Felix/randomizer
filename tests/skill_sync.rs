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

#[test]
fn installed_skill_preserves_randomized_response_intent() {
    let directory = tempfile::tempdir().unwrap();
    initialize(directory.path());
    let skill_root = directory.path().join(".agents/skills/randomizer-mocks");
    let skill = fs::read_to_string(skill_root.join("SKILL.md")).unwrap();
    let contracts = fs::read_to_string(skill_root.join("references/contracts.md")).unwrap();
    let generic =
        fs::read_to_string(skill_root.join("references/generic-wire-contract.md")).unwrap();
    let runtime =
        fs::read_to_string(skill_root.join("references/runtime-capabilities.md")).unwrap();

    for required_guard in [
        "explicitly asks for randomized or dynamic responses",
        "Never invent business values",
        ".randomizer/sources/",
        "task_slug",
        "taskType",
        "selected local profile/settings",
        "locally derived output fields",
        "omit local `task_type`",
        "consumer must receive",
        "unambiguously matches RFC 3339",
        "does not prove time-zone policy",
        "body.contract",
        "coerce: integer",
        "randomizer verify --help",
        "--require-managed-contract-route",
        "stop before changing files",
        "at least three contract-backed runtime",
        "fixture fallback: none",
        "application-start command: not run/not applicable",
    ] {
        assert!(
            skill.contains(required_guard),
            "installed skill is missing randomized-response guard {required_guard:?}"
        );
    }
    for required_detail in [
        "managed contract envelope",
        "explicit user agreement",
        "Derive an auditable source schema",
        "Unknown plain-string domains",
        "task_slug",
        "taskType",
        "selected local",
        "local enrichment",
        "Do not",
        "put those values in a `taskType` enum",
        "consumer must receive",
        "unambiguously matches RFC 3339",
        "does not establish",
        "at least three successful runtime samples",
        "at least two distinct valid values",
    ] {
        assert!(
            contracts.contains(required_detail),
            "installed contract reference is missing {required_detail:?}"
        );
    }
    assert!(runtime.contains("Only a managed `contract` body"));
    assert!(runtime.contains("Multi-sample dynamic validation"));
    assert!(runtime.contains("upstream HTTP boundary"));
    assert!(runtime.contains("local enrichment"));
    assert!(runtime.contains("selected local profile/settings"));
    assert!(runtime.contains("coerce: integer"));
    assert!(runtime.contains("--require-managed-contract-route"));
    assert!(generic.contains("upstream HTTP boundary"));
    assert!(generic.contains("Unknown plain-string domains"));

    for language in ["go", "java", "python", "rust", "typescript"] {
        let guidance =
            fs::read_to_string(skill_root.join(format!("references/languages/{language}.md")))
                .unwrap();
        assert!(
            guidance.contains("static")
                && guidance.contains("fallback")
                && guidance.contains("explicit user")
                && guidance.contains("agreement")
                && guidance.contains(".randomizer/sources/"),
            "{language} guidance permits an implicit fixture downgrade"
        );
    }
}

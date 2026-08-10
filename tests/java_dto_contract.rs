use std::{fs, process::Command};

use serde_json::Value;

fn run_randomizer(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_randomizer"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn imports_and_checks_a_maven_java_dto_contract() {
    let directory = tempfile::tempdir().unwrap();
    fs::write(
        directory.path().join("pom.xml"),
        include_str!("fixtures/java-maven/pom.xml"),
    )
    .unwrap();
    let sources = directory.path().join("src/main/java/example");
    fs::create_dir_all(&sources).unwrap();
    fs::write(
        sources.join("ServiceResponse.java"),
        include_str!("fixtures/java-maven/src/main/java/example/ServiceResponse.java"),
    )
    .unwrap();
    fs::write(
        sources.join("TaskDto.java"),
        include_str!("fixtures/java-maven/src/main/java/example/TaskDto.java"),
    )
    .unwrap();
    fs::write(
        sources.join("Location.java"),
        include_str!("fixtures/java-maven/src/main/java/example/Location.java"),
    )
    .unwrap();

    let root = directory.path().to_str().unwrap();
    let initialized = run_randomizer(&["init", root, "--no-apply"]);
    assert!(
        initialized.status.success(),
        "{}",
        String::from_utf8_lossy(&initialized.stderr)
    );
    let imported = run_randomizer(&[
        "contract",
        "import-java",
        "--project",
        root,
        "--name",
        "service-task-response",
        "--type",
        "example.ServiceResponse<example.TaskDto>",
    ]);
    assert!(
        imported.status.success(),
        "{}",
        String::from_utf8_lossy(&imported.stderr)
    );

    let contract_path = directory
        .path()
        .join(".randomizer/contracts/service-task-response.json");
    let contract: Value = serde_json::from_slice(&fs::read(contract_path).unwrap()).unwrap();
    let encoded = contract["schema"].to_string();
    assert!(encoded.contains("task_id"));
    assert!(encoded.contains("master_task_config_slug"));
    assert!(encoded.contains("date-time"));
    assert!(encoded.contains("requested_locations"));

    let checked = run_randomizer(&["contract", "check", "--project", root]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );

    let mut corrupted = contract;
    corrupted["content_hash"] = Value::String("invalid".into());
    fs::write(
        directory
            .path()
            .join(".randomizer/contracts/service-task-response.json"),
        serde_json::to_vec_pretty(&corrupted).unwrap(),
    )
    .unwrap();
    let stale = run_randomizer(&["contract", "check", "--project", root]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale Java DTO contracts"));

    let refreshed = run_randomizer(&["contract", "refresh", "--project", root]);
    assert!(
        refreshed.status.success(),
        "{}",
        String::from_utf8_lossy(&refreshed.stderr)
    );
}

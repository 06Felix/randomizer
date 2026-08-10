use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=RANDOMIZER_JAVA_EXPORTER_JAR");
    println!("cargo:rerun-if-changed=tools/java-dto-exporter/pom.xml");
    println!("cargo:rerun-if-changed=tools/java-dto-exporter/src");

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let output =
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("randomizer-java-dto-exporter.jar");

    let source = if let Some(configured) = env::var_os("RANDOMIZER_JAVA_EXPORTER_JAR") {
        PathBuf::from(configured)
    } else {
        let pom = manifest_dir.join("tools/java-dto-exporter/pom.xml");
        let mut command = if cfg!(windows) {
            let mut command = Command::new("cmd");
            command.args(["/C", "mvn"]);
            command
        } else {
            Command::new("mvn")
        };
        let status = command
            .args(["-q", "-DskipTests", "package", "-f"])
            .arg(&pom)
            .status()
            .unwrap_or_else(|error| {
                panic!("failed to start Maven while building the Java DTO exporter: {error}")
            });
        assert!(
            status.success(),
            "failed to build tools/java-dto-exporter; run `mvn -f tools/java-dto-exporter/pom.xml package` for details"
        );
        manifest_dir.join("tools/java-dto-exporter/target/randomizer-java-dto-exporter.jar")
    };

    fs::copy(&source, &output).unwrap_or_else(|error| {
        panic!(
            "failed to copy Java DTO exporter from {} to {}: {error}",
            source.display(),
            output.display()
        )
    });
}

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn write_project(base: &Path, sub: &str, target_size_bytes: usize) {
    let dir = base.join(sub);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"{sub}\"\nversion = \"0.1.0\"\n"),
    )
    .unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(dir.join("target/debug")).unwrap();
    // Write a file of the requested size into target/
    let data = vec![b'x'; target_size_bytes];
    fs::write(dir.join("target/debug/artifact"), &data).unwrap();
}

#[test]
fn min_size_filters_small_projects() {
    let temp = TempDir::new().unwrap();
    // small: 1 KB, large: 1 MB
    write_project(temp.path(), "small", 1024);
    write_project(temp.path(), "large", 1024 * 1024);

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg(temp.path())
        .arg("--min-size")
        .arg("100KB")
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert_eq!(summary["total_projects"], 1, "only the large project should be cleaned");

    // small project's target should still exist, large should be cleaned
    assert!(
        temp.path().join("small/target").exists(),
        "small project should be skipped by --min-size"
    );
    assert!(
        !temp.path().join("large/target").exists(),
        "large project should be cleaned"
    );
}

#[test]
fn exclude_pattern_skips_directories() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "keep", 1024);
    let skip_dir = temp.path().join("skip");
    fs::create_dir_all(skip_dir.join("src")).unwrap();
    fs::write(
        skip_dir.join("Cargo.toml"),
        "[package]\nname = \"skip\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(skip_dir.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(skip_dir.join("target/debug")).unwrap();
    fs::write(skip_dir.join("target/debug/artifact"), "data").unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg(temp.path())
        .arg("--exclude")
        .arg("**/skip")
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert_eq!(summary["total_projects"], 1, "excluded project should be skipped");

    // skip/target should still exist, keep/target should be cleaned
    assert!(
        temp.path().join("skip/target").exists(),
        "excluded project target should be preserved"
    );
    assert!(
        !temp.path().join("keep/target").exists(),
        "non-excluded project should be cleaned"
    );
}

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn write_project(base: &Path, sub: &str) {
    let dir = base.join(sub);
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("Cargo.toml"),
        format!("[package]\nname = \"{sub}\"\nversion = \"0.1.0\"\n"),
    )
    .unwrap();
    fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(dir.join("target/debug")).unwrap();
    fs::write(dir.join("target/debug/artifact"), "build output").unwrap();
}

#[test]
fn dry_run_preserves_all_target_dirs() {
    let temp = TempDir::new().unwrap();
    for sub in ["proj-a", "proj-b"] {
        write_project(temp.path(), sub);
    }

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg(temp.path())
        .arg("--dry-run")
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        output.status.success(),
        "dry run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // target dirs must still exist after dry run
    for sub in ["proj-a", "proj-b"] {
        assert!(
            temp.path().join(sub).join("target").exists(),
            "target should be preserved in dry run for {sub}"
        );
    }

    // JSON should report the size that would be freed
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("dry run should output valid JSON");
    assert_eq!(summary["total_projects"], 2);
    assert_eq!(summary["cleaned"], 2);
    assert!(summary["total_freed_bytes"].as_u64().unwrap() > 0);
}

#[test]
fn dry_run_via_subcommand_path_preserves_targets() {
    // Regression test for the subcommand flag-swallowing bug:
    // `cargo deepclean --dry-run` previously swallowed --dry-run as argv[0].
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "regression");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg(temp.path().join("regression"))
        .arg("--dry-run")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(output.status.success());

    // If --dry-run was swallowed, the target would be deleted.
    assert!(
        temp.path().join("regression/target").exists(),
        "--dry-run via subcommand path must preserve target (regression check)"
    );
}

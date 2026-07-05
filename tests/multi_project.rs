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
fn end_to_end_cleans_multiple_subfolder_projects() {
    let temp = TempDir::new().unwrap();
    let subs = ["services/api", "apps/web", "tools/cli"];
    for sub in subs {
        write_project(temp.path(), sub);
    }

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        output.status.success(),
        "clean failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for sub in subs {
        assert!(
            !temp.path().join(sub).join("target").exists(),
            "target should be removed for {sub}"
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("output should be valid JSON");
    assert_eq!(summary["total_projects"], 3);
    assert_eq!(summary["cleaned"], 3);
    assert_eq!(summary["failed"], 0);
}

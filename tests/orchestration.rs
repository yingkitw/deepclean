use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
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

fn run(bin: &str, args: &[&str]) -> std::process::Output {
    Command::new(bin)
        .args(args)
        .output()
        .expect("failed to run cargo-deepclean")
}

#[test]
fn verbose_dry_run_reports_cleaning_messages() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "verbose-proj");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--dry-run",
            "--verbose",
        ],
    );

    assert!(
        output.status.success(),
        "verbose dry run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleaning:"), "verbose mode should announce each project");
    assert!(stdout.contains("DRY RUN MODE"));
    assert!(stdout.contains("Found 1 project(s)"));
    assert!(
        temp.path().join("verbose-proj/target").exists(),
        "dry run must preserve target"
    );
}

#[test]
fn clean_deps_dry_run_reports_unused_without_removing() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("deps-proj");
    write_project(temp.path(), "deps-proj");
    // Add a dependency that is never referenced in source.
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"deps-proj\"\nversion = \"0.1.0\"\n\n[dependencies]\nleftpad = \"1.0\"\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--clean-deps",
            "--dry-run",
        ],
    );

    assert!(
        output.status.success(),
        "clean-deps dry run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unused dependency"),
        "should report unused dependencies, got: {stdout}"
    );
    assert!(
        stdout.contains("leftpad"),
        "should name the unused dependency"
    );
    assert!(
        stdout.contains("Would remove"),
        "dry run should suggest --remove-deps instead of removing"
    );
    assert!(
        dir.join("Cargo.toml").exists(),
        "dry run must not modify Cargo.toml"
    );
    assert!(
        dir.join("target").exists(),
        "dry run must preserve target"
    );
}

#[test]
fn no_projects_found_warns_and_succeeds() {
    let temp = TempDir::new().unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(bin, &["deepclean", temp.path().to_str().unwrap(), "--dry-run"]);

    assert!(
        output.status.success(),
        "no projects should not be an error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No Cargo projects found"));
}

#[test]
fn min_size_threshold_filtering_to_empty_reports_info() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "tiny-proj");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--dry-run",
            "--min-size",
            "1GB",
        ],
    );

    assert!(
        output.status.success(),
        "min-size filtering failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No projects found above the minimum size threshold"));
    assert!(
        temp.path().join("tiny-proj/target").exists(),
        "filtered-out project must be preserved"
    );
}

#[test]
fn invalid_min_size_fails_with_helpful_error() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "any-proj");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--min-size",
            "nonsense",
        ],
    );

    assert!(
        !output.status.success(),
        "invalid --min-size must exit non-zero"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Invalid min-size"),
        "error should explain the min-size format, got: {stderr}"
    );
}

#[test]
fn interactive_cancel_preserves_target() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "interactive-proj");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let mut child = Command::new(bin)
        .arg("deepclean")
        .arg(temp.path().to_str().unwrap())
        .arg("--interactive")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn cargo-deepclean");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"n\n").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait");
    assert!(
        output.status.success(),
        "cancelling should exit cleanly: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cancelled by user"));
    assert!(
        temp.path().join("interactive-proj/target").exists(),
        "cancelling must preserve target"
    );
}

#[test]
fn json_mode_suppresses_human_chatter() {
    let temp = TempDir::new().unwrap();
    write_project(temp.path(), "json-proj");

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--dry-run",
            "--json",
            "--verbose",
        ],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("JSON mode must emit parseable stdout even with --verbose");
    assert_eq!(summary["total_projects"], 1);
    assert!(
        !stdout.contains("DRY RUN MODE"),
        "human chatter must not leak into JSON output"
    );
}

#[test]
fn json_clean_deps_includes_unused_deps_in_results() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("json-deps");
    write_project(temp.path(), "json-deps");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"json-deps\"\nversion = \"0.1.0\"\n\n[dependencies]\nleftpad = \"1.0\"\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--clean-deps",
            "--dry-run",
            "--json",
        ],
    );

    assert!(
        output.status.success(),
        "json clean-deps run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("should output valid JSON");
    let results = summary["results"].as_array().expect("results array");
    assert_eq!(results.len(), 1);
    let unused = results[0]["unused_deps"]
        .as_array()
        .expect("--clean-deps should add unused_deps to results");
    assert_eq!(unused.len(), 1);
    assert_eq!(unused[0]["name"], "leftpad");
    assert_eq!(unused[0]["location"], "[dependencies]");
}

#[test]
fn json_without_clean_deps_omits_unused_deps_field() {
    let temp = TempDir::new().unwrap();
    let dir = temp.path().join("plain-json");
    write_project(temp.path(), "plain-json");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"plain-json\"\nversion = \"0.1.0\"\n\n[dependencies]\nleftpad = \"1.0\"\n",
    )
    .unwrap();

    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = run(
        bin,
        &[
            "deepclean",
            temp.path().to_str().unwrap(),
            "--dry-run",
            "--json",
        ],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let summary: serde_json::Value =
        serde_json::from_str(&stdout).expect("should output valid JSON");
    let results = summary["results"].as_array().expect("results array");
    assert!(
        results[0].get("unused_deps").is_none(),
        "unused_deps must be absent from JSON unless --clean-deps ran (contract stability)"
    );
}

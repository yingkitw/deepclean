use std::process::Command;
use std::io::Write;

#[test]
fn caches_json_entries_have_required_fields() {
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let entries: serde_json::Value =
        serde_json::from_str(&stdout).expect("caches --json should output valid JSON");
    let arr = entries.as_array().expect("should be a JSON array");

    for (i, entry) in arr.iter().enumerate() {
        assert!(entry["id"].is_string(), "entry {i} missing id");
        assert!(entry["name"].is_string(), "entry {i} missing name");
        assert!(entry["category"].is_string(), "entry {i} missing category");
        assert!(entry["risk"].is_string(), "entry {i} missing risk");
        assert!(entry["size_bytes"].is_u64(), "entry {i} missing size_bytes");

        let risk = entry["risk"].as_str().unwrap();
        assert!(
            risk == "safe" || risk == "heavy",
            "entry {i} has invalid risk: {risk}"
        );
    }
}

#[test]
fn caches_dry_run_with_selection_does_not_error() {
    // Pipe "1\ny" to select cache #1 and confirm in dry-run mode.
    // Dry-run skips the confirm prompt, so just "1" is needed.
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let mut child = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .arg("--dry-run")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn cargo-deepclean");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"1\n").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait");
    assert!(
        output.status.success(),
        "caches dry-run with selection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("DRY RUN MODE"),
        "dry-run mode should be indicated in output"
    );
}

#[test]
fn caches_quit_cancels_cleanly() {
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let mut child = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn cargo-deepclean");

    {
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(b"q\n").unwrap();
    }

    let output = child.wait_with_output().expect("failed to wait");
    assert!(
        output.status.success(),
        "quit should exit cleanly with status 0"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cancelled"), "quit should print Cancelled");
}

#[test]
fn caches_all_dry_run_is_non_interactive() {
    // The whole point of --all: no prompt. stdin is closed (EOF) — an
    // interactive prompt would either hang or read EOF and cancel; a
    // non-interactive run must complete successfully on its own.
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .arg("--all")
        .arg("--dry-run")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        output.status.success(),
        "--caches --all --dry-run should succeed without stdin: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Select caches to clean"),
        "--all must not prompt for selection"
    );
    assert!(
        !stdout.contains("Proceed?"),
        "--all must not prompt for confirmation"
    );
    if stdout.contains("safe cache") {
        assert!(
            stdout.contains("DRY RUN MODE"),
            "dry-run must be indicated when safe caches were found"
        );
        assert!(
            stdout.contains("Heavy caches skipped"),
            "--all must never include heavy caches"
        );
    }
}

#[test]
fn caches_all_json_reports_clean_results() {
    // --all + --json contract: a JSON array of clean results (not the
    // discovered-cache listing), each with the CacheCleanResult fields.
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .arg("--all")
        .arg("--json")
        .arg("--dry-run")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        output.status.success(),
        "--caches --all --json --dry-run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("--all --json should output valid JSON");
    let arr = parsed.as_array().expect("should be a JSON array");
    for (i, r) in arr.iter().enumerate() {
        assert!(r["id"].is_string(), "result {i} missing id");
        assert!(r["name"].is_string(), "result {i} missing name");
        assert!(r["success"].is_boolean(), "result {i} missing success");
        assert!(r["freed_bytes"].is_u64(), "result {i} missing freed_bytes");
        // Heavy caches must never appear in --all results
        assert_ne!(r["id"].as_str(), Some("huggingface"));
        assert_ne!(r["id"].as_str(), Some("playwright"));
    }
}

#[test]
fn caches_all_requires_caches_flag() {
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg("--all")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        !output.status.success(),
        "--all without --caches must be rejected"
    );
}

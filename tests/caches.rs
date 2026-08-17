use std::process::Command;

#[test]
fn caches_json_outputs_valid_array() {
    let bin = env!("CARGO_BIN_EXE_cargo-deepclean");
    let output = Command::new(bin)
        .arg("deepclean")
        .arg("--caches")
        .arg("--json")
        .output()
        .expect("failed to run cargo-deepclean");

    assert!(
        output.status.success(),
        "caches mode failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).expect("caches --json should output valid JSON");
    assert!(value.is_array(), "caches --json should output a JSON array");
}

use anyhow::{Context, Result};
use crate::project::Project;
use colored::Colorize;
use std::fs;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, Clone, serde::Serialize)]
pub struct UnusedDependency {
    pub name: String,
    pub location: String, // e.g., "[dependencies]", "[dev-dependencies]"
}

#[derive(Debug, serde::Serialize)]
pub struct DependencyCleanResult {
    pub path: String,
    pub success: bool,
    pub unused_deps: Vec<UnusedDependency>,
    pub removed_count: usize,
    pub error: Option<String>,
}

/// Extract dependency names from Cargo.toml
fn extract_dependencies(cargo_toml_path: &Path) -> Result<Vec<(String, String)>> {
    let content = fs::read_to_string(cargo_toml_path)
        .with_context(|| format!("Failed to read Cargo.toml: {:?}", cargo_toml_path))?;
    
    let toml: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse Cargo.toml: {:?}", cargo_toml_path))?;
    
    let mut deps = Vec::new();
    
    // Extract [dependencies]
    if let Some(deps_table) = toml.get("dependencies").and_then(|v| v.as_table()) {
        for (name, _) in deps_table {
            // Skip workspace dependencies and path dependencies for now
            // Only check crates.io dependencies
            deps.push((name.clone(), "[dependencies]".to_string()));
        }
    }
    
    // Extract [dev-dependencies]
    if let Some(dev_deps_table) = toml.get("dev-dependencies").and_then(|v| v.as_table()) {
        for (name, _) in dev_deps_table {
            deps.push((name.clone(), "[dev-dependencies]".to_string()));
        }
    }
    
    // Extract [build-dependencies]
    if let Some(build_deps_table) = toml.get("build-dependencies").and_then(|v| v.as_table()) {
        for (name, _) in build_deps_table {
            deps.push((name.clone(), "[build-dependencies]".to_string()));
        }
    }
    
    Ok(deps)
}

/// Normalize crate name for matching (handle dashes vs underscores)
fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

/// Build the search patterns for a single dependency name.
fn search_patterns_for(normalized_dep: &str) -> [String; 7] {
    [
        format!("use {}::", normalized_dep),
        format!("use {};", normalized_dep),
        format!("use crate::{}", normalized_dep),
        format!("{}::", normalized_dep),
        format!("extern crate {}", normalized_dep),
        format!("{}!", normalized_dep),
        format!("#[{}", normalized_dep),
    ]
}

/// Collect the text of every Rust source file relevant to usage detection,
/// reading each file exactly once.
fn collect_source_text(project_path: &Path) -> Vec<String> {
    let mut contents: Vec<String> = Vec::new();

    for sub in ["src", "examples", "tests"] {
        let dir = project_path.join(sub);
        if !dir.exists() {
            continue;
        }
        for entry in WalkDir::new(&dir).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file()
                && entry.path().extension().is_some_and(|e| e == "rs")
                && let Ok(content) = fs::read_to_string(entry.path())
            {
                contents.push(content);
            }
        }
    }

    // build.rs (build dependencies)
    let build_rs = project_path.join("build.rs");
    if let Ok(content) = fs::read_to_string(&build_rs) {
        contents.push(content);
    }

    contents
}

/// Check whether a dependency is referenced anywhere in the project sources or
/// manifest. All files are already in `sources`; the manifest is checked for
/// feature/alias references.
fn is_dependency_used(
    dep_name: &str,
    sources: &[String],
    cargo_toml_content: &str,
) -> bool {
    let normalized = normalize_crate_name(dep_name);
    let patterns = search_patterns_for(&normalized);

    for content in sources {
        for pattern in &patterns {
            if content.contains(pattern.as_str()) {
                return true;
            }
        }
    }

    // Cargo.toml feature flags / crate references (e.g. "dep/", "crate-name/").
    if cargo_toml_content.contains(&format!("{}/", dep_name))
        || cargo_toml_content.contains(&format!("{}-", dep_name))
        || cargo_toml_content.contains(&format!("{}/", normalized))
        || cargo_toml_content.contains(&format!("{}-", normalized))
    {
        return true;
    }

    false
}

/// Check for unused dependencies in a project
pub fn check_unused_dependencies(project: &Project) -> Result<Vec<UnusedDependency>> {
    let cargo_toml = project.path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Ok(vec![]);
    }

    let all_deps = extract_dependencies(&cargo_toml)?;

    // Common dependencies that may be used indirectly (macros, build scripts,
    // proc-macros). Skipped to avoid false positives. Allocated once.
    const SKIP_LIST: &[&str] = &[
        "proc-macro2",
        "quote",
        "syn",
        "serde",
        "serde_derive",
        "serde_json", // Often used in build scripts
    ];

    // Filter to the dependencies we actually need to check.
    let deps_to_check: Vec<(String, String)> = all_deps
        .into_iter()
        .filter(|(name, _)| {
            !SKIP_LIST.contains(&name.as_str())
                && !name.ends_with("_derive")
                && !name.contains("proc-macro")
        })
        .collect();

    if deps_to_check.is_empty() {
        return Ok(vec![]);
    }

    // Read every relevant source file exactly once.
    let sources = collect_source_text(&project.path);
    let cargo_content = fs::read_to_string(&cargo_toml).unwrap_or_default();

    let mut unused = Vec::new();
    for (dep_name, location) in deps_to_check {
        if !is_dependency_used(&dep_name, &sources, &cargo_content) {
            unused.push(UnusedDependency {
                name: dep_name,
                location,
            });
        }
    }

    Ok(unused)
}

/// Remove unused dependencies from Cargo.toml
pub fn remove_unused_dependencies(
    project: &Project,
    unused_deps: &[UnusedDependency],
    dry_run: bool,
    verbose: bool,
) -> Result<usize> {
    if dry_run || unused_deps.is_empty() {
        return Ok(0);
    }

    // Check if cargo-remove is available first
    let check_output = Command::new("cargo")
        .args(["remove", "--help"])
        .output();
    
    match check_output {
        Ok(output) if output.status.success() => {
            // cargo-remove is available
        }
        _ => {
            return Err(anyhow::anyhow!(
                "cargo-remove is not installed. Install it with: cargo install cargo-edit"
            ));
        }
    }

    // Use cargo-remove to remove dependencies
    let mut removed = 0;
    let mut errors = Vec::new();
    
    for dep in unused_deps {
        if verbose {
            println!("  {} Attempting to remove dependency: {} ({})", "[DEBUG]".cyan(), dep.name, dep.location);
        }
        
        // Determine which section the dependency is in
        let is_dev = dep.location.contains("dev-dependencies");
        let is_build = dep.location.contains("build-dependencies");
        
        // Build the cargo remove command with appropriate flags
        let mut cmd_args = vec!["remove".to_string(), dep.name.clone()];
        if is_dev {
            cmd_args.push("--dev".to_string());
        } else if is_build {
            cmd_args.push("--build".to_string());
        }
        
        let output = Command::new("cargo")
            .args(&cmd_args)
            .current_dir(&project.path)
            .output()
            .with_context(|| format!("Failed to run `cargo remove {}`", dep.name))?;

        if output.status.success() {
            removed += 1;
            if verbose {
                println!("  {} Successfully removed: {} ({})", "[DEBUG]".green(), dep.name, dep.location);
            }
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let error_msg = format!("Failed to remove {} ({}): {}", dep.name, dep.location, stderr);
            errors.push(error_msg.clone());
            if verbose {
                println!("  {} Failed to remove {} ({}): {}", "[DEBUG]".red(), dep.name, dep.location, stderr);
            }
        }
    }

    if !errors.is_empty() && removed == 0 {
        return Err(anyhow::anyhow!(
            "Failed to remove dependencies:\n{}",
            errors.join("\n")
        ));
    }

    Ok(removed)
}

/// Clean unused dependencies for a project
pub fn clean_dependencies(
    project: &Project,
    dry_run: bool,
    remove: bool,
    verbose: bool,
) -> Result<DependencyCleanResult> {
    let unused_deps = check_unused_dependencies(project)
        .with_context(|| format!("Failed to check unused dependencies in {:?}", project.path))?;

    let removed_count = if remove && !unused_deps.is_empty() {
        match remove_unused_dependencies(project, &unused_deps, dry_run, verbose) {
            Ok(count) => count,
            Err(e) => {
                // Return error in the result instead of failing completely
                return Ok(DependencyCleanResult {
                    path: project.path.to_string_lossy().to_string(),
                    success: false,
                    unused_deps,
                    removed_count: 0,
                    error: Some(e.to_string()),
                });
            }
        }
    } else {
        0
    };

    Ok(DependencyCleanResult {
        path: project.path.to_string_lossy().to_string(),
        success: true,
        unused_deps,
        removed_count,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_crate_name() {
        assert_eq!(normalize_crate_name("my-crate"), "my_crate");
        assert_eq!(normalize_crate_name("my_crate"), "my_crate");
        assert_eq!(normalize_crate_name("serde-json"), "serde_json");
    }

    #[test]
    fn test_extract_dependencies() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = "1.0"

[dev-dependencies]
tempfile = "3.0"
"#,
        ).unwrap();

        let deps = extract_dependencies(&cargo_toml).unwrap();
        assert!(deps.len() >= 2);
        let dep_names: Vec<String> = deps.iter().map(|(n, _)| n.clone()).collect();
        assert!(dep_names.contains(&"serde".to_string()));
        assert!(dep_names.contains(&"tokio".to_string()));
    }

    #[test]
    fn test_extract_dependencies_includes_build_deps() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"

[build-dependencies]
cc = "1.0"
"#,
        )
        .unwrap();

        let deps = extract_dependencies(&cargo_toml).unwrap();
        let names: Vec<String> = deps.iter().map(|(n, _)| n.clone()).collect();
        assert!(names.contains(&"cc".to_string()));
        let cc_loc = deps
            .iter()
            .find(|(n, _)| n == "cc")
            .map(|(_, l)| l.as_str())
            .unwrap();
        assert_eq!(cc_loc, "[build-dependencies]");
    }

    #[test]
    fn test_is_dependency_used_with_use_statement() {
        let sources = vec!["use serde::Deserialize;".to_string()];
        assert!(is_dependency_used("serde", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_unused() {
        let sources = vec!["fn main() {}".to_string()];
        assert!(!is_dependency_used("serde", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_dash_underscore_normalization() {
        // Crate name "serde-json" should match `use serde_json::`
        let sources = vec!["use serde_json::Value;".to_string()];
        assert!(is_dependency_used("serde-json", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_macro_invocation() {
        let sources = vec!["println!(\"hi\");".to_string()];
        assert!(is_dependency_used("println", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_attribute() {
        let sources = vec!["#[derive(Debug)]".to_string()];
        assert!(is_dependency_used("derive", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_cargo_feature_reference() {
        // Dependency referenced in Cargo.toml via "crate-name/feature" pattern
        let sources = vec!["fn main() {}".to_string()];
        let cargo = r#"[features]
feat1 = ["some-crate/feature"]
"#;
        assert!(is_dependency_used("some-crate", &sources, cargo));
    }

    #[test]
    fn test_check_unused_dependencies_finds_unused() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
rayon = "1.0"
"#,
        )
        .unwrap();
        // rayon is never used in source
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        let names: Vec<String> = unused.iter().map(|d| d.name.clone()).collect();
        assert!(
            names.contains(&"rayon".to_string()),
            "rayon should be detected as unused, got: {:?}",
            names
        );
    }

    #[test]
    fn test_check_unused_dependencies_no_false_positive_for_used() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
rayon = "1.0"
"#,
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("src/main.rs"),
            "use rayon::prelude::*;\nfn main() {}",
        )
        .unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        let names: Vec<String> = unused.iter().map(|d| d.name.clone()).collect();
        assert!(
            !names.contains(&"rayon".to_string()),
            "rayon is used and should not be flagged as unused"
        );
    }

    #[test]
    fn test_check_unused_dependencies_skips_skip_list() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
serde = "1.0"
serde_derive = "1.0"
"#,
        )
        .unwrap();
        // Neither serde nor serde_derive is used in source, but both are in the skip list
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        let names: Vec<String> = unused.iter().map(|d| d.name.clone()).collect();
        assert!(
            !names.contains(&"serde".to_string()),
            "serde is in skip list and should not be flagged"
        );
    }

    #[test]
    fn test_collect_source_text_includes_build_rs() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::create_dir_all(temp_dir.path().join("examples")).unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(
            temp_dir.path().join("examples/demo.rs"),
            "fn main() {}",
        )
        .unwrap();
        fs::write(temp_dir.path().join("build.rs"), "fn main() {}").unwrap();

        let texts = collect_source_text(temp_dir.path());
        // src/main.rs + examples/demo.rs + build.rs = 3 files
        assert_eq!(texts.len(), 3);
    }

    #[test]
    fn test_collect_source_text_empty_project() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let texts = collect_source_text(temp_dir.path());
        assert!(texts.is_empty());
    }

    #[test]
    fn test_extract_dependencies_renamed_dep_uses_manifest_key() {
        // `foo = { package = "bar" }` means the crate is imported as `foo` in
        // code, so the manifest key (not the package name) is the dep name.
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"
[dependencies]
foo = { package = "bar", version = "1.0" }
"#,
        )
        .unwrap();

        let deps = extract_dependencies(&cargo_toml).unwrap();
        let names: Vec<String> = deps.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(names, vec!["foo".to_string()]);
    }

    #[test]
    fn test_extract_dependencies_ignores_workspace_dependencies() {
        // `[workspace.dependencies]` entries live under the `workspace` key and
        // are intentionally not extracted: inheritance is not resolved, so
        // inherited deps can never be falsely flagged as unused.
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(
            &cargo_toml,
            r#"
[workspace.dependencies]
serde = "1.0"
"#,
        )
        .unwrap();

        let deps = extract_dependencies(&cargo_toml).unwrap();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_extract_dependencies_invalid_toml_is_err() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        fs::write(&cargo_toml, "not [ valid toml").unwrap();
        assert!(extract_dependencies(&cargo_toml).is_err());
    }

    #[test]
    fn test_extract_dependencies_missing_file_is_err() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        assert!(extract_dependencies(&temp_dir.path().join("Cargo.toml")).is_err());
    }

    #[test]
    fn test_check_unused_dependencies_missing_manifest_is_empty() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        assert!(unused.is_empty());
    }

    #[test]
    fn test_is_dependency_used_bare_use_statement() {
        let sources = vec!["use tempfile;".to_string()];
        assert!(is_dependency_used("tempfile", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_use_crate_path() {
        let sources = vec!["use crate::serde_json::Value;".to_string()];
        assert!(is_dependency_used("serde-json", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_extern_crate() {
        let sources = vec!["extern crate libc;".to_string()];
        assert!(is_dependency_used("libc", &sources, ""));
    }

    #[test]
    fn test_is_dependency_used_manifest_normalized_variants() {
        // Dependency "serde-json" referenced in the manifest with its
        // underscore-normalized name (feature refs, aliases, version pins).
        let sources = vec!["fn main() {}".to_string()];
        assert!(is_dependency_used(
            "serde-json",
            &sources,
            "[features]\nbig = [\"serde_json/fancy\"]\n"
        ));
        assert!(is_dependency_used(
            "serde-json",
            &sources,
            "[package]\nname = \"serde_json-something\"\n"
        ));
    }

    #[test]
    fn test_is_dependency_used_plain_name_no_false_positive() {
        // A bare mention without ::, !, #[, /, or - separators must NOT count.
        let sources = vec!["let serdejson = 1;".to_string()];
        assert!(!is_dependency_used("serde-json", &sources, ""));
    }

    #[test]
    fn test_check_unused_dependencies_dev_and_build_locations() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dev-dependencies]
unused-dev = "1.0"

[build-dependencies]
cc = "1.0"
"#,
        )
        .unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        // cc is used by build.rs, so it must not be flagged.
        fs::write(
            temp_dir.path().join("build.rs"),
            "fn main() { let _ = cc::compiler(); }",
        )
        .unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        let names: Vec<String> = unused.iter().map(|d| d.name.clone()).collect();
        assert!(names.contains(&"unused-dev".to_string()));
        assert!(!names.contains(&"cc".to_string()), "cc is used in build.rs");
        let dev = unused.iter().find(|d| d.name == "unused-dev").unwrap();
        assert_eq!(dev.location, "[dev-dependencies]");
    }

    #[test]
    fn test_check_unused_dependencies_skips_derive_and_proc_macro_names() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
some_derive = "1.0"
my-proc-macro-helper = "1.0"
thiserror = "1.0"
"#,
        )
        .unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = check_unused_dependencies(&project).unwrap();
        let names: Vec<String> = unused.iter().map(|d| d.name.clone()).collect();
        assert!(
            !names.contains(&"some_derive".to_string()),
            "*_derive names are skipped (likely used via re-exported macros)"
        );
        assert!(
            !names.contains(&"my-proc-macro-helper".to_string()),
            "names containing proc-macro are skipped"
        );
        assert!(
            names.contains(&"thiserror".to_string()),
            "normal deps outside the skip filters are still checked"
        );
    }

    #[test]
    fn test_collect_source_text_includes_tests_dir() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("tests")).unwrap();
        fs::write(
            temp_dir.path().join("tests/integration.rs"),
            "use my_crate::thing;",
        )
        .unwrap();
        let texts = collect_source_text(temp_dir.path());
        assert_eq!(texts.len(), 1);
        assert!(texts[0].contains("my_crate"));
    }

    #[test]
    fn test_collect_source_text_skips_invalid_utf8() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(temp_dir.path().join("src/binary.bin.rs"), [0xFF, 0xFE, 0x00]).unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        let texts = collect_source_text(temp_dir.path());
        assert_eq!(texts.len(), 1, "invalid UTF-8 file skipped without error");
    }

    #[test]
    fn test_remove_unused_dependencies_dry_run_is_noop() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let unused = vec![UnusedDependency {
            name: "whatever".to_string(),
            location: "[dependencies]".to_string(),
        }];
        let removed = remove_unused_dependencies(&project, &unused, true, false).unwrap();
        assert_eq!(removed, 0, "dry run removes nothing");
    }

    #[test]
    fn test_remove_unused_dependencies_empty_list_is_noop() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let removed = remove_unused_dependencies(&project, &[], false, false).unwrap();
        assert_eq!(removed, 0, "nothing to remove");
    }

    #[test]
    fn test_clean_dependencies_dry_run_reports_without_removing() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(temp_dir.path().join("src")).unwrap();
        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"

[dependencies]
never-used = "1.0"
"#,
        )
        .unwrap();
        fs::write(temp_dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let project = crate::project::Project {
            path: temp_dir.path().to_path_buf(),
            is_workspace: false,
        };
        let result = clean_dependencies(&project, true, true, false).unwrap();
        assert!(result.success);
        assert_eq!(result.removed_count, 0, "dry run never removes");
        assert_eq!(result.unused_deps.len(), 1);
        assert_eq!(result.unused_deps[0].name, "never-used");
        assert!(result.error.is_none());
    }
}

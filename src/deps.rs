use anyhow::{Context, Result};
use crate::project::Project;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
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

/// Check if a dependency is used in the source code
fn is_dependency_used(dep_name: &str, project_path: &Path) -> bool {
    let normalized_dep = normalize_crate_name(dep_name);
    let search_patterns = vec![
        // Direct use statements
        format!("use {}::", normalized_dep),
        format!("use {};", normalized_dep),
        format!("use crate::{}", normalized_dep),
        // In paths
        format!("{}::", normalized_dep),
        // Extern crate (older style)
        format!("extern crate {}", normalized_dep),
        // Macro invocations
        format!("{}!", normalized_dep),
        // Attribute macros
        format!("#[{}", normalized_dep),
    ];
    
    // Search in src/ directory
    let src_dir = project_path.join("src");
    if src_dir.exists() {
        if search_in_directory(&src_dir, &search_patterns) {
            return true;
        }
    }
    
    // Search in examples/ directory
    let examples_dir = project_path.join("examples");
    if examples_dir.exists() {
        if search_in_directory(&examples_dir, &search_patterns) {
            return true;
        }
    }
    
    // Search in tests/ directory
    let tests_dir = project_path.join("tests");
    if tests_dir.exists() {
        if search_in_directory(&tests_dir, &search_patterns) {
            return true;
        }
    }
    
    // Check build.rs
    let build_rs = project_path.join("build.rs");
    if build_rs.exists() {
        if let Ok(content) = fs::read_to_string(&build_rs) {
            for pattern in &search_patterns {
                if content.contains(pattern) {
                    return true;
                }
            }
        }
    }
    
    // Check Cargo.toml for feature flags or other references
    let cargo_toml = project_path.join("Cargo.toml");
    if let Ok(content) = fs::read_to_string(&cargo_toml) {
        // Check if it's used in feature definitions or other places
        // This is a simple check - might need refinement
        let normalized = normalize_crate_name(dep_name);
        if content.contains(&format!("{}/", dep_name)) 
            || content.contains(&format!("{}-", dep_name))
            || content.contains(&format!("{}/", normalized))
            || content.contains(&format!("{}-", normalized)) {
            return true;
        }
    }
    
    // Check for proc-macro usage (they're used via attributes, not imports)
    // This is a heuristic - proc-macros are tricky
    if dep_name.contains("proc-macro") || dep_name.contains("derive") {
        // These are likely used even if not directly imported
        // Be conservative and assume they're used
        return true;
    }
    
    false
}

/// Search for patterns in a directory
fn search_in_directory(dir: &Path, patterns: &[String]) -> bool {
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let path = entry.path();
            // Only check Rust files
            if let Some(ext) = path.extension() {
                if ext == "rs" {
                    if let Ok(content) = fs::read_to_string(path) {
                        for pattern in patterns {
                            if content.contains(pattern) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
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
    let mut unused = Vec::new();
    
    for (dep_name, location) in all_deps {
        // Skip some common dependencies that might be used indirectly
        // These are often used in macros, build scripts, or procedural macros
        let skip_list = vec![
            "proc-macro2",
            "quote",
            "syn",
            "serde",
            "serde_derive",
            "serde_json", // Often used in build scripts
        ];
        
        // Also skip if it's a proc-macro crate (they're used via attributes)
        if skip_list.contains(&dep_name.as_str()) 
            || dep_name.ends_with("_derive")
            || dep_name.contains("proc-macro") {
            continue;
        }
        
        if !is_dependency_used(&dep_name, &project.path) {
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
) -> Result<usize> {
    if dry_run || unused_deps.is_empty() {
        return Ok(0);
    }

    // Use cargo-remove if available (from cargo-edit)
    let mut removed = 0;
    for dep in unused_deps {
        let output = Command::new("cargo")
            .args(&["remove", &dep.name])
            .current_dir(&project.path)
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                removed += 1;
            }
        } else {
            // Fallback: manually edit Cargo.toml
            // This is more complex and error-prone, so we skip it for now
            // TODO: Implement manual Cargo.toml editing
        }
    }

    Ok(removed)
}

/// Clean unused dependencies for a project
pub fn clean_dependencies(
    project: &Project,
    dry_run: bool,
    remove: bool,
    _verbose: bool,
) -> Result<DependencyCleanResult> {
    let unused_deps = check_unused_dependencies(project)
        .with_context(|| format!("Failed to check unused dependencies in {:?}", project.path))?;

    let removed_count = if remove && !unused_deps.is_empty() {
        remove_unused_dependencies(project, &unused_deps, dry_run)
            .unwrap_or(0)
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
}

use anyhow::{Context, Result};
use crate::project::Project;
use std::process::Command;

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

/// Check for unused dependencies in a project
pub fn check_unused_dependencies(project: &Project) -> Result<Vec<UnusedDependency>> {
    // Try cargo-udeps first (more accurate)
    let udeps_output = Command::new("cargo")
        .args(&["udeps", "--output", "json"])
        .current_dir(&project.path)
        .output();

    if let Ok(output) = &udeps_output {
        // cargo-udeps may exit with non-zero even when it finds unused deps
        // Check if we got any output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        if !stdout.is_empty() || !stderr.is_empty() {
            // Try parsing the output
            let parsed = parse_udeps_output(&stdout);
            if !parsed.is_empty() {
                return Ok(parsed);
            }
            // Also check stderr for cargo-udeps output
            let parsed_stderr = parse_udeps_output(&stderr);
            if !parsed_stderr.is_empty() {
                return Ok(parsed_stderr);
            }
        }
    }

    // Fallback to cargo-machete
    let machete_output = Command::new("cargo")
        .arg("machete")
        .current_dir(&project.path)
        .output();

    if let Ok(output) = &machete_output {
        // cargo-machete exits with code 1 if unused deps found, 0 if none found
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        
        // Check both stdout and stderr
        let parsed_stdout = parse_machete_output(&stdout)?;
        if !parsed_stdout.is_empty() {
            return Ok(parsed_stdout);
        }
        let parsed_stderr = parse_machete_output(&stderr)?;
        if !parsed_stderr.is_empty() {
            return Ok(parsed_stderr);
        }
    }

    // If neither tool is available or found nothing, return empty
    Ok(vec![])
}

/// Parse cargo-udeps JSON output
fn parse_udeps_output(output: &str) -> Vec<UnusedDependency> {
    // cargo-udeps JSON format is complex, for now return empty
    // TODO: Implement proper JSON parsing
    // The JSON structure is: {"unused_deps": [{"name": "...", "location": "..."}]}
    let mut unused = Vec::new();
    
    // Try to parse as JSON
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(output) {
        if let Some(unused_deps) = json.get("unused_deps").and_then(|v| v.as_array()) {
            for dep in unused_deps {
                if let (Some(name), Some(location)) = (
                    dep.get("name").and_then(|v| v.as_str()),
                    dep.get("location").and_then(|v| v.as_str()),
                ) {
                    unused.push(UnusedDependency {
                        name: name.to_string(),
                        location: location.to_string(),
                    });
                }
            }
        }
    }
    
    unused
}

/// Parse cargo-machete output
fn parse_machete_output(output: &str) -> Result<Vec<UnusedDependency>> {
    let mut unused = Vec::new();
    
    for line in output.lines() {
        // cargo-machete output formats:
        // "unused dependency: `dependency_name`"
        // or just the dependency name in some cases
        let line = line.trim();
        
        if line.contains("unused dependency:") {
            if let Some(start) = line.find('`') {
                if let Some(end) = line[start + 1..].find('`') {
                    let dep_name = &line[start + 1..start + 1 + end];
                    unused.push(UnusedDependency {
                        name: dep_name.to_string(),
                        location: "[dependencies]".to_string(), // machete doesn't specify location
                    });
                }
            }
        } else if line.starts_with("`") && line.ends_with("`") && line.len() > 2 {
            // Sometimes cargo-machete just outputs the dependency name in backticks
            let dep_name = &line[1..line.len() - 1];
            if !dep_name.is_empty() && !dep_name.contains(' ') {
                unused.push(UnusedDependency {
                    name: dep_name.to_string(),
                    location: "[dependencies]".to_string(),
                });
            }
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

    let cargo_toml = project.path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(anyhow::anyhow!("Cargo.toml not found"));
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
    fn test_parse_machete_output() {
        let output = "unused dependency: `some-crate`\nunused dependency: `another-crate`\n";
        let deps = parse_machete_output(output).unwrap();
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "some-crate");
        assert_eq!(deps[1].name, "another-crate");
    }

    #[test]
    fn test_parse_machete_output_empty() {
        let output = "No unused dependencies found.\n";
        let deps = parse_machete_output(output).unwrap();
        assert_eq!(deps.len(), 0);
    }
}


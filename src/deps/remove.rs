//! Removal of unused dependencies via `cargo remove` (cargo-edit).
//!
//! Detection lives in [`super::detect`]; this module is the only place that
//! mutates manifests and the only place that shells out per dependency.

use anyhow::{Context, Result};
use crate::deps::UnusedDependency;
use crate::project::Project;
use colored::Colorize;
use std::process::Command;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}

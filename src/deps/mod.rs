//! Unused-dependency detection and removal.
//!
//! - [`detect`] — manifest parsing + source scanning (pure detection, no
//!   subprocesses, no mutations)
//! - [`remove`] — removal via `cargo remove` (cargo-edit) subprocesses
//! - This module wires the two into the per-project orchestration entry point
//!   [`clean_dependencies`] and holds the shared result types.

mod detect;
mod remove;

pub use detect::check_unused_dependencies;
pub use remove::remove_unused_dependencies;

use anyhow::{Context, Result};
use crate::project::Project;

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
    use std::fs;

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

use anyhow::{Context, Result};
use crate::project::Project;
use crate::utils::get_directory_size;
use std::process::Command;

#[derive(Debug, serde::Serialize)]
pub struct CleanResult {
    pub path: String,
    pub success: bool,
    pub freed_bytes: u64,
    pub error: Option<String>,
}

/// Clean a single Cargo project
pub fn clean_project(project: &Project, dry_run: bool, _verbose: bool) -> Result<CleanResult> {
    let target_dir = project.path.join("target");
    let freed_bytes = if target_dir.exists() {
        get_directory_size(&target_dir).unwrap_or(0)
    } else {
        0
    };

    if dry_run {
        return Ok(CleanResult {
            path: project.path.to_string_lossy().to_string(),
            success: true,
            freed_bytes,
            error: None,
        });
    }

    // Try cargo clean first
    let output = Command::new("cargo")
        .arg("clean")
        .current_dir(&project.path)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let after_size = if target_dir.exists() {
                get_directory_size(&target_dir).unwrap_or(0)
            } else {
                0
            };
            let actually_freed = freed_bytes.saturating_sub(after_size);

            Ok(CleanResult {
                path: project.path.to_string_lossy().to_string(),
                success: true,
                freed_bytes: actually_freed,
                error: None,
            })
        }
        _ => {
            // Fallback: remove target directory directly
            if target_dir.exists() {
                std::fs::remove_dir_all(&target_dir)
                    .with_context(|| format!("Failed to remove target directory: {:?}", target_dir))?;

                Ok(CleanResult {
                    path: project.path.to_string_lossy().to_string(),
                    success: true,
                    freed_bytes,
                    error: None,
                })
            } else {
                Ok(CleanResult {
                    path: project.path.to_string_lossy().to_string(),
                    success: true,
                    freed_bytes: 0,
                    error: None,
                })
            }
        }
    }
}


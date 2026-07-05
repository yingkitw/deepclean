use anyhow::Result;
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

/// Clean a single Cargo project.
///
/// The target directory size is computed exactly once. Both `cargo clean` and a
/// direct `remove_dir_all` delete the directory, so we report the pre-clean size
/// as freed without re-walking afterward.
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

    // Try `cargo clean` first.
    let output = Command::new("cargo")
        .arg("clean")
        .current_dir(&project.path)
        .output();

    let success = match output {
        Ok(o) if o.status.success() => true,
        _ => {
            // Fallback: remove the target directory directly.
            if target_dir.exists() {
                match std::fs::remove_dir_all(&target_dir) {
                    Ok(()) => true,
                    Err(e) => {
                        return Ok(CleanResult {
                            path: project.path.to_string_lossy().to_string(),
                            success: false,
                            freed_bytes: 0,
                            error: Some(format!(
                                "Failed to remove target directory {:?}: {}",
                                target_dir, e
                            )),
                        });
                    }
                }
            } else {
                true
            }
        }
    };

    Ok(CleanResult {
        path: project.path.to_string_lossy().to_string(),
        success,
        freed_bytes,
        error: None,
    })
}

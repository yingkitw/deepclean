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
                                "Failed to remove target directory {:?}: {}. \
Try running `cargo clean` manually in this project, or check file permissions.",
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

#[cfg(test)]
mod tests {
    use super::*;
    use rayon::prelude::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn write_project_with_target(base: &Path, sub: &str) {
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

    #[test]
    fn test_parallel_clean_multiple_subfolder_projects() {
        let temp = TempDir::new().unwrap();
        let subs = ["services/api", "apps/web", "tools/cli"];
        for sub in subs {
            write_project_with_target(temp.path(), sub);
        }

        let projects: Vec<Project> = subs
            .iter()
            .map(|sub| Project {
                path: temp.path().join(sub),
                is_workspace: false,
            })
            .collect();

        let results: Vec<CleanResult> = projects
            .par_iter()
            .map(|project| {
                clean_project(project, false, false).expect("clean should not error")
            })
            .collect();

        assert_eq!(results.len(), 3);
        assert!(
            results.iter().all(|r| r.success),
            "every subfolder project should clean successfully"
        );
        for sub in subs {
            assert!(
                !temp.path().join(sub).join("target").exists(),
                "target should be removed for {sub}"
            );
        }
    }
}

use anyhow::Result;
use crate::project::Project;
use crate::utils::get_directory_size;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, serde::Serialize)]
pub struct CleanResult {
    pub path: String,
    pub success: bool,
    pub freed_bytes: u64,
    pub error: Option<String>,
    /// Unused-dependency findings, populated only when `--clean-deps` ran.
    /// Omitted from JSON otherwise so existing consumers see no change.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub unused_deps: Vec<crate::deps::UnusedDependency>,
}

/// Compute the `target/` size for every project in parallel, once, so that
/// later phases (min-size filtering, interactive confirmation, dry-run
/// reporting) reuse the results instead of re-walking the same directories.
/// Projects without a `target/` directory map to 0.
pub fn compute_target_sizes(projects: &[Project]) -> HashMap<PathBuf, u64> {
    projects
        .par_iter()
        .map(|project| {
            let target_dir = project.path.join("target");
            let size = if target_dir.exists() {
                get_directory_size(&target_dir).unwrap_or(0)
            } else {
                0
            };
            (target_dir, size)
        })
        .collect()
}

/// Clean a single Cargo project.
///
/// The target directory size is computed exactly once. Both `cargo clean` and a
/// direct `remove_dir_all` delete the directory, so we report the pre-clean size
/// as freed without re-walking afterward.
///
/// `cached_size` (from [`compute_target_sizes`]) is trusted **only in dry-run
/// mode**, where nothing mutates the disk so a previously computed size is
/// still exact. Real runs always re-measure right before deletion so freed-byte
/// accounting stays accurate even if a build ran in the meantime (e.g. during
/// an interactive confirmation pause).
pub fn clean_project(
    project: &Project,
    dry_run: bool,
    _verbose: bool,
    cached_size: Option<u64>,
) -> Result<CleanResult> {
    let target_dir = project.path.join("target");
    let freed_bytes = if dry_run {
        cached_size.unwrap_or_else(|| {
            if target_dir.exists() {
                get_directory_size(&target_dir).unwrap_or(0)
            } else {
                0
            }
        })
    } else if target_dir.exists() {
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
            unused_deps: Vec::new(),
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
                            unused_deps: Vec::new(),
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
        unused_deps: Vec::new(),
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
                clean_project(project, false, false, None).expect("clean should not error")
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

    #[test]
    fn test_clean_project_dry_run_preserves_target() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("proj");
        write_project_with_target(temp.path(), "proj");

        let project = Project {
            path: dir.clone(),
            is_workspace: false,
        };
        let result = clean_project(&project, true, false, None).unwrap();
        assert!(result.success);
        assert!(result.freed_bytes > 0, "dry run should report size that would be freed");
        assert!(
            dir.join("target").exists(),
            "dry run must not delete target"
        );
    }

    #[test]
    fn test_clean_project_no_target_already_clean() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("clean_proj");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"clean_proj\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        // No target/ directory created

        let project = Project {
            path: dir.clone(),
            is_workspace: false,
        };
        let result = clean_project(&project, false, false, None).unwrap();
        assert!(result.success);
        assert_eq!(result.freed_bytes, 0, "no target means 0 bytes freed");
    }

    #[test]
    fn test_clean_project_removes_target() {
        let temp = TempDir::new().unwrap();
        write_project_with_target(temp.path(), "single");
        let dir = temp.path().join("single");

        let project = Project {
            path: dir.clone(),
            is_workspace: false,
        };
        let result = clean_project(&project, false, false, None).unwrap();
        assert!(result.success);
        assert!(result.freed_bytes > 0);
        assert!(!dir.join("target").exists());
    }

    #[test]
    fn test_dry_run_uses_cached_size_without_rewalk() {
        let temp = TempDir::new().unwrap();
        write_project_with_target(temp.path(), "cached");
        let dir = temp.path().join("cached");
        let actual = get_directory_size(&dir.join("target")).unwrap();
        assert!(actual > 0);

        let project = Project {
            path: dir,
            is_workspace: false,
        };
        // A cached value that differs from the real size must be reported as-is:
        // proves the dry-run path trusts the cache instead of re-walking.
        let result = clean_project(&project, true, false, Some(actual + 12345)).unwrap();
        assert!(result.success);
        assert_eq!(result.freed_bytes, actual + 12345);
    }

    #[test]
    fn test_dry_run_without_cache_still_walks() {
        let temp = TempDir::new().unwrap();
        write_project_with_target(temp.path(), "nocache");
        let dir = temp.path().join("nocache");
        let actual = get_directory_size(&dir.join("target")).unwrap();

        let project = Project {
            path: dir,
            is_workspace: false,
        };
        let result = clean_project(&project, true, false, None).unwrap();
        assert_eq!(result.freed_bytes, actual);
    }

    #[test]
    fn test_real_run_ignores_cached_size() {
        let temp = TempDir::new().unwrap();
        write_project_with_target(temp.path(), "stale");
        let dir = temp.path().join("stale");
        let actual = get_directory_size(&dir.join("target")).unwrap();

        let project = Project {
            path: dir.clone(),
            is_workspace: false,
        };
        // Real runs re-measure right before deletion; a stale cache value must
        // never be reported as freed.
        let result = clean_project(&project, false, false, Some(actual + 999_999)).unwrap();
        assert!(result.success);
        assert_eq!(result.freed_bytes, actual);
        assert!(!dir.join("target").exists());
    }

    #[test]
    fn test_compute_target_sizes_maps_targets_and_zeros_missing() {
        let temp = TempDir::new().unwrap();
        write_project_with_target(temp.path(), "with_target");
        let bare = temp.path().join("bare");
        fs::create_dir_all(bare.join("src")).unwrap();
        fs::write(
            bare.join("Cargo.toml"),
            "[package]\nname = \"bare\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::write(bare.join("src/main.rs"), "fn main() {}").unwrap();

        let projects = vec![
            Project {
                path: temp.path().join("with_target"),
                is_workspace: false,
            },
            Project {
                path: bare.clone(),
                is_workspace: false,
            },
        ];
        let sizes = compute_target_sizes(&projects);
        assert_eq!(sizes.len(), 2);
        assert_eq!(
            sizes.get(&temp.path().join("with_target").join("target")),
            Some(&get_directory_size(&temp.path().join("with_target").join("target")).unwrap())
        );
        assert_eq!(sizes.get(&bare.join("target")), Some(&0));
    }
}

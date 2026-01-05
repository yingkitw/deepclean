use anyhow::Result;
use cargo_metadata::MetadataCommand;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Project {
    pub path: PathBuf,
    pub is_workspace: bool,
}

/// Find all Cargo projects in the given directory
pub fn find_cargo_projects(root: &Path, exclude_patterns: &[String]) -> Result<Vec<Project>> {
    let mut projects = Vec::new();
    let mut seen_workspaces = HashSet::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden directories and common exclusions
            let name = e.file_name().to_string_lossy();
            if name.starts_with('.') && name != "." && name != ".." {
                return false;
            }

            // Check exclude patterns
            for pattern in exclude_patterns {
                if glob::Pattern::new(pattern)
                    .ok()
                    .and_then(|p| {
                        e.path()
                            .strip_prefix(root)
                            .ok()
                            .and_then(|rel| Some(p.matches(&rel.to_string_lossy())))
                    })
                    .unwrap_or(false)
                {
                    return false;
                }
            }
            true
        })
    {
        let entry = entry?;
        if entry.file_name() == "Cargo.toml" {
            let project_dir = entry.path().parent().unwrap().to_path_buf();

            // Check if this is part of a workspace
            let mut is_workspace_member = false;
            let mut current = project_dir.parent();
            while let Some(parent) = current {
                let workspace_toml = parent.join("Cargo.toml");
                if workspace_toml.exists() {
                    // Try to parse as workspace
                    if let Ok(metadata) = MetadataCommand::new()
                        .manifest_path(&workspace_toml)
                        .exec()
                    {
                        if metadata.workspace_root == parent {
                            // This is a workspace member
                            let workspace_path: PathBuf = metadata.workspace_root.into();
                            if !seen_workspaces.contains(&workspace_path) {
                                seen_workspaces.insert(workspace_path.clone());
                                projects.push(Project {
                                    path: workspace_path,
                                    is_workspace: true,
                                });
                            }
                            is_workspace_member = true;
                            break;
                        }
                    }
                }
                current = parent.parent();
            }

            // If not a workspace member, add as standalone project
            if !is_workspace_member {
                projects.push(Project {
                    path: project_dir,
                    is_workspace: false,
                });
            }
        }
    }

    // Remove duplicates
    projects.sort_by_key(|p| p.path.clone());
    projects.dedup_by_key(|p| p.path.clone());

    Ok(projects)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_cargo_projects_empty() {
        let temp_dir = TempDir::new().unwrap();
        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_find_cargo_projects_standalone() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("my-project");
        fs::create_dir(&project_dir).unwrap();
        fs::write(project_dir.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, project_dir);
        assert!(!projects[0].is_workspace);
    }
}


use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct Project {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub is_workspace: bool,
}

/// Cheap detection of a `[workspace]` table in a Cargo.toml manifest.
///
/// Looks only at table headers (lines of the form `[workspace]` or `[workspace.*]`),
/// skipping comments. This avoids spawning `cargo metadata` for every manifest and
/// every ancestor, which is the dominant cost in discovery.
fn manifest_declares_workspace(content: &str) -> bool {
    for raw in content.lines() {
        let line = raw.trim_start();
        if line.starts_with('#') {
            continue;
        }
        if line == "[workspace]" || line.starts_with("[workspace.") {
            return true;
        }
    }
    false
}

fn is_workspace_manifest(path: &Path) -> bool {
    match std::fs::read_to_string(path) {
        Ok(content) => manifest_declares_workspace(&content),
        Err(_) => false,
    }
}

/// Find all Cargo projects in the given directory.
///
/// A project's build output lives at its **workspace root** when it belongs to a
/// workspace, otherwise at the project directory itself. We resolve this purely by
/// scanning manifest files (no subprocesses), so discovery scales to large trees.
pub fn find_cargo_projects(root: &Path, exclude_patterns: &[String]) -> Result<Vec<Project>> {
    // Compile exclude patterns once instead of per directory entry.
    let compiled_excludes: Vec<glob::Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| glob::Pattern::new(p).ok())
        .collect();

    // Collect every Cargo.toml, pruning hidden dirs and excluded paths early.
    let mut manifests: Vec<PathBuf> = Vec::new();
    let mut it = WalkDir::new(root).into_iter();
    while let Some(res) = it.next() {
        let entry = match res {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.depth() > 0 && entry.file_type().is_dir() {
            let name = entry.file_name().to_string_lossy();

            // Prune hidden directories (but allow the root itself).
            if name.starts_with('.') {
                it.skip_current_dir();
                continue;
            }

            // Prune excluded directories.
            if !compiled_excludes.is_empty() {
                if let Ok(rel) = entry.path().strip_prefix(root) {
                    let rel_str = rel.to_string_lossy();
                    if compiled_excludes.iter().any(|p| p.matches(&rel_str)) {
                        it.skip_current_dir();
                        continue;
                    }
                }
            }
        }

        if entry.file_name() == "Cargo.toml" && entry.file_type().is_file() {
            if let Some(parent) = entry.path().parent() {
                manifests.push(parent.to_path_buf());
            }
        }
    }

    // Identify all workspace roots up front (one read per manifest, cached).
    let workspace_roots: HashSet<PathBuf> = manifests
        .iter()
        .filter(|dir| is_workspace_manifest(&dir.join("Cargo.toml")))
        .cloned()
        .collect();

    // For each manifest dir, resolve the nearest enclosing workspace root
    // (including itself). Members resolve to their root; standalone projects
    // resolve to themselves. Deduplicate on first encounter.
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut projects: Vec<Project> = Vec::new();

    for dir in &manifests {
        let target = nearest_workspace_root(dir, &workspace_roots);
        match target {
            Some(root_dir) => {
                if seen.insert(root_dir.clone()) {
                    projects.push(Project {
                        path: root_dir,
                        is_workspace: true,
                    });
                }
            }
            None => {
                if seen.insert(dir.clone()) {
                    projects.push(Project {
                        path: dir.clone(),
                        is_workspace: false,
                    });
                }
            }
        }
    }

    // Stable, deterministic ordering for output.
    projects.sort_unstable_by(|a, b| a.path.cmp(&b.path));

    Ok(projects)
}

/// Walk up from `dir` to find the nearest ancestor (including `dir`) that is a
/// workspace root. Pure path comparisons — no I/O.
fn nearest_workspace_root(dir: &Path, roots: &HashSet<PathBuf>) -> Option<PathBuf> {
    let mut current = Some(dir);
    while let Some(d) = current {
        if roots.contains(d) {
            return Some(d.to_path_buf());
        }
        current = d.parent();
    }
    None
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
        fs::write(
            project_dir.join("Cargo.toml"),
            "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        fs::create_dir(project_dir.join("src")).unwrap();
        fs::write(project_dir.join("src/main.rs"), "fn main() {}").unwrap();

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, project_dir);
        assert!(!projects[0].is_workspace);
    }

    #[test]
    fn test_find_cargo_projects_workspace_collapse() {
        // A workspace root plus a member should collapse into a single target.
        let temp_dir = TempDir::new().unwrap();
        let ws_root = temp_dir.path().join("ws");
        let member = ws_root.join("member");
        fs::create_dir_all(&member).unwrap();
        fs::write(
            ws_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .unwrap();
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 1, "workspace + member should collapse to one target");
        assert_eq!(projects[0].path, ws_root);
        assert!(projects[0].is_workspace);
    }

    #[test]
    fn test_find_multiple_subfolder_projects() {
        let temp_dir = TempDir::new().unwrap();
        for sub in ["services/api", "apps/web", "tools/cli"] {
            let dir = temp_dir.path().join(sub);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("Cargo.toml"),
                "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
        }

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 3);

        let paths: Vec<_> = projects.iter().map(|p| p.path.clone()).collect();
        assert!(paths.contains(&temp_dir.path().join("services/api")));
        assert!(paths.contains(&temp_dir.path().join("apps/web")));
        assert!(paths.contains(&temp_dir.path().join("tools/cli")));
    }

    #[test]
    fn test_find_cargo_projects_exclude_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let included = temp_dir.path().join("included");
        let excluded = temp_dir.path().join("vendor").join("nested");
        fs::create_dir_all(&excluded).unwrap();
        fs::create_dir(&included).unwrap();

        for dir in [&included, &excluded] {
            fs::write(
                dir.join("Cargo.toml"),
                "[package]\nname = \"test\"\nversion = \"0.1.0\"\n",
            )
            .unwrap();
        }

        let projects =
            find_cargo_projects(temp_dir.path(), &["**/vendor".to_string()]).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, included);
    }

    #[test]
    fn test_manifest_declares_workspace_ignores_comments() {
        // A `[workspace]` that only appears in a comment must not count.
        assert!(!manifest_declares_workspace("# [workspace]\n[dependencies]\nfoo = \"1\"\n"));
        assert!(manifest_declares_workspace("[workspace]\nmembers = []\n"));
        assert!(!manifest_declares_workspace("# this is [workspace]\n[package]\nname=\"x\"\n"));
        assert!(!manifest_declares_workspace("[workspace-dependencies]\n"));
    }

    #[test]
    fn test_hidden_directories_pruned() {
        // A Cargo.toml inside a hidden directory (e.g. .git) should not be found.
        let temp_dir = TempDir::new().unwrap();
        let hidden = temp_dir.path().join(".hidden");
        fs::create_dir_all(&hidden).unwrap();
        fs::write(
            hidden.join("Cargo.toml"),
            "[package]\nname = \"hidden\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        assert_eq!(projects.len(), 0, "hidden directories should be pruned");
    }

    #[test]
    fn test_nearest_workspace_root_direct() {
        let mut roots = HashSet::new();
        roots.insert(PathBuf::from("/projects/ws"));
        assert_eq!(
            nearest_workspace_root(Path::new("/projects/ws"), &roots),
            Some(PathBuf::from("/projects/ws"))
        );
    }

    #[test]
    fn test_nearest_workspace_root_ancestor() {
        let mut roots = HashSet::new();
        roots.insert(PathBuf::from("/projects/ws"));
        assert_eq!(
            nearest_workspace_root(Path::new("/projects/ws/member"), &roots),
            Some(PathBuf::from("/projects/ws"))
        );
    }

    #[test]
    fn test_nearest_workspace_root_none() {
        let roots = HashSet::new();
        assert_eq!(
            nearest_workspace_root(Path::new("/projects/standalone"), &roots),
            None
        );
    }

    #[test]
    fn test_find_cargo_projects_nested_workspace() {
        // A workspace root with a member that is itself a workspace: both are
        // independent workspace roots, so both are discovered as separate
        // projects (each has its own target/ dir).
        let temp_dir = TempDir::new().unwrap();
        let outer = temp_dir.path().join("outer");
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::write(
            outer.join("Cargo.toml"),
            "[workspace]\nmembers = [\"inner\"]\n",
        )
        .unwrap();
        fs::write(
            inner.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        // inner declares [workspace] so it is its own root, not collapsed into
        // outer. Both are found as separate workspace projects.
        assert_eq!(projects.len(), 2);
        let paths: Vec<_> = projects.iter().map(|p| p.path.clone()).collect();
        assert!(paths.contains(&outer));
        assert!(paths.contains(&inner));
    }

    #[test]
    fn test_find_cargo_projects_deterministic_order() {
        let temp_dir = TempDir::new().unwrap();
        for sub in ["zebra", "apple", "mango"] {
            let dir = temp_dir.path().join(sub);
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("Cargo.toml"),
                format!("[package]\nname = \"{sub}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
        }

        let projects = find_cargo_projects(temp_dir.path(), &[]).unwrap();
        let names: Vec<String> = projects
            .iter()
            .map(|p| p.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["apple", "mango", "zebra"]);
    }
}

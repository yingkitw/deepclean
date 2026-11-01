use anyhow::Result;
use rayon::prelude::*;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Resolve the user's home directory from explicit environment values.
///
/// Prefers `HOME` (Unix convention), falls back to `USERPROFILE` (Windows).
/// Empty values are treated as unset. Extracted as a pure function so it can
/// be tested without mutating process environment (racy under parallel tests).
pub fn resolve_home(home_env: Option<OsString>, userprofile_env: Option<OsString>) -> Option<PathBuf> {
    home_env
        .filter(|v| !v.is_empty())
        .or_else(|| userprofile_env.filter(|v| !v.is_empty()))
        .map(PathBuf::from)
}

/// Resolve the user's home directory from the process environment.
pub fn home_dir() -> Option<PathBuf> {
    resolve_home(
        std::env::var_os("HOME"),
        std::env::var_os("USERPROFILE"),
    )
}

/// Format bytes into human-readable string
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

/// Get the total size of a directory in bytes
pub fn get_directory_size(path: &Path) -> Result<u64> {
    let mut total = 0u64;
    if !path.exists() {
        return Ok(0);
    }

    for entry in WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            total += entry.metadata()?.len();
        }
    }
    Ok(total)
}

/// Depth (in directory levels below the root) up to which traversal runs
/// rayon-parallel; deeper levels walk sequentially per directory. Bounds rayon
/// task overhead so directories holding thousands of tiny files don't pay
/// scheduling costs dominated by per-task spawn.
const PARALLEL_WALK_DEPTH: usize = 2;

/// Parallel variant of [`get_directory_size`] for large trees (global caches,
/// big workspace `target/` dirs). Shallow levels are walked with rayon,
/// deeper levels sequentially. Returns 0 for missing/unreadable paths and
/// never follows symlinks — matching `get_directory_size`'s walkdir
/// semantics (no double-counting, no symlink loops).
pub fn get_directory_size_parallel(path: &Path) -> u64 {
    if !path.is_dir() {
        return 0;
    }
    dir_size(path, 0)
}

fn dir_size(path: &Path, depth: usize) -> u64 {
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut file_bytes = 0u64;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        // read_dir's file_type() does not follow symlinks, so symlinks fall
        // through both branches and contribute nothing — same as the
        // no-follow walkdir in get_directory_size.
        if let Ok(ft) = entry.file_type() {
            if ft.is_file() {
                file_bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            } else if ft.is_dir() {
                subdirs.push(entry.path());
            }
        }
    }
    let subdir_bytes: u64 = if depth + 1 < PARALLEL_WALK_DEPTH {
        subdirs.par_iter().map(|d| dir_size(d, depth + 1)).sum()
    } else {
        // Below the parallel threshold, delegate whole subtrees to the
        // sequential walkdir primitive — same semantics, no per-dir task churn.
        subdirs.iter().map(|d| get_directory_size(d).unwrap_or(0)).sum()
    };
    file_bytes + subdir_bytes
}

/// Parse size string (e.g., "100MB", "1GB") to bytes
pub fn parse_size(size_str: &str) -> Result<u64> {
    use anyhow::anyhow;
    let size_str = size_str.trim().to_uppercase();
    let (number_str, unit) = if size_str.ends_with("B") {
        if size_str.ends_with("KB") {
            (&size_str[..size_str.len() - 2], "KB")
        } else if size_str.ends_with("MB") {
            (&size_str[..size_str.len() - 2], "MB")
        } else if size_str.ends_with("GB") {
            (&size_str[..size_str.len() - 2], "GB")
        } else if size_str.ends_with("TB") {
            (&size_str[..size_str.len() - 2], "TB")
        } else {
            (&size_str[..size_str.len() - 1], "B")
        }
    } else {
        return Err(anyhow!("Invalid size format: expected format like '100MB' or '1GB'"));
    };

    let number: f64 = number_str
        .trim()
        .parse()
        .map_err(|_| anyhow!("Invalid number in size: {}", number_str))?;

    let multiplier = match unit {
        "B" => 1,
        "KB" => 1024,
        "MB" => 1024 * 1024,
        "GB" => 1024_u64 * 1024 * 1024,
        "TB" => 1024_u64 * 1024 * 1024 * 1024,
        _ => return Err(anyhow!("Unknown unit: {}", unit)),
    };

    Ok((number * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
        assert_eq!(format_bytes(1073741824), "1.00 GB");
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("100B").unwrap(), 100);
        assert_eq!(parse_size("1KB").unwrap(), 1024);
        assert_eq!(parse_size("1MB").unwrap(), 1048576);
        assert_eq!(parse_size("1GB").unwrap(), 1073741824);
        assert_eq!(parse_size("1.5MB").unwrap(), 1572864);
        assert!(parse_size("invalid").is_err());
    }

    #[test]
    fn test_get_directory_size_with_files() {
        let temp = tempfile::TempDir::new().unwrap();
        fs::write(temp.path().join("a.txt"), "hello").unwrap();
        fs::write(temp.path().join("b.txt"), "world!").unwrap();

        let size = get_directory_size(temp.path()).unwrap();
        assert_eq!(size, 11);
    }

    #[test]
    fn test_get_directory_size_nonexistent() {
        let size = get_directory_size(Path::new("/nonexistent/path"));
        assert!(size.is_ok());
        assert_eq!(size.unwrap(), 0);
    }

    #[test]
    fn test_get_directory_size_parallel_matches_sequential() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        // Nested tree: root/{a,b}/{x,y}/{deep.txt}, root/top.txt
        for a in ["a", "b"] {
            for x in ["x", "y"] {
                let dir = root.join(a).join(x);
                fs::create_dir_all(&dir).unwrap();
                fs::write(dir.join("deep.txt"), "deep").unwrap();
            }
        }
        fs::write(root.join("top.txt"), "top!").unwrap();
        // Empty dir contributes 0 but must not break traversal.
        fs::create_dir_all(root.join("a/empty")).unwrap();

        assert_eq!(
            get_directory_size_parallel(root),
            get_directory_size(root).unwrap()
        );
    }

    #[test]
    fn test_get_directory_size_parallel_skips_symlinks() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        fs::write(root.join("real.txt"), "12345").unwrap();
        let sub = root.join("sub");
        fs::create_dir_all(&sub).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("real.txt"), sub.join("link.txt")).unwrap();
            std::os::unix::fs::symlink(&sub, root.join("loop")).unwrap();
            // Symlinked file must not be counted (no double-count), and the
            // symlinked dir must not be traversed (no loop).
            assert_eq!(get_directory_size_parallel(root), 5);
        }
        #[cfg(not(unix))]
        assert_eq!(
            get_directory_size_parallel(root),
            get_directory_size(root).unwrap()
        );
    }

    #[test]
    fn test_get_directory_size_parallel_nonexistent_and_empty() {
        assert_eq!(get_directory_size_parallel(Path::new("/nonexistent/path")), 0);
        let temp = tempfile::TempDir::new().unwrap();
        assert_eq!(get_directory_size_parallel(temp.path()), 0);
    }

    #[test]
    fn test_resolve_home_prefers_home() {
        let home = resolve_home(
            Some("/home/unix".into()),
            Some("/profile/win".into()),
        );
        assert_eq!(home, Some(PathBuf::from("/home/unix")));
    }

    #[test]
    fn test_resolve_home_falls_back_to_userprofile() {
        let home = resolve_home(None, Some("/profile/win".into()));
        assert_eq!(home, Some(PathBuf::from("/profile/win")));
    }

    #[test]
    fn test_resolve_home_none_when_both_missing() {
        assert_eq!(resolve_home(None, None), None);
    }

    #[test]
    fn test_resolve_home_treats_empty_as_unset() {
        assert_eq!(resolve_home(Some("".into()), None), None);
        assert_eq!(resolve_home(Some("".into()), Some("/p".into())), Some(PathBuf::from("/p")));
    }

    #[test]
    fn test_home_dir_resolves_in_test_env() {
        // The test process always has HOME or USERPROFILE set on CI/dev machines.
        assert!(home_dir().is_some(), "expected HOME or USERPROFILE to be set");
    }
}


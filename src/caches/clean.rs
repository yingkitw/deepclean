//! Cache deletion: removes discovered [`CacheEntry`] paths and reports what
//! was freed. Every destructive path here respects dry-run and isolates
//! per-path failures (one failure never aborts the batch).

use super::registry::CacheEntry;
use rayon::prelude::*;
use std::path::Path;

#[derive(Debug, serde::Serialize)]
pub struct CacheCleanResult {
    pub id: String,
    pub name: String,
    pub success: bool,
    pub freed_bytes: u64,
    pub error: Option<String>,
}

fn remove_path(p: &Path) -> std::io::Result<()> {
    if p.is_dir() {
        std::fs::remove_dir_all(p)
    } else if p.is_file() {
        std::fs::remove_file(p)
    } else {
        Ok(())
    }
}

/// Clean the given caches. Reports the pre-computed size as freed bytes.
pub fn clean_caches(entries: &[CacheEntry], dry_run: bool) -> Vec<CacheCleanResult> {
    entries
        .par_iter()
        .map(|e| {
            let freed = e.size_bytes;
            if dry_run {
                return CacheCleanResult {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    success: true,
                    freed_bytes: freed,
                    error: None,
                };
            }
            let mut error = None;
            let mut success = true;
            for p in &e.paths {
                if p.exists()
                    && let Err(err) = remove_path(p)
                {
                    success = false;
                    error = Some(format!("{}: {}", p.display(), err));
                }
            }
            CacheCleanResult {
                id: e.id.clone(),
                name: e.name.clone(),
                success,
                freed_bytes: freed,
                error,
            }
        })
        .collect()
}

/// Filter a discovered cache list down to entries tagged `"safe"`.
///
/// Pure so the `--all` selection rule (heavy caches are never cleaned without
/// an explicit interactive pick) is unit-testable.
pub(crate) fn select_safe_caches(caches: &[CacheEntry]) -> Vec<CacheEntry> {
    caches
        .iter()
        .filter(|c| c.risk == "safe")
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::get_directory_size;
    use std::fs;
    use tempfile::TempDir;

    fn entry(id: &str, paths: Vec<std::path::PathBuf>, size_bytes: u64) -> CacheEntry {
        CacheEntry {
            id: id.into(),
            name: id.into(),
            category: "Test".into(),
            risk: "safe".into(),
            paths,
            size_bytes,
            note: None,
        }
    }

    #[test]
    fn test_clean_caches_removes_dirs() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("npm_cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a"), "hello world").unwrap();
        let size = get_directory_size(&dir).unwrap();

        let entry = entry("test", vec![dir.clone()], size);
        let results = clean_caches(&[entry], false);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert_eq!(results[0].freed_bytes, size);
        assert!(!dir.exists());
    }

    #[test]
    fn test_clean_caches_dry_run_keeps_dirs() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a"), "data").unwrap();
        let size = get_directory_size(&dir).unwrap();
        let entry = entry("test", vec![dir.clone()], size);
        let results = clean_caches(&[entry], true);
        assert!(results[0].success);
        assert!(dir.exists(), "dry run should not delete");
    }

    #[test]
    fn test_clean_caches_multiple_paths() {
        let temp = TempDir::new().unwrap();
        let dir1 = temp.path().join("cache1");
        let dir2 = temp.path().join("cache2");
        fs::create_dir_all(&dir1).unwrap();
        fs::create_dir_all(&dir2).unwrap();
        fs::write(dir1.join("a"), "aaa").unwrap();
        fs::write(dir2.join("b"), "bbbbbb").unwrap();
        let size = get_directory_size(&dir1).unwrap() + get_directory_size(&dir2).unwrap();

        let entry = entry("multi", vec![dir1.clone(), dir2.clone()], size);
        let results = clean_caches(&[entry], false);
        assert!(results[0].success);
        assert_eq!(results[0].freed_bytes, size);
        assert!(!dir1.exists());
        assert!(!dir2.exists());
    }

    #[test]
    fn test_clean_caches_nonexistent_paths_succeeds() {
        let entry = entry(
            "ghost",
            vec![std::path::PathBuf::from("/nonexistent/path/that/does/not/exist")],
            100,
        );
        let results = clean_caches(&[entry], false);
        assert!(results[0].success, "non-existent paths should not cause failure");
    }

    #[test]
    fn test_clean_caches_empty_entries() {
        let results = clean_caches(&[], false);
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_path_on_file() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("single_file");
        fs::write(&file, "data").unwrap();
        assert!(file.is_file());
        remove_path(&file).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn test_remove_path_nonexistent_is_ok() {
        let result = remove_path(Path::new("/nonexistent/path"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_select_safe_caches_filters_heavy() {
        let mk = |id: &str, risk: &str| CacheEntry {
            id: id.into(),
            name: id.into(),
            category: "T".into(),
            risk: risk.into(),
            paths: vec![],
            size_bytes: 10,
            note: None,
        };
        let caches = vec![mk("a", "safe"), mk("b", "heavy"), mk("c", "safe")];
        let safe = select_safe_caches(&caches);
        let ids: Vec<_> = safe.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c"]);
        assert!(select_safe_caches(&[]).is_empty());
        let heavy_only = vec![mk("h", "heavy")];
        assert!(select_safe_caches(&heavy_only).is_empty());
    }
}

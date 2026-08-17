use crate::utils::{format_bytes, get_directory_size};
use anyhow::Result;
use colored::Colorize;
use rayon::prelude::*;
use serde::Serialize;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// A discoverable global toolchain cache that can be cleaned.
#[derive(Debug, Clone, Serialize)]
pub struct CacheEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    /// "safe" = pure download cache (re-fetches on demand);
    /// "heavy" = re-download required (model weights, browser binaries, runtimes).
    pub risk: String,
    #[serde(skip)]
    paths: Vec<PathBuf>,
    pub size_bytes: u64,
    pub note: Option<String>,
}

impl CacheEntry {
    fn new(id: &str, name: &str, category: &str, risk: &str, paths: Vec<PathBuf>) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            category: category.to_string(),
            risk: risk.to_string(),
            paths,
            size_bytes: 0,
            note: None,
        }
    }

    fn with_note(mut self, note: &str) -> Self {
        self.note = Some(note.to_string());
        self
    }
}

#[derive(Debug, Serialize)]
pub struct CacheCleanResult {
    pub id: String,
    pub name: String,
    pub success: bool,
    pub freed_bytes: u64,
    pub error: Option<String>,
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Build the full registry of known caches. Paths that do not exist are filtered
/// out later by [`discover_caches`].
pub fn build_registry() -> Vec<CacheEntry> {
    let home = match home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let xdg = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache"));
    let mac = home.join("Library/Caches");

    vec![
        // Rust
        CacheEntry::new(
            "cargo-cache",
            "Cargo registry cache",
            "Rust",
            "safe",
            vec![home.join(".cargo/registry/cache")],
        ),
        CacheEntry::new(
            "cargo-src",
            "Cargo registry src",
            "Rust",
            "heavy",
            vec![home.join(".cargo/registry/src")],
        )
        .with_note("Unpacked crate sources; re-extracts on next build"),
        // Python
        CacheEntry::new(
            "pip",
            "pip",
            "Python",
            "safe",
            vec![mac.join("pip"), xdg.join("pip")],
        ),
        CacheEntry::new(
            "uv",
            "uv",
            "Python",
            "safe",
            vec![xdg.join("uv"), mac.join("uv")],
        ),
        CacheEntry::new(
            "poetry",
            "Poetry",
            "Python",
            "safe",
            vec![mac.join("pypoetry"), xdg.join("pypoetry")],
        ),
        // JS/TS
        CacheEntry::new(
            "npm",
            "npm",
            "JS/TS",
            "safe",
            vec![home.join(".npm/_cacache"), home.join(".npm/_npx")],
        ),
        CacheEntry::new(
            "bun",
            "Bun install cache",
            "JS/TS",
            "safe",
            vec![home.join(".bun/install/cache"), mac.join("bun")],
        ),
        CacheEntry::new(
            "pnpm",
            "pnpm store",
            "JS/TS",
            "safe",
            vec![
                home.join(".local/share/pnpm"),
                home.join("Library/pnpm"),
            ],
        ),
        CacheEntry::new(
            "yarn",
            "Yarn cache",
            "JS/TS",
            "safe",
            vec![mac.join("Yarn"), home.join(".yarn")],
        ),
        // Other
        CacheEntry::new(
            "homebrew",
            "Homebrew",
            "Other",
            "safe",
            vec![mac.join("Homebrew")],
        ),
        CacheEntry::new(
            "puppeteer",
            "Puppeteer (Chrome)",
            "Other",
            "heavy",
            vec![xdg.join("puppeteer")],
        )
        .with_note("Browser binaries; re-downloads on next run"),
        CacheEntry::new(
            "playwright",
            "Playwright browsers",
            "Other",
            "heavy",
            vec![mac.join("ms-playwright"), xdg.join("ms-playwright")],
        )
        .with_note("Browser binaries; re-downloads on next run"),
        CacheEntry::new(
            "huggingface",
            "HuggingFace",
            "Other",
            "heavy",
            vec![xdg.join("huggingface")],
        )
        .with_note("Model weights; re-downloads on demand"),
        CacheEntry::new(
            "torch",
            "PyTorch models",
            "Other",
            "heavy",
            vec![xdg.join("torch")],
        )
        .with_note("Model weights; re-downloads on demand"),
        CacheEntry::new(
            "go-build",
            "Go build cache",
            "Other",
            "safe",
            vec![mac.join("go-build"), xdg.join("go-build")],
        ),
        CacheEntry::new(
            "codex-runtimes",
            "Codex runtimes",
            "Other",
            "heavy",
            vec![xdg.join("codex-runtimes")],
        )
        .with_note("Agent runtimes; re-fetches on next session"),
    ]
}

/// Resolve the registry to caches that currently exist on disk, with sizes
/// computed in parallel. Entries with no existing paths are dropped.
pub fn discover_caches() -> Vec<CacheEntry> {
    let mut entries = build_registry();
    entries.par_iter_mut().for_each(|e| {
        e.size_bytes = e
            .paths
            .iter()
            .filter(|p| p.exists())
            .map(|p| get_directory_size(p).unwrap_or(0))
            .sum();
    });
    entries.into_iter().filter(|e| e.size_bytes > 0).collect()
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

/// Parse a 1-based, comma/space separated selection into 0-based indices,
/// deduplicated and bounded by `count`. Out-of-range and non-numeric tokens are
/// silently ignored.
pub fn parse_selection(input: &str, count: usize) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for token in input.split(|c: char| c == ',' || c.is_whitespace()) {
        if let Ok(n) = token.trim().parse::<usize>()
            && n >= 1
            && n <= count
            && seen.insert(n - 1)
        {
            out.push(n - 1);
        }
    }
    out
}

fn print_cache_list(caches: &[CacheEntry]) {
    println!("{} Available caches to clean:", "[INFO]".blue().bold());
    println!();
    println!(
        "  {:>3}  {:<26} {:<10} {:<6} {:>10}",
        "#", "Cache", "Category", "Risk", "Size"
    );
    for (i, c) in caches.iter().enumerate() {
        println!(
            "  {:>3}  {:<26} {:<10} {:<6} {:>10}",
            i + 1,
            c.name,
            c.category,
            c.risk,
            format_bytes(c.size_bytes)
        );
        if let Some(ref note) = c.note {
            println!("        {}", note.dimmed());
        }
    }
}

fn print_cache_results(results: &[CacheCleanResult]) {
    println!();
    println!("{} === CACHE CLEAN SUMMARY ===", "[INFO]".blue().bold());
    let total_freed: u64 = results.iter().map(|r| r.freed_bytes).sum();
    let ok = results.iter().filter(|r| r.success).count();
    let failed = results.len() - ok;
    for r in results {
        if r.success {
            println!(
                "  {} {} (freed: {})",
                "✓".green(),
                r.name,
                format_bytes(r.freed_bytes)
            );
        } else {
            println!(
                "  {} {} - {}",
                "✗".red(),
                r.name,
                r.error.as_deref().unwrap_or("failed")
            );
        }
    }
    println!();
    if failed == 0 {
        println!(
            "{} Cleaned {} cache(s), freed {}",
            "[SUCCESS]".green().bold(),
            ok,
            format_bytes(total_freed)
        );
    } else {
        println!(
            "{} Cleaned {}, failed {}, freed {}",
            "[INFO]".blue().bold(),
            ok,
            failed,
            format_bytes(total_freed)
        );
    }
}

fn prompt_selection(count: usize) -> Result<Vec<usize>> {
    print!("Select caches to clean (comma-separated numbers, 'all', or 'q' to quit): ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    let trimmed = line.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "q" | "quit" | "exit" | "n" | "" => Ok(Vec::new()),
        "all" | "a" | "y" | "yes" => Ok((0..count).collect()),
        other => Ok(parse_selection(other, count)),
    }
}

fn confirm() -> Result<bool> {
    print!("Proceed? [y/N]: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Entry point for `--caches` mode: discover caches, list them, let the user
/// select, then clean. In JSON mode the discovered caches are printed as JSON
/// and no prompt is shown (useful for automation/inspection).
pub fn run_cache_mode(dry_run: bool, json: bool) -> Result<()> {
    let caches = discover_caches();

    if caches.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("{} No caches found to clean", "[INFO]".blue().bold());
        }
        return Ok(());
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&caches)?);
        return Ok(());
    }

    print_cache_list(&caches);
    let total: u64 = caches.iter().map(|c| c.size_bytes).sum();
    println!();
    println!("  Total reclaimable: {}", format_bytes(total).bold());
    println!();

    let selection = prompt_selection(caches.len())?;
    if selection.is_empty() {
        println!("{} Cancelled", "[INFO]".blue().bold());
        return Ok(());
    }

    let selected: Vec<CacheEntry> = selection
        .iter()
        .filter_map(|&i| caches.get(i).cloned())
        .collect();
    let sel_total: u64 = selected.iter().map(|c| c.size_bytes).sum();

    println!();
    println!(
        "{} About to clean {} cache(s), freeing ~{}",
        "[INFO]".cyan().bold(),
        selected.len(),
        format_bytes(sel_total)
    );
    if dry_run {
        println!(
            "{} DRY RUN MODE - no changes will be made",
            "[INFO]".yellow().bold()
        );
    } else if !confirm()? {
        println!("{} Cancelled by user", "[INFO]".blue().bold());
        return Ok(());
    }

    let results = clean_caches(&selected, dry_run);
    print_cache_results(&results);

    if results.iter().any(|r| !r.success) {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_build_registry_has_known_caches() {
        let reg = build_registry();
        assert!(!reg.is_empty());
        let ids: Vec<_> = reg.iter().map(|e| e.id.as_str()).collect();
        assert!(ids.contains(&"npm"));
        assert!(ids.contains(&"cargo-cache"));
        assert!(ids.contains(&"huggingface"));
    }

    #[test]
    fn test_parse_selection_basic() {
        assert_eq!(parse_selection("1,3", 5), vec![0, 2]);
        assert_eq!(parse_selection("1 3", 5), vec![0, 2]);
    }

    #[test]
    fn test_parse_selection_out_of_range() {
        assert!(parse_selection("0", 5).is_empty());
        assert!(parse_selection("6", 5).is_empty());
    }

    #[test]
    fn test_parse_selection_dedup() {
        assert_eq!(parse_selection("1,1,2", 5), vec![0, 1]);
    }

    #[test]
    fn test_parse_selection_invalid_ignored() {
        assert_eq!(parse_selection("1,foo,3", 5), vec![0, 2]);
    }

    #[test]
    fn test_clean_caches_removes_dirs() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("npm_cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a"), "hello world").unwrap();
        let size = get_directory_size(&dir).unwrap();

        let entry = CacheEntry {
            id: "test".into(),
            name: "test".into(),
            category: "Test".into(),
            risk: "safe".into(),
            paths: vec![dir.clone()],
            size_bytes: size,
            note: None,
        };
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
        let entry = CacheEntry {
            id: "test".into(),
            name: "test".into(),
            category: "Test".into(),
            risk: "safe".into(),
            paths: vec![dir.clone()],
            size_bytes: size,
            note: None,
        };
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

        let entry = CacheEntry {
            id: "multi".into(),
            name: "multi".into(),
            category: "Test".into(),
            risk: "safe".into(),
            paths: vec![dir1.clone(), dir2.clone()],
            size_bytes: size,
            note: None,
        };
        let results = clean_caches(&[entry], false);
        assert!(results[0].success);
        assert_eq!(results[0].freed_bytes, size);
        assert!(!dir1.exists());
        assert!(!dir2.exists());
    }

    #[test]
    fn test_clean_caches_nonexistent_paths_succeeds() {
        let entry = CacheEntry {
            id: "ghost".into(),
            name: "ghost".into(),
            category: "Test".into(),
            risk: "safe".into(),
            paths: vec![PathBuf::from("/nonexistent/path/that/does/not/exist")],
            size_bytes: 100,
            note: None,
        };
        let results = clean_caches(&[entry], false);
        assert!(results[0].success, "non-existent paths should not cause failure");
    }

    #[test]
    fn test_clean_caches_empty_entries() {
        let results = clean_caches(&[], false);
        assert!(results.is_empty());
    }

    #[test]
    fn test_registry_risk_values_valid() {
        let reg = build_registry();
        for entry in &reg {
            assert!(
                entry.risk == "safe" || entry.risk == "heavy",
                "entry {} has invalid risk: {}",
                entry.id,
                entry.risk
            );
        }
    }

    #[test]
    fn test_registry_unique_ids() {
        let reg = build_registry();
        let mut ids: Vec<&str> = reg.iter().map(|e| e.id.as_str()).collect();
        ids.sort();
        let len_before = ids.len();
        ids.dedup();
        assert_eq!(
            ids.len(),
            len_before,
            "registry has duplicate cache IDs"
        );
    }

    #[test]
    fn test_registry_all_categories_nonempty() {
        let reg = build_registry();
        for entry in &reg {
            assert!(!entry.category.is_empty(), "entry {} has empty category", entry.id);
            assert!(!entry.name.is_empty(), "entry {} has empty name", entry.id);
        }
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
    fn test_parse_selection_all_tokens() {
        assert_eq!(parse_selection("1,2,3,4,5", 5), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_parse_selection_mixed_separators() {
        assert_eq!(parse_selection("1, 2 3", 5), vec![0, 1, 2]);
    }

    #[test]
    fn test_parse_selection_empty() {
        assert!(parse_selection("", 5).is_empty());
    }
}

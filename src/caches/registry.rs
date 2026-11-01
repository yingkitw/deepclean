//! The cache registry: WHAT can be cleaned. Pure data — every known global
//! toolchain cache with its id, category, risk tag, and cross-platform paths.
//! No I/O beyond env/home resolution; no deletion, no prompts.

use crate::utils::home_dir;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// A discoverable global toolchain cache that can be cleaned.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheEntry {
    pub id: String,
    pub name: String,
    pub category: String,
    /// "safe" = pure download cache (re-fetches on demand);
    /// "heavy" = re-download required (model weights, browser binaries, runtimes).
    pub risk: String,
    #[serde(skip)]
    pub(crate) paths: Vec<PathBuf>,
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

/// Build the full registry of known caches. Paths that do not exist are filtered
/// out later by [`super::discover_caches`].
pub fn build_registry() -> Vec<CacheEntry> {
    let home = match home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let xdg = env_base_dir(
        std::env::var_os("XDG_CACHE_HOME"),
        home.join(".cache"),
    );
    let mac = home.join("Library/Caches");
    let win_local = env_base_dir(
        std::env::var_os("LOCALAPPDATA"),
        home.join("AppData/Local"),
    );
    build_registry_for(&home, &xdg, &mac, &win_local)
}

/// Resolve a base directory from an env var, falling back when unset **or empty**.
///
/// A set-but-empty value must not swallow the fallback (it would yield a bogus
/// relative path `""`) — same lesson as `utils::resolve_home`. Pure so it is
/// testable without mutating process environment.
fn env_base_dir(env: Option<OsString>, fallback: PathBuf) -> PathBuf {
    env.filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or(fallback)
}

/// Build the registry from explicit base directories (pure, testable).
///
/// Base-dir conventions:
/// - `home`: `$HOME` / `%USERPROFILE%` — tool dirs like `.cargo`, `.npm`, `.bun`
/// - `xdg`: `$XDG_CACHE_HOME`, defaulting to `~/.cache` (also where Windows tools
///   like puppeteer/HuggingFace/torch live, since `%USERPROFILE%\.cache` is their default)
/// - `mac`: `~/Library/Caches` (macOS per-tool cache subdirs)
/// - `win_local`: `%LOCALAPPDATA%`, defaulting to `~/AppData/Local` (Windows per-tool caches)
fn build_registry_for(home: &Path, xdg: &Path, mac: &Path, win_local: &Path) -> Vec<CacheEntry> {
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
            vec![mac.join("pip"), xdg.join("pip"), win_local.join("pip")],
        ),
        CacheEntry::new(
            "uv",
            "uv",
            "Python",
            "safe",
            vec![
                xdg.join("uv"),
                mac.join("uv"),
                win_local.join("uv"),
            ],
        ),
        CacheEntry::new(
            "poetry",
            "Poetry",
            "Python",
            "safe",
            vec![
                mac.join("pypoetry"),
                xdg.join("pypoetry"),
                win_local.join("pypoetry"),
            ],
        ),
        // JS/TS
        CacheEntry::new(
            "npm",
            "npm",
            "JS/TS",
            "safe",
            vec![
                home.join(".npm/_cacache"),
                home.join(".npm/_npx"),
                win_local.join("npm-cache"),
            ],
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
                win_local.join("pnpm"),
            ],
        ),
        CacheEntry::new(
            "yarn",
            "Yarn cache",
            "JS/TS",
            "safe",
            vec![
                mac.join("Yarn"),
                home.join(".yarn"),
                win_local.join("Yarn"),
            ],
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
            vec![
                mac.join("ms-playwright"),
                xdg.join("ms-playwright"),
                win_local.join("ms-playwright"),
            ],
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
            vec![
                mac.join("go-build"),
                xdg.join("go-build"),
                win_local.join("go-build"),
            ],
        ),
        CacheEntry::new(
            "go-mod",
            "Go module cache",
            "Other",
            "safe",
            vec![home.join("go/pkg/mod")],
        )
        .with_note("Re-downloads modules on next build"),
        // JVM
        CacheEntry::new(
            "gradle",
            "Gradle",
            "Java",
            "safe",
            vec![home.join(".gradle/caches")],
        )
        .with_note("Build + dependency caches; regenerates on next build"),
        CacheEntry::new(
            "maven",
            "Maven",
            "Java",
            "safe",
            vec![home.join(".m2/repository")],
        )
        .with_note("Re-downloads artifacts on next build"),
        // Apple
        CacheEntry::new(
            "xcode-deriveddata",
            "Xcode DerivedData",
            "Apple",
            "safe",
            vec![home.join("Library/Developer/Xcode/DerivedData")],
        )
        .with_note("Build artifacts; recompiles on next build"),
        // JS/TS binaries
        CacheEntry::new(
            "cypress",
            "Cypress",
            "JS/TS",
            "heavy",
            vec![
                mac.join("Cypress"),
                xdg.join("Cypress"),
                win_local.join("Cypress/Cache"),
            ],
        )
        .with_note("Browser binaries; re-downloads on next run"),
        CacheEntry::new(
            "electron",
            "Electron",
            "JS/TS",
            "heavy",
            vec![
                mac.join("electron"),
                xdg.join("electron"),
                win_local.join("electron/Cache"),
            ],
        )
        .with_note("Electron binaries; re-downloads on next install"),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_env_base_dir_treats_empty_as_unset() {
        let fallback = PathBuf::from("/fallback");
        assert_eq!(env_base_dir(None, fallback.clone()), fallback);
        assert_eq!(env_base_dir(Some("".into()), fallback.clone()), fallback);
        assert_eq!(
            env_base_dir(Some("/custom".into()), fallback),
            PathBuf::from("/custom")
        );
    }

    #[test]
    fn test_registry_windows_localappdata_paths() {
        let home = PathBuf::from("/home/t");
        let win_local = home.join("AppData/Local");
        let reg = build_registry_for(
            &home,
            &home.join(".cache"),
            &home.join("Library/Caches"),
            &win_local,
        );
        let find = |id: &str| {
            reg.iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("missing entry {}", id))
        };
        assert!(find("npm").paths.contains(&win_local.join("npm-cache")));
        assert!(find("pip").paths.contains(&win_local.join("pip")));
        assert!(find("uv").paths.contains(&win_local.join("uv")));
        assert!(find("poetry").paths.contains(&win_local.join("pypoetry")));
        assert!(find("pnpm").paths.contains(&win_local.join("pnpm")));
        assert!(find("yarn").paths.contains(&win_local.join("Yarn")));
        assert!(find("playwright").paths.contains(&win_local.join("ms-playwright")));
        assert!(find("go-build").paths.contains(&win_local.join("go-build")));
    }

    #[test]
    fn test_registry_xdg_default_covers_windows_dotcache() {
        // On Windows XDG_CACHE_HOME is unset and xdg defaults to ~/.cache, which is
        // also where puppeteer/HuggingFace/torch/codex store data on Windows.
        let home = PathBuf::from("/home/t");
        let reg = build_registry_for(
            &home,
            &home.join(".cache"),
            &home.join("Library/Caches"),
            &home.join("AppData/Local"),
        );
        let find = |id: &str| {
            reg.iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("missing entry {}", id))
        };
        assert!(find("puppeteer").paths.contains(&home.join(".cache/puppeteer")));
        assert!(find("huggingface").paths.contains(&home.join(".cache/huggingface")));
        assert!(find("torch").paths.contains(&home.join(".cache/torch")));
    }

    #[test]
    fn test_registry_cargo_and_bun_are_home_based_on_all_platforms() {
        let home = PathBuf::from("/home/t");
        let reg = build_registry_for(
            &home,
            &home.join(".cache"),
            &home.join("Library/Caches"),
            &home.join("AppData/Local"),
        );
        let find = |id: &str| {
            reg.iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("missing entry {}", id))
        };
        assert!(find("cargo-cache").paths.contains(&home.join(".cargo/registry/cache")));
        assert!(find("bun").paths.contains(&home.join(".bun/install/cache")));
    }

    #[test]
    fn test_registry_tier1_paths() {
        let home = PathBuf::from("/home/t");
        let mac = home.join("Library/Caches");
        let xdg = home.join(".cache");
        let win_local = home.join("AppData/Local");
        let reg = build_registry_for(&home, &xdg, &mac, &win_local);
        let find = |id: &str| {
            reg.iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("missing entry {}", id))
        };
        // Xcode DerivedData lives under ~/Library/Developer, not ~/Library/Caches
        assert!(find("xcode-deriveddata")
            .paths
            .contains(&home.join("Library/Developer/Xcode/DerivedData")));
        assert!(find("gradle").paths.contains(&home.join(".gradle/caches")));
        assert!(find("maven").paths.contains(&home.join(".m2/repository")));
        assert!(find("go-mod").paths.contains(&home.join("go/pkg/mod")));
        let cypress = find("cypress");
        assert!(cypress.paths.contains(&mac.join("Cypress")));
        assert!(cypress.paths.contains(&xdg.join("Cypress")));
        assert!(cypress.paths.contains(&win_local.join("Cypress/Cache")));
        let electron = find("electron");
        assert!(electron.paths.contains(&mac.join("electron")));
        assert!(electron.paths.contains(&xdg.join("electron")));
        assert!(electron.paths.contains(&win_local.join("electron/Cache")));
    }

    #[test]
    fn test_registry_tier1_risk_and_category() {
        let reg = build_registry();
        let find = |id: &str| {
            reg.iter()
                .find(|e| e.id == id)
                .unwrap_or_else(|| panic!("missing entry {}", id))
        };
        for id in ["gradle", "maven", "go-mod", "xcode-deriveddata"] {
            assert_eq!(find(id).risk, "safe", "{} should be safe", id);
        }
        // Browser/runtime binaries follow the playwright/puppeteer convention
        for id in ["cypress", "electron"] {
            assert_eq!(find(id).risk, "heavy", "{} should be heavy", id);
        }
        assert_eq!(find("xcode-deriveddata").category, "Apple");
        assert_eq!(find("gradle").category, "Java");
        assert_eq!(find("maven").category, "Java");
    }
}

# MEMORY.md — deepclean Institutional Knowledge

Harvested patterns, domain knowledge, and conventions. Consult before starting work; harvest after completing work. (Created 2026-09-25.)

## 1. Discovery & Workspace Patterns

### Manifest-header workspace detection (proven)
- `src/project.rs:18-36` (`manifest_declares_workspace`): workspace detection scans manifest lines for exactly `[workspace]` or `[workspace.*]` prefixes, skipping `#` comments. No subprocesses — one file read per manifest. Scales to large trees.
- **Why not `cargo-metadata`**: subprocess spawn per manifest dominated discovery cost; header scanning is deterministic and platform-independent. NOTE: docs previously claimed cargo-metadata everywhere — that was drift, not reality. The registry uses manifest scanning only.
- Resolution algorithm (`src/project.rs:88-100`): collect all manifest dirs → build `HashSet` of workspace roots → resolve each manifest to its **nearest enclosing workspace root** (including itself) → dedup via `seen` HashSet on first encounter. Nested workspaces resolve to the nearest root, which is correct because each root owns its members' `target/`.

### walkdir traversal conventions
- Compile glob exclude patterns **once** before traversal, not per entry (`src/project.rs:45-48`)
- Prune hidden dirs (name starts with `.`) with `it.skip_current_dir()` — but never prune the root itself (`entry.depth() > 0` guard)
- Exclude matching compares the path **relative to root** (`strip_prefix`), so patterns like `**/node_modules` behave predictably
- Errors from walkdir are skipped (`continue`), never fatal — discovery degrades gracefully

## 2. Cleaning & Deletion Safety Patterns

### Deletion ordering and accounting
- `src/cleaner.rs:19-74`: compute target size **once** before any deletion; both `cargo clean` and `remove_dir_all` remove the whole dir, so freed = pre-clean size. Never re-walk after deletion.
- Fallback ordering: `cargo clean` first; on any failure (spawn error, non-zero status), fall back to direct `remove_dir_all(target)`. If target doesn't exist, report success (already clean).
- Dry-run gate sits **before** any I/O and returns early with the computed size (`src/cleaner.rs:27-34`). Cache cleaning mirrors this (`src/caches.rs:228-236`).

### Per-item error isolation
- `src/main.rs:279-414`: the rayon closure maps failures to `Ok(CleanResult { success: false, error })` instead of `Err`, so one broken project never aborts the batch; final exit code is 1 if any failed (`main.rs:438-440`). Cache mode does the same (`caches.rs:422-424`).
- Anti-pattern avoided: collecting `Result` with `?` inside parallel iterators aborts remaining work on first error.

### What must never be deleted
- Only `target/` under discovered project roots, and cache paths from the explicit registry. Never source files, `.git`, or anything not in the discovered scope. `remove_path` (`caches.rs:212-220`) handles dir/file/missing — missing is `Ok(())`, not an error.

## 3. Cache Discovery Patterns

### Registry structure
- `src/caches.rs:58-195` (`build_registry`): pure function returning all known caches (16 entries) with `(id, name, category, risk, paths)`. `risk` is `"safe"` (download cache, re-fetches) or `"heavy"` (re-download required). `discover_caches` computes sizes in parallel (`par_iter_mut`) and **filters out entries with size 0** — nonexistent paths simply vanish from the list.
- Cross-platform paths: `$HOME`-based (`.cargo`, `.npm`), XDG (`$XDG_CACHE_HOME` fallback `~/.cache`), macOS (`~/Library/Caches`), Windows (`%LOCALAPPDATA%` fallback `~/AppData/Local` via `caches::build_registry_for` `win_local` base — 2026-09-25). XDG-defaulted entries (`xdg.join(...)`) double as Windows paths for `~/.cache` tools (puppeteer, huggingface, torch, codex-runtimes).
- Tests guard invariants: unique ids, risk values ∈ {safe, heavy}, non-empty names/categories (`caches.rs` unit tests).

### Home-directory resolution (2026-09-25)
- `src/utils.rs` `resolve_home(home_env, userprofile_env)`: pure function; prefers `HOME`, falls back to `USERPROFILE`, treats empty strings as unset. `home_dir()` wraps the process-env read.
- **Bug lesson**: `home_env.or(userprofile_env).filter(!empty)` is wrong — a set-but-empty `HOME` swallows the `USERPROFILE` fallback. Filter each option *before* `.or_else()`. The unit test `test_resolve_home_treats_empty_as_unset` caught this before merge.
- **Env-var testing pattern**: extract env reads into pure functions taking `Option<OsString>` args; mutating process env in tests is racy under the parallel test harness.

### Interactive selection
- `parse_selection(input, count)` (`caches.rs:261-274`): pure, 1-based, comma/space separated, dedup, out-of-range and non-numeric tokens silently ignored — fully unit-testable without I/O.
- Shortcut tokens: `q|quit|exit|n|""` → cancel; `all|a|y|yes` → everything (`caches.rs:346-350`).

## 4. Dependency Detection Patterns

- `src/deps.rs`: native detection = parse `[dependencies]`, `[dev-dependencies]`, `[build-dependencies]` from Cargo.toml + text-scan `src/`, `examples/`, `tests/`, `build.rs` (each file read once via `collect_source_text`).
- 7 usage patterns per dep (`search_patterns_for`, `deps.rs:66-76`): `use x::`, `use x;`, `use crate::x`, `x::`, `extern crate x`, `x!`, `#[x`. Names normalized dashes→underscores (`normalize_crate_name`).
- Manifest cross-check: feature/alias references inside Cargo.toml count as usage (avoids false positives on renamed deps).
- **Known limitation** (documented in docs/DEPENDENCY_CLEANING.md): workspace discovery collapses members to roots, so only the root manifest is checked; `[workspace.dependencies]` inheritance not resolved.
- Removal shells out to `cargo-remove` (cargo-edit); detection itself has zero external-tool dependencies. `--remove-deps` implies `--clean-deps` (`main.rs:260`).

## 5. Parallelism & Performance Patterns

### Single-pass size caching (2026-09-25)
- `src/cleaner.rs` `compute_target_sizes(&[Project]) -> HashMap<PathBuf, u64>`: one rayon-parallel pass over all `target/` dirs, keyed by target path (missing → 0). Consumed by three phases in `main.rs`: `--min-size` filtering, `confirm_interactive` display, and dry-run freed-byte reporting.
- **Trust rule**: cached sizes are trusted **only on the dry-run path** (`clean_project(..., cached_size: Option<u64>)` — used exclusively when `dry_run`). Real runs always re-measure right before deletion: during an `--interactive` pause a build may grow/shrink the dir, and freed-byte accounting must reflect what was actually freed. Guard tests: `test_dry_run_uses_cached_size_without_rewalk` (cached value ≠ real size is reported as-is → proves no re-walk) and `test_real_run_ignores_cached_size` (stale value never reported).
- Plain real runs (no flags) skip the sizing pass entirely — one walk per project inside `clean_project`, same as before. Walk counts: min-size+interactive went 3→1, min-size+dry-run 2→1.
- Mature-module compliance: `utils::get_directory_size` untouched (still the sequential primitive); caching lives in cleaner.rs/main.rs which are not frozen.

- Global rayon pool built once from resolved `jobs` (`main.rs`); CLI `--jobs` overrides config overrides `available_parallelism()` (`config.rs:91-93`).
- `par_iter().with_min_len(1)` for project cleaning keeps per-item progress increments responsive.
- Parallel size computation: `entries.par_iter_mut().for_each(...)` in `discover_caches`. `get_directory_size` itself is still sequential walkdir (open TODO item — do not claim otherwise in docs).
- Progress bars (`indicatif`) are `Option`-gated: disabled when `--json` or `--verbose`, so output stays parseable. The `MultiProgress` is `Arc`-shared into rayon closures.

## 6. Output & CLI Patterns

### Cargo subcommand arg handling (critical)
- `src/main.rs:73-94` (`parse_args`): when argv[1] == "deepclean" (invoked as `cargo deepclean --flag`), re-prepend the program name so clap doesn't swallow the first flag as argv[0]. Regression risk if refactored — keep the test coverage in `tests/dry_run.rs` (`dry_run_via_subcommand_path_preserves_targets`).

### JSON vs human output
- Serde structs: `Summary` (output.rs), `CleanResult` (cleaner.rs), `CacheEntry`/`CacheCleanResult` (caches.rs). `CacheEntry.paths` is `#[serde(skip)]` — internal paths never leak into JSON.
- JSON mode suppresses all human chatter (`!settings.json` guards); exit-code contract: non-zero if any operation failed (both project and cache modes).
- `colored` output gated via `control::set_override(settings.use_color)`; color defaults to TTY detection (`std::io::stdout().is_terminal()`), overridable by `[output] color` in config.

### Config
- `.deepclean.toml` priority: CLI args > `./.deepclean.toml` > `$HOME/.deepclean.toml` > built-in defaults (`config.rs:41-53`). Exclude patterns from config and CLI are **concatenated**, not overridden (`config.rs:88-89`).

## 7. Testing Patterns

- Unit tests inline (`#[cfg(test)] mod tests`), integration tests in `tests/` using `tempfile::TempDir` fixtures — never real user dirs.
- Integration tests drive the compiled binary via `env!("CARGO_BIN_EXE_cargo-deepclean")` + `Command`, piping stdin for interactive prompts (`tests/caches_integration.rs`).
- Every deletion path has a dry-run regression test proving nothing is removed (`test_clean_caches_dry_run_keeps_dirs`, `tests/dry_run.rs`).
- Pure-function extraction is the testability lever: `parse_selection`, `resolve_settings`, `resolve_home`, `manifest_declares_workspace`, `parse_args_from`, `env_base_dir` all testable without I/O or env mutation.
- JSON output assertions parse stdout with `serde_json` and assert field presence/types.

### Coverage playbook (2026-09-25, llvm-cov)
- `parse_args` env-dependency removed by extracting `parse_args_from(Vec<String>) -> Args` (`src/main.rs`); `parse_args()` just collects `std::env::args()`. Subcommand-offset regression tests then run in-process (8 cases: direct vs `cargo deepclean` argv shapes, positional dir, `-j`, excludes). main.rs regions 56%→65%.
- **Subprocess loops stay uncovered by design**: `remove_unused_dependencies`'s `cargo remove` loop is environment-dependent; covering it needs a fake `cargo` shim on PATH (env manipulation = anti-pattern) or a command-factory seam (over-abstraction). Instead, test the cheap gates: `dry_run=true` → `Ok(0)`, empty list → `Ok(0)`, and `clean_dependencies(dry_run=true, remove=true)` full flow. deps.rs regions 80%→90.5%.
- Behavioral contracts documented as tests (they double as spec): manifest **rename key** (`foo = { package = "bar" }` → dep name is `foo`); `[workspace.dependencies]` **not extracted** (inherited deps can never be false-flagged — safe direction); skip-filter precision (`*_derive`/`proc-macro` names skipped, normal names still checked); invalid UTF-8 source skipped silently; `cc` used in `build.rs` not flagged.
- Don't trust test names — one test written with inverted assertions (`thiserror` expected-skipped) passed review by luck; assert all three sides of a filter (skipped-A, skipped-B, checked-C).

### JSON contract evolution (2026-09-25)
- `CleanResult.unused_deps` (`src/cleaner.rs`): new field populated by main.rs after `clean_dependencies` succeeds (`if let Ok(ref mut r) = result { r.unused_deps = deps_clean.unused_deps.clone() }`). Uses `#[serde(skip_serializing_if = "Vec::is_empty")]` so the field **disappears** unless `--clean-deps` ran — existing consumers see a byte-identical shape. Additive contract rule: new JSON fields must be opt-in-visible (skip-serialize) or documented as breaking.
- Contract tests live in `tests/orchestration.rs`: field present with `--clean-deps` (`json_clean_deps_includes_unused_deps_in_results`), field absent without it (`json_without_clean_deps_omits_unused_deps_field`). Every JSON field needs both a presence and an absence test.
- **Edit-safety lesson**: replacing a brace-terminated block via old/new strings can silently delete a closing `}` (match arm) — after structural edits, run `cargo check` immediately, don't batch.

## 8. Docs-Drift Anti-Patterns (2026-09-25 audit)

- **The cargo-metadata myth**: README/ARCHITECTURE/IMPROVEMENTS claimed `cargo-metadata` workspace detection while the code (and Cargo.toml) used manifest scanning. Lesson: when the implementation strategy changes, grep all docs for the old mechanism (`grep -r "cargo-metadata" *.md docs/`).
- **Tool-generation drift**: docs/DEPENDENCY_CLEANING.md described the old cargo-udeps/cargo-machete workflow after native detection shipped. Example output blocks in docs must match actual printed strings.
- **Platform overclaims**: "✅ Windows" while `$HOME`-only resolution silently no-ops `--caches` on Windows. Qualify support honestly; name the exact gap.
- **Stale checkboxes**: TODO.md items completed in code but left unchecked (integration tests, parallel size calc). Sweep TODO.md during audits.
- Rule of thumb: docs and code disagree → update the **doc**, and add the missing test if behavior was actually wrong.

## Gaps / Open Items Noted
- `get_directory_size` sequential traversal is the remaining performance TODO.
- Windows support not CI-tested (no Windows runner); registry paths are unit-tested via `build_registry_for` only.
- web2md MCP server unavailable in current environment; competitive-intelligence web research deferred (Step 8) — use built-in fetch when needed and harvest any durable findings here.

## 9. Process Patterns (2026-09-25)

### Uncommitted-work-tree drift
- Found TODO/ARCHITECTURE/MEMORY describing the `%LOCALAPPDATA%` registry as "missing" while it was fully implemented (with tests) in **uncommitted** working-tree changes. Lesson: before picking a TODO item, read the code + `git status`/`git diff` first — the item may be done but unharvested. Fix is docs-side (stale-checkbox anti-pattern, §8), not code.

### Empty-env-var fallback (`env_base_dir`, 2026-09-25)
- `src/caches.rs` `env_base_dir(env: Option<OsString>, fallback: PathBuf)`: `.filter(|v| !v.is_empty())` **before** `.unwrap_or(fallback)`. Applied to `XDG_CACHE_HOME` and `LOCALAPPDATA` reads in `build_registry`.
- Anti-pattern: `var_os(x).map(PathBuf::from).unwrap_or_else(fallback)` — a set-but-empty env var yields a bogus relative path `""` and silently swallows the fallback. Same class as the `resolve_home` bug (§3). Grep for this shape when adding any new env-derived path.
- Guard test: `test_env_base_dir_treats_empty_as_unset` (caches.rs).

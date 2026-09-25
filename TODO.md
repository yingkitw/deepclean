# TODO

## Scorecard (2026-09-25, after orchestration integration tests)

| Dimension | Score | Evidence |
|---|---|---|
| Correctness & coverage | 5 | 111 tests green; llvm-cov total 92.5% regions; main.rs 85.7% (rest: failure-path println branches needing permission-error injection — diminishing returns); deps.rs 90.5%; every deletion path has a dry-run test |
| Deletion safety | 5 | All destructive paths gated (dry-run/interactive/confirm); per-item error isolation; cached-size trust rule (dry-run only) regression-tested; interactive-cancel preserves target (integration-proven) |
| Maintainability | 5 | Clippy at 2 warnings, both in `project.rs` — frozen per Mature Module Policy, documented |
| Docs alignment | 5 | TODO/ARCHITECTURE/MEMORY/README audited across iterations; JSON `unused_deps` contract documented in README |
| Wiring & ergonomics | 5 | All flags wired and unit-tested; `--clean-deps` findings now in JSON `results[].unused_deps` (presence + absence contract tests) |
| Footprint | 5 | 11 direct deps, all used; no new deps added |
| **Total** | **30/30** | Remaining known gaps are all documented deliberate trade-offs (subprocess loops env-dependent, project.rs frozen, failure-path printlns) |

### Improvement item from weakest dimension
- [x] Raise coverage of the detection/orchestration core
  - [x] deps.rs ≥ 90%: unit tests for pattern edge cases (feature-gated deps, renamed deps, build-deps, `[workspace.dependencies]` skip) — 90.5% regions; rename-key/workspace-skip/skip-filter contracts encoded as tests
  - [x] main.rs `parse_args`: direct unit tests for subcommand arg-offset handling and flag permutations — `parse_args_from` extracted, 8 in-process tests
- [x] Raise main.rs orchestration coverage — `tests/orchestration.rs` (9 tests): verbose human mode, clean-deps dry-run messaging, no-projects warning, min-size empty filter, invalid min-size error, interactive cancel, JSON/verbose non-interference, deps-in-JSON presence + absence contracts
- [x] Include unused-deps findings in the JSON `Summary` contract — `results[].unused_deps` via `skip_serializing_if` (absent unless `--clean-deps` ran); README documents the contract

## High Priority

### Code Organization
- [x] Create TODO.md
- [x] Create ARCHITECTURE.md
- [x] Modularize code: Split main.rs into separate modules
  - [x] `src/project.rs` - Project discovery and workspace detection
  - [x] `src/cleaner.rs` - Cleaning logic
  - [x] `src/output.rs` - Output formatting and display
  - [x] `src/utils.rs` - Utility functions (format_bytes, get_directory_size)
  - [x] `src/config.rs` - Configuration file support

### Testing
- [x] Add unit tests for core functionality
  - [x] Test project discovery
  - [x] Test workspace detection
  - [x] Test size calculation
  - [x] Test byte formatting
  - [x] Test exclude pattern matching

### Features
- [x] Add configuration file support (`.deepclean.toml`)
  - [x] Default exclude patterns
  - [x] Default job count
  - [x] Default output format preferences
- [x] Add interactive confirmation mode (`--interactive` flag)
- [x] Add `--min-size` flag to only clean projects above threshold
- [x] Add `--clean-deps` flag to detect and remove unused dependencies
  - [x] Detect unused dependencies using native Cargo.toml + source analysis
  - [x] Report unused dependencies
  - [x] Optionally remove them (with confirmation via `--remove-deps`)
- [x] Add `--caches` flag to list and interactively clean global toolchain caches
  - [x] Discover caches (npm, bun, cargo, pip, uv, homebrew, huggingface, torch, puppeteer, playwright, go-build, codex-runtimes)
  - [x] Show estimated reclaimable space per cache
  - [x] Interactive multi-select (numbers, 'all', 'q')
  - [x] Risk tagging (safe vs heavy) and notes
  - [x] JSON output mode for automation
  - [x] Dry-run support
- [x] Fix `cargo deepclean <flags>` swallowing the first flag as the binary name
- [x] Improve error messages with context and suggestions

## Medium Priority

### Performance
- [ ] Optimize directory size calculation
  - [ ] Use faster method (consider `du` command on Unix)
  - [x] Cache size results during discovery (`cleaner::compute_target_sizes` — one parallel pass reused by `--min-size` filtering, `--interactive` confirmation, and dry-run reporting; real runs still re-measure pre-deletion for accurate freed-byte accounting)
  - [ ] Parallelize `get_directory_size` traversal itself
  - [x] Parallel size computation for `--caches` discovery and `--min-size` filtering
- [ ] Add progress indication for size calculation phase

### User Experience
- [ ] Add `--keep` flag to preserve certain build artifacts
- [ ] Add `--only-workspaces` flag to only clean workspace roots
- [ ] Add `--only-standalone` flag to only clean standalone projects
- [x] Add color support detection (auto-disable on non-TTY)

### Platform Support
- [x] Resolve home directory via `USERPROFILE` fallback on Windows (`utils::resolve_home`; fixes `--caches` and home config fallback silently no-oping when `HOME` is unset)
- [x] Add `%LOCALAPPDATA%`-based cache paths to the `--caches` registry (npm, pip, uv, Poetry, pnpm, Yarn, Playwright, go-build — `caches::build_registry_for` `win_local` base dir; `%LOCALAPPDATA%` unset/empty falls back to `~/AppData/Local` via `env_base_dir`)

### Code Quality
- [ ] Add proper logging framework (tracing or log crate)
- [ ] Improve documentation with more examples
- [x] Add integration tests (tests/: caches, caches_integration, dry_run, filtering, multi_project)
- [ ] Add benchmarks for performance-critical paths

## Low Priority

### Infrastructure
- [ ] Add GitHub Actions CI/CD
  - [ ] Run tests on push
  - [ ] Build for multiple platforms
  - [ ] Publish to crates.io on release
- [ ] Add pre-commit hooks
- [ ] Add code coverage reporting

### Documentation
- [ ] Add man page
- [ ] Add shell completion scripts (bash, zsh, fish)
- [ ] Add more usage examples in README
- [ ] Add troubleshooting guide

## Future Ideas

- [ ] Support for cleaning other build artifacts (node_modules, etc.)
- [ ] Integration with cargo-watch for automatic cleaning
- [ ] Statistics tracking (how much space saved over time)
- [ ] Web UI for monitoring cleaning operations
- [ ] Support for remote cleaning (SSH)
- [ ] `--caches --all` non-interactive flag to clean all safe caches without prompting
- [ ] Configurable cache registry via `.deepclean.toml` (custom paths, exclusions)
- [ ] Use tool-native clean commands (e.g. `npm cache clean`, `brew cleanup`) where they clean more than `rm -rf`

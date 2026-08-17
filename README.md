# deepclean 🧹 — Fast Rust Cache Cleaner & Disk Space Recovery

**deepclean** is a fast, parallel command-line tool written in Rust that reclaims disk space by cleaning Cargo `target/` build directories, removing unused dependencies, and clearing global toolchain caches (npm, Bun, cargo, pip, uv, Homebrew, HuggingFace, Playwright, Puppeteer, and more).

It works as a `cargo` subcommand (`cargo deepclean`) and scans your projects in parallel across all CPU cores, so cleaning dozens of repositories takes seconds instead of minutes.

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/yingkitw/deepclean)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

## Why deepclean?

Rust projects accumulate **gigabytes** of build artifacts in `target/` directories. A single workspace can consume 5–10 GB; a folder of projects can eat 50+ GB. On top of that, global toolchain caches (npm's `_cacache`, Bun's install cache, HuggingFace model weights, Playwright browsers) silently grow to tens of gigabytes over months of development.

**deepclean solves three problems at once:**

- 🚀 **Parallel Cargo cleaning** — removes `target/` directories across many projects simultaneously, using all CPU cores.
- 🧹 **Unused dependency removal** — detects and optionally removes dependencies in `Cargo.toml` that your source code no longer uses, speeding up builds.
- 💾 **Global cache cleaning** — lists npm, Bun, cargo, pip, uv, Homebrew, HuggingFace, and other caches with estimated reclaimable sizes, then lets you interactively pick which to clean.

## Quick Start

### Install

```bash
# From source (recommended for latest features)
git clone https://github.com/yingkitw/deepclean.git
cd deepclean
cargo install --path .

# Or directly from the repository
cargo install --git https://github.com/yingkitw/deepclean.git
```

> **Note:** Ensure `~/.cargo/bin` is on your `PATH` so the `cargo deepclean` subcommand is found.

### First run

```bash
# Clean target/ directories in the current folder and all subfolders
cargo deepclean

# Preview what would be cleaned (no deletion)
cargo deepclean --dry-run
```

## Features

### Cargo project cleaning

- ✅ **Parallel processing** — cleans multiple projects simultaneously across all CPU cores
- ✅ **Smart workspace detection** — uses `cargo-metadata` to find and collapse workspaces accurately
- ✅ **Size filtering** — `--min-size 100MB` cleans only projects above a threshold
- ✅ **Exclude patterns** — skip directories with glob patterns (`-e "**/node_modules"`)
- ✅ **Dry-run mode** — preview exactly what would be cleaned before deleting
- ✅ **Progress bars** — real-time progress via `indicatif`
- ✅ **JSON output** — machine-readable results for scripts and CI

### Unused dependency detection

- ✅ **Built-in detection** — parses `Cargo.toml` and scans source code; no external tools required
- ✅ **Report mode** — `--clean-deps` lists unused dependencies
- ✅ **Auto-remove** — `--remove-deps` removes them (requires `cargo-edit`)

### Global toolchain cache cleaning (new)

- ✅ **Discovers 16+ caches** — npm, Bun, pnpm, Yarn, cargo registry, pip, uv, Poetry, Homebrew, HuggingFace, PyTorch, Puppeteer, Playwright, go-build, codex-runtimes
- ✅ **Estimated sizes** — shows reclaimable disk space per cache, computed in parallel
- ✅ **Risk tagging** — each cache is marked **safe** (re-fetches on demand) or **heavy** (re-download required, e.g. model weights or browser binaries)
- ✅ **Interactive selection** — pick caches by number, type `all`, or `q` to quit
- ✅ **Dry-run and JSON** — preview without deleting, or emit JSON for automation

## CLI Options

| Option | Description |
|--------|-------------|
| `[DIRECTORY]` | Directory to start cleaning from (default: `.`) |
| `-j, --jobs <N>` | Number of parallel jobs (default: CPU count) |
| `-e, --exclude <PATTERN>` | Exclude directories matching a glob pattern (repeatable) |
| `--dry-run` | Preview mode — no changes are made |
| `--min-size <SIZE>` | Only clean projects above this size (e.g. `100MB`, `1GB`) |
| `--clean-deps` | Detect and report unused dependencies |
| `--remove-deps` | Remove unused dependencies (requires `cargo-edit`) |
| `--caches` | List global toolchain caches and interactively select which to clean |
| `--interactive` | Prompt for confirmation before cleaning projects |
| `-v, --verbose` | Verbose output |
| `--json` | Output results as JSON |
| `-h, --help` | Print help |

## Usage Examples

### Clean all Cargo projects in a directory tree

```bash
cargo deepclean /path/to/projects
```

### Clean only large projects (above 500 MB)

```bash
cargo deepclean --min-size 500MB
```

### Preview without deleting (dry run)

```bash
cargo deepclean --dry-run
```

### Find and remove unused dependencies

```bash
# Detect only
cargo deepclean --clean-deps

# Detect and remove (requires cargo-edit)
cargo deepclean --remove-deps
```

### Exclude specific directories

```bash
cargo deepclean --exclude "**/target/debug" --exclude "**/node_modules"
```

### Limit parallelism

```bash
cargo deepclean -j 8
```

### Clean global toolchain caches

Beyond per-project `target/` directories, dev toolchains accumulate large global caches. `--caches` discovers them, shows their sizes, and lets you interactively pick which to clean:

```bash
# Interactive: pick caches by number, 'all', or 'q' to quit
cargo deepclean --caches

# Preview without deleting
cargo deepclean --caches --dry-run

# Machine-readable list of discovered caches (JSON array)
cargo deepclean --caches --json
```

Each cache is tagged **safe** (pure download cache that re-fetches on demand) or **heavy** (model weights, browser binaries, or runtimes that must re-download). Caches are removed with `rm -rf` and regenerated automatically by their tools on next use.

#### Supported caches

| Category | Caches |
|----------|--------|
| Rust | cargo registry cache, cargo registry src |
| Python | pip, uv, Poetry |
| JS/TS | npm (`_cacache` + `_npx`), Bun, pnpm, Yarn |
| Other | Homebrew, Puppeteer, Playwright, HuggingFace, PyTorch models, go-build, codex-runtimes |

### JSON output for automation and CI

```bash
cargo deepclean --json
cargo deepclean --caches --json
```

## How It Works

1. **Discovery** — recursively finds all `Cargo.toml` files using `walkdir` and resolves workspaces with `cargo-metadata`.
2. **Filtering** — applies exclude patterns and optional `--min-size` threshold.
3. **Cleaning** — removes `target/` directories in parallel via `rayon`; falls back to direct `rm -rf` if `cargo clean` fails.
4. **Dependency analysis** — parses `Cargo.toml` and scans `src/`, `examples/`, `tests/`, and `build.rs` for dependency usage; reports or removes unused crates.
5. **Cache cleaning** (`--caches`) — scans known global cache locations, computes sizes in parallel, presents an interactive selection, and removes chosen caches.

## Requirements

- **Rust toolchain** and **Cargo** (for installation and project cleaning)
- **Optional:** [`cargo-edit`](https://github.com/killercup/cargo-edit) for `--remove-deps` (detection works without it)

```bash
cargo install cargo-edit
```

## Performance

deepclean is built in Rust for maximum performance:

- **Parallel execution** across all CPU cores via `rayon`'s work-stealing scheduler
- **Efficient directory traversal** with `walkdir` and early pruning of hidden/excluded dirs
- **Parallel size calculation** for global cache discovery
- **Minimal memory footprint** — streams results rather than buffering

## FAQ

### Is deepclean safe to use?

Yes. `--dry-run` previews every change without deleting anything. Project cleaning removes only `target/` directories (regenerated by the next `cargo build`). Cache cleaning removes only the listed cache directories, which tools re-create on next use. The `--interactive` flag adds a confirmation prompt before cleaning projects.

### Does deepclean modify my source code?

Only when you explicitly pass `--remove-deps`, which edits `Cargo.toml` to remove unused dependencies. Without that flag, deepclean only deletes `target/` directories and (with `--caches`) global cache directories.

### How is deepclean different from `cargo clean`?

`cargo clean` cleans a single project. deepclean finds and cleans **all** projects in a directory tree in parallel, detects unused dependencies, and cleans global toolchain caches — none of which `cargo clean` does.

### Which caches does `--caches` support?

npm, Bun, pnpm, Yarn, cargo registry, pip, uv, Poetry, Homebrew, Puppeteer, Playwright, HuggingFace, PyTorch, go-build, and codex-runtimes. See the [Supported caches](#supported-caches) table above.

### Can I use deepclean in CI?

Yes. Use `--json` for machine-readable output and `--dry-run` for safe previews. The exit code is non-zero if any cleaning operation fails.

## Contributing

Contributions are welcome! Please open issues or submit pull requests at [github.com/yingkitw/deepclean](https://github.com/yingkitw/deepclean).

## License

Apache-2.0

## Links

- **Repository:** [github.com/yingkitw/deepclean](https://github.com/yingkitw/deepclean)
- **Documentation:** [docs.rs/deepclean](https://docs.rs/deepclean)
- **Crates.io:** [crates.io/crates/deepclean](https://crates.io/crates/deepclean)

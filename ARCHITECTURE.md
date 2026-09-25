# Architecture

## Overview

`deepclean` is a cargo subcommand that recursively finds and cleans Cargo projects. It's designed to be fast, reliable, and user-friendly.

## Design Principles

1. **Modularity**: Code is organized into logical modules for maintainability
2. **Performance**: Parallel processing for speed, efficient algorithms
3. **Reliability**: Robust error handling, graceful degradation
4. **User Experience**: Clear output, progress indication, helpful error messages
5. **Testability**: Code structured for easy unit and integration testing

## Architecture Layers

### 1. CLI Layer (`src/main.rs`)
- Argument parsing using `clap`
- Handles cargo subcommand invocation
- Orchestrates the cleaning process
- Manages output formatting

### 2. Project Discovery (`src/project.rs`)
- Finds Cargo projects recursively
- Detects workspaces by scanning manifests for `[workspace]` tables
- Filters projects based on exclude patterns
- Handles edge cases (nested workspaces, etc.)

### 3. Cleaning Logic (`src/cleaner.rs`)
- Executes `cargo clean` commands
- Falls back to direct directory removal
- Calculates space freed
- Handles errors gracefully

### 4. Output Layer (`src/output.rs`)
- Formats output (human-readable and JSON)
- Manages progress bars
- Color-coded messages
- Summary generation

### 5. Utilities (`src/utils.rs`)
- Byte formatting
- Directory size calculation
- Path utilities
- Size string parsing

### 6. Dependency Cleaning (`src/deps.rs`)
- Detects unused dependencies natively (Cargo.toml parsing + source scan; no external tools)
- Matches dependency names against usage patterns (use statements, path expressions, macros, attributes)
- Removes unused dependencies using cargo-remove
- Reports dependency cleanup results

### 7. Configuration (`src/config.rs`)
- Loads `.deepclean.toml` configuration files
- Merges CLI args with config
- Validates configuration

### 8. Global Cache Cleaning (`src/caches.rs`)
- Discovers global toolchain caches (npm, bun, cargo registry, pip, uv, Homebrew, HuggingFace, torch, Puppeteer, Playwright, go-build, codex-runtimes)
- Computes reclaimable sizes in parallel
- Tags each cache as `safe` (download cache) or `heavy` (re-download required)
- Interactive multi-select prompt; `--json` emits the discovered list for automation
- Removes selected caches with `rm -rf` (tools regenerate on next use)

## Data Flow

```
User Input (CLI args)
    ↓
[if --caches] Cache Discovery + Interactive Select → Cache Cleaning → Summary
    ↓ (otherwise)
Config Loading (if .deepclean.toml exists)
    ↓
Project Discovery (walkdir + manifest scan)
    ↓
Project Filtering (exclude patterns, size thresholds)
    ↓
Parallel Cleaning (rayon)
    ↓
Result Collection
    ↓
Output Formatting (human-readable or JSON)
```

## Key Components

### Project Discovery

**Algorithm:**
1. Walk directory tree using `walkdir`
2. Find all `Cargo.toml` files
3. For each found file:
   - Check if it declares a `[workspace]` table (manifest header scan)
   - If it's a workspace member, add workspace root (once)
   - If standalone, add project directory
4. Deduplicate results

**Workspace Detection:**
- Scans manifest headers for `[workspace]` / `[workspace.*]` tables, skipping comments
- No subprocesses — one file read per manifest, scales to large trees
- Resolves each manifest to its nearest enclosing workspace root
- Handles nested workspaces correctly
- Avoids duplicate workspace processing

### Cleaning Process

**Size Computation (single-pass caching):**
1. When any phase needs sizes (`--min-size`, `--interactive`, `--dry-run`), all `target/` dirs are sized once in parallel up front (`cleaner::compute_target_sizes`)
2. The min-size filter and interactive confirmation reuse the cached sizes instead of re-walking
3. Dry-run reporting trusts the cached size (nothing mutates the disk, so it is still exact)
4. Real runs always re-measure immediately before deletion, so freed-byte accounting stays accurate even if a build ran during an interactive pause
5. Plain real runs (no flags) skip the up-front pass entirely — one walk per project inside `clean_project`

**Target Directory Cleaning:**
1. Calculate target directory size before cleaning
2. Try `cargo clean` command first
3. If that fails, fall back to direct `rm -rf target`
4. Report the pre-clean size as space freed (the whole directory is removed, no post-walk needed)

**Dependency Cleaning (optional):**
1. Parse `Cargo.toml` to extract all dependencies
2. Search through source code (`src/`, `examples/`, `tests/`, `build.rs`) for usage
3. Match dependency names against code patterns (use statements, macro invocations, etc.)
4. Report unused dependencies
5. If `--remove-deps` is set, use `cargo-remove` to remove them
6. Report removal status

**Parallelization:**
- Uses `rayon` for parallel execution
- Configurable job count (default: CPU count)
- Thread-safe progress reporting

### Error Handling

**Approach:**
- Continue processing other projects if one fails
- Collect all errors and report at end
- Provide context in error messages
- Exit with non-zero code if any failures

## Dependencies

### Core
- `rayon`: Parallel processing
- `clap`: CLI argument parsing
- `anyhow`: Error handling

### UI/Output
- `indicatif`: Progress bars
- `colored`: Colored terminal output
- `serde`/`serde_json`: JSON serialization

### Utilities
- `walkdir`: Directory traversal and manifest discovery
- `glob`: Pattern matching for excludes
- `toml`: Manifest and config file parsing

## Configuration

### Configuration File (`.deepclean.toml`)

Located in project root or home directory:

```toml
[defaults]
exclude = ["**/node_modules", "**/.git"]
jobs = 4
min_size = "100MB"

[output]
color = true
format = "human"  # or "json"
```

### Configuration Priority

1. CLI arguments (highest priority)
2. `.deepclean.toml` in current directory
3. `~/.deepclean.toml` in home directory
4. Built-in defaults (lowest priority)

## Performance Considerations

### Directory Size Calculation

**Current Approach:**
- Walk directory tree and sum file sizes
- Sequential for each project
- Can be slow for large target directories

**Future Optimization:**
- Use platform-specific tools (`du` on Unix)
- Cache results during discovery
- Parallel size calculation

### Project Discovery

**Optimization:**
- Early filtering with `filter_entry` in walkdir
- Skip hidden directories immediately
- Check exclude patterns before deep traversal

### Parallel Processing

**Strategy:**
- Use rayon's work-stealing scheduler
- Configurable parallelism
- Balance between CPU and I/O bound work

## Testing Strategy

### Unit Tests
- Test individual functions in isolation
- Mock external dependencies (cargo commands)
- Test edge cases and error conditions

### Integration Tests
- Test full workflow with real cargo projects
- Test workspace detection with various structures
- Test exclude patterns

### Performance Tests
- Benchmark directory size calculation
- Benchmark project discovery
- Benchmark parallel cleaning

## Extension Points

### Adding New Features

1. **New CLI flags**: Add to `Args` struct in `main.rs`
2. **New cleaning strategies**: Extend `cleaner.rs`
3. **New output formats**: Extend `output.rs`
4. **New project filters**: Extend `project.rs`

### Plugin System (Future)

Could support plugins for:
- Custom cleaning strategies
- Custom project filters
- Custom output formatters

## Security Considerations

1. **Path Traversal**: Validate all paths before operations
2. **Command Injection**: Use structured command execution, never shell
3. **Permissions**: Handle permission errors gracefully
4. **Symlinks**: Follow symlinks safely (or skip them)

## Platform Support

### Current
- ✅ Linux
- ✅ macOS
- ⚠️ Windows — near-complete: project cleaning, dependency detection, home-directory resolution (`HOME` → `USERPROFILE` fallback in `utils::resolve_home`), and `--caches` registry paths (`%LOCALAPPDATA%`-based via `caches::build_registry_for`, covering npm, pip, uv, Poetry, pnpm, Yarn, Playwright, go-build; XDG-defaulted entries cover `~/.cache` tools like Puppeteer/HuggingFace/PyTorch) all work. Not CI-tested on Windows.

### Platform-Specific Considerations
- Path separators handled by Rust stdlib
- Line endings handled automatically
- Terminal colors detected automatically
- Home-directory resolution checks `HOME` then `USERPROFILE` (`src/utils.rs`)

## Future Architecture Improvements

1. **Async/Await**: Consider async for I/O-bound operations
2. **Streaming**: Stream results instead of collecting all
3. **Caching**: Cache project discovery results
4. **Incremental**: Only clean projects that have changed


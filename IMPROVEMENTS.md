# Design Decisions & Rationale

Why deepclean is written in Rust, and why each major crate and technique was chosen. For current module structure and data flow, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Why Rust?

This project was originally implemented as a bash script, but was rewritten in Rust for the following reasons:

### Issues with Bash Implementation

1. **Fragile Workspace Detection**
   - Used `grep "^\[workspace\]"` which could match in comments or strings
   - Didn't use Cargo's metadata API for accurate workspace detection
   - Could incorrectly skip or process workspace members

2. **OS-Specific Code**
   - macOS-specific `stat -f%z` for directory size
   - Required different logic for different platforms
   - Not truly cross-platform

3. **Performance**
   - Sequential processing (one project at a time)
   - No parallel execution
   - Slow for large project trees

4. **Limited Features**
   - No dry-run mode
   - No exclude patterns
   - No progress indication for large operations

5. **Code Quality**
   - Used `eval` for variable manipulation (error-prone)
   - String-based arithmetic (can fail with edge cases)
   - Limited error messages

6. **Fragile Detection Logic**
   - `grep`-based workspace detection couldn't resolve members to their workspace root
   - No deduplication of workspace roots
   - No handling of nested workspaces

## Rust Implementation Benefits
**Advantages:**
- ✅ **Cross-platform**: No OS-specific code needed
- ✅ **Better error handling**: Result types provide type-safe error handling
- ✅ **Proper workspace detection**: Manifest-header scanning (`[workspace]` tables) with nearest-root resolution and deduplication
- ✅ **Parallel processing**: Uses `rayon` for concurrent cleaning
- ✅ **Type safety**: Compile-time guarantees prevent many bugs
- ✅ **Better performance**: Parallel execution significantly faster
- ✅ **Single binary**: No shell dependencies, easy distribution
- ✅ **Easier to test**: Unit testing is straightforward
- ✅ **Cargo plugin**: Can be installed and used as `cargo deepclean`

**Implemented Features:**
- ✅ Parallel cleaning with configurable concurrency
- ✅ Dry-run mode (`--dry-run`)
- ✅ Exclude patterns (`--exclude` with glob support)
- ✅ Progress bars with real-time project status (using `indicatif`)
- ✅ Robust workspace detection using cargo metadata
- ✅ JSON output option (`--json`)
- ✅ Verbose mode (`--verbose`)
- ✅ Size calculation and reporting

## Architecture Decisions

### Why manifest-based workspace detection?
- No subprocesses — one file read per manifest, so discovery scales to large trees
- A targeted scan of table headers (`[workspace]`, `[workspace.*]`), skipping comments, is deterministic and platform-independent
- Members resolve to their nearest enclosing workspace root; roots are deduplicated
- Avoids the version-pinning and compile-time cost of heavyweight metadata crates

### Why rayon?
- Simple parallel processing API
- Automatically manages thread pool
- Efficient work stealing for load balancing

### Why indicatif?
- Beautiful progress bars
- MultiProgress support for showing multiple active operations
- Non-intrusive (can be disabled for JSON/verbose modes)

### Why clap?
- Industry standard for Rust CLI tools
- Excellent help generation
- Supports both cargo plugin and standalone usage


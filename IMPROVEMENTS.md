# Design Decisions & Architecture

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

6. **No Proper Cargo Integration**
   - Didn't use `cargo metadata` for workspace detection
   - Manual parsing of Cargo.toml files
   - Could miss edge cases in workspace configuration

## Rust Implementation Benefits
**Advantages:**
- ✅ **Cross-platform**: No OS-specific code needed
- ✅ **Better error handling**: Result types provide type-safe error handling
- ✅ **Proper workspace detection**: Uses `cargo-metadata` crate for accurate workspace detection
- ✅ **Parallel processing**: Uses `rayon` for concurrent cleaning
- ✅ **Type safety**: Compile-time guarantees prevent many bugs
- ✅ **Better performance**: Parallel execution significantly faster
- ✅ **Single binary**: No shell dependencies, easy distribution
- ✅ **Easier to test**: Unit testing is straightforward
- ✅ **Cargo plugin**: Can be installed and used as `cargo rclean`

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

### Why cargo-metadata?
- Uses Cargo's own APIs for workspace detection
- Handles all edge cases that manual parsing would miss
- Automatically handles workspace member resolution

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


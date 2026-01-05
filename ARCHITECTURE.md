# Architecture

## Overview

`rclean` is a cargo subcommand that recursively finds and cleans Cargo projects. It's designed to be fast, reliable, and user-friendly.

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
- Detects workspaces using `cargo-metadata`
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

### 6. Configuration (`src/config.rs`)
- Loads `.rclean.toml` configuration files
- Merges CLI args with config
- Validates configuration

## Data Flow

```
User Input (CLI args)
    ↓
Config Loading (if .rclean.toml exists)
    ↓
Project Discovery (walkdir + cargo-metadata)
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
   - Check if it's part of a workspace (using `cargo-metadata`)
   - If workspace member, add workspace root (once)
   - If standalone, add project directory
4. Deduplicate results

**Workspace Detection:**
- Uses `cargo-metadata` API for accurate detection
- Handles nested workspaces correctly
- Avoids duplicate workspace processing

### Cleaning Process

**Strategy:**
1. Calculate target directory size before cleaning
2. Try `cargo clean` command first
3. If that fails, fall back to direct `rm -rf target`
4. Calculate actual space freed
5. Report results

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
- `cargo-metadata`: Workspace detection
- `rayon`: Parallel processing
- `clap`: CLI argument parsing
- `anyhow`: Error handling

### UI/Output
- `indicatif`: Progress bars
- `colored`: Colored terminal output
- `serde`/`serde_json`: JSON serialization

### Utilities
- `walkdir`: Directory traversal
- `glob`: Pattern matching for excludes

## Configuration

### Configuration File (`.rclean.toml`)

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
2. `.rclean.toml` in current directory
3. `~/.rclean.toml` in home directory
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
- ✅ Windows

### Platform-Specific Considerations
- Path separators handled by Rust stdlib
- Line endings handled automatically
- Terminal colors detected automatically

## Future Architecture Improvements

1. **Async/Await**: Consider async for I/O-bound operations
2. **Streaming**: Stream results instead of collecting all
3. **Caching**: Cache project discovery results
4. **Incremental**: Only clean projects that have changed


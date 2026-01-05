# Dependency Cleaning Feature

## Overview

The dependency cleaning feature allows `rclean` to detect and optionally remove unused dependencies from your Cargo projects. This helps keep your `Cargo.toml` files clean and reduces build times.

## How It Works

### Detection

`rclean` uses external tools to detect unused dependencies:

1. **cargo-udeps** (recommended): More accurate, uses nightly compiler features
   - Detects dependencies that are never imported
   - Can detect hidden imports from macro expansions (with `--expand` flag)
   - Outputs JSON format

2. **cargo-machete** (fallback): Simpler, works on stable Rust
   - Quick detection of obviously unused dependencies
   - Text-based output

### Removal

If `--remove-deps` is specified, `rclean` will attempt to remove unused dependencies using:

- **cargo-remove** (from cargo-edit): Safely removes dependencies from `Cargo.toml`
  - Handles both `[dependencies]` and `[dev-dependencies]`
  - Preserves formatting and comments where possible

## Usage

### Basic Detection

```bash
# Check for unused dependencies (dry-run by default)
cargo rclean --clean-deps

# Check with verbose output
cargo rclean --clean-deps --verbose
```

### Detection and Removal

```bash
# Check and remove unused dependencies
cargo rclean --clean-deps --remove-deps

# Preview what would be removed (dry-run)
cargo rclean --clean-deps --remove-deps --dry-run
```

### Combined with Target Cleaning

```bash
# Clean both target directories and unused dependencies
cargo rclean --clean-deps --remove-deps
```

## Installation Requirements

### For Detection

Install one of:
```bash
cargo install cargo-udeps    # Recommended (more accurate)
# or
cargo install cargo-machete  # Simpler, works on stable
```

### For Removal

```bash
cargo install cargo-edit  # Provides cargo-remove command
```

## How Detection Works

1. **cargo-udeps approach**:
   - Runs `cargo udeps --output json` in each project
   - Parses JSON output to find unused dependencies
   - More accurate but requires nightly Rust for full features

2. **cargo-machete approach** (fallback):
   - Runs `cargo machete` in each project
   - Parses text output looking for "unused dependency: `name`" lines
   - Works on stable Rust but may miss some edge cases

## Limitations

1. **False Positives**: Some dependencies might appear unused but are actually needed:
   - Build scripts (`build.rs`)
   - Procedural macros
   - Conditional compilation features
   - Dynamic loading

2. **Tool Availability**: 
   - If neither `cargo-udeps` nor `cargo-machete` is installed, dependency checking is skipped
   - If `cargo-remove` is not installed, dependencies are detected but not removed

3. **Workspace Handling**:
   - Each workspace member is checked individually
   - Workspace-level dependencies are not currently checked

## Best Practices

1. **Always use `--dry-run` first** to see what would be removed
2. **Review the output** before removing dependencies
3. **Test after removal** to ensure nothing breaks
4. **Use version control** so you can revert if needed
5. **Consider using `cargo-udeps`** for more accurate detection

## Example Output

```
[INFO] Starting cargo clean from: "/path/to/projects"
[INFO] Found 3 project(s)
[INFO] Dependency cleaning enabled (requires cargo-udeps or cargo-machete)

⠋ Cleaning: project1
[INFO] Found 2 unused dependency(ies) in project1
  - unused-crate ([dependencies])
  - old-util ([dependencies])
[SUCCESS] Removed 2 unused dependency(ies)

✓ project1
[████████████████████████] 3/3 projects completed
```

## Future Improvements

- [ ] Better JSON parsing for cargo-udeps output
- [ ] Support for workspace-level dependency checking
- [ ] Manual Cargo.toml editing as fallback
- [ ] Integration with cargo-audit for security checks
- [ ] Support for checking feature flags


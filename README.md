# rclean

A cargo subcommand to recursively clean Cargo projects with workspace support.

## 🚀 Quick Start

### As a Cargo Plugin (Recommended)

```bash
# Install from local source
cargo install --path .

# Or install from crates.io (when published)
# cargo install rclean

# Use it as a cargo subcommand
cargo rclean [options] [directory]
```

### Standalone Binary

```bash
# Build the tool
cargo build --release

# Use it directly
./target/release/cargo-rclean [options] [directory]
```

## 🎯 Usage

### As a Cargo Plugin

```bash
# Clean all projects in current directory
cargo rclean

# Clean projects in specific directory
cargo rclean /path/to/projects

# Dry run (preview what would be cleaned)
cargo rclean --dry-run

# Exclude certain patterns
cargo rclean --exclude "**/target" --exclude "**/node_modules"

# Parallel processing with custom job count
cargo rclean -j 8

# Verbose output
cargo rclean --verbose

# JSON output for scripting
cargo rclean --json

# Only clean projects above 100MB
cargo rclean --min-size 100MB

# Check for unused dependencies (requires cargo-udeps or cargo-machete)
cargo rclean --clean-deps

# Check and remove unused dependencies (requires cargo-remove)
cargo rclean --clean-deps --remove-deps
```

### As a Standalone Binary

```bash
# Same commands, but use cargo-rclean instead of cargo rclean
cargo-rclean
cargo-rclean /path/to/projects
cargo-rclean --dry-run
# ... etc
```

### Advanced Options

```bash
cargo rclean --help
```

**Options:**
- `-j, --jobs <N>`: Number of parallel jobs (default: CPU count)
- `-e, --exclude <PATTERN>`: Exclude glob patterns (can be specified multiple times)
- `--dry-run`: Preview mode (doesn't actually clean)
- `--min-size <SIZE>`: Only clean projects above this size threshold (e.g., "100MB", "1GB")
- `--clean-deps`: Check for unused dependencies (built-in detection, no external tools needed)
- `--remove-deps`: Remove unused dependencies (requires `--clean-deps` and `cargo-remove`)
- `-v, --verbose`: Verbose output
- `--json`: Output results as JSON

## ✨ Features

- ✅ **Cross-platform**: Works on Windows, macOS, and Linux
- ✅ **Robust workspace detection**: Uses cargo-metadata API for accurate workspace detection
- ✅ **Parallel processing**: Clean multiple projects simultaneously (configurable)
- ✅ **Dry-run mode**: Preview what would be cleaned without making changes
- ✅ **Exclude patterns**: Skip specific directories using glob patterns
- ✅ **Progress indication**: Real-time progress bars showing which projects are being cleaned
- ✅ **JSON output**: Machine-readable output for scripting and automation
- ✅ **Strong error handling**: Detailed error messages and graceful error recovery
- ✅ **High performance**: Parallel execution for fast cleaning of large project trees
- ✅ **Dependency cleaning**: Detect and optionally remove unused dependencies

## 🔧 Requirements

- Rust toolchain (for building)
- Cargo (Rust package manager)

### Optional Dependencies

For dependency removal (when using `--remove-deps`):
- **cargo-remove** (from cargo-edit): For removing unused dependencies
  ```bash
  cargo install cargo-edit
  ```

**Note:** Dependency detection is now built-in and doesn't require external tools! The tool parses `Cargo.toml` and searches your source code to find unused dependencies.

## 📦 Installation

### As a Cargo Plugin

```bash
# Clone or download this repository
git clone https://github.com/yingkitw/rclean.git
cd rclean

# Install as a cargo plugin
cargo install --path .

# Now you can use it as:
cargo rclean
```

**Note:** After installation, make sure `~/.cargo/bin` is in your PATH. The `cargo install` command will show you the installation path.

### As a Standalone Binary

```bash
# Clone or download this repository
git clone https://github.com/yingkitw/rclean.git
cd rclean

# Build
cargo build --release

# Use directly
./target/release/cargo-rclean
```

## 🎨 Example Output

### Standard Output

```
[INFO] Starting cargo clean from: "/path/to/projects"
[INFO] Searching for Cargo projects...
[INFO] Found 5 project(s)

⠋ Cleaning: project1
⠙ Cleaning: project2
⠹ Cleaning: project3
[████████████████████░░░░] 3/5 projects completed

✓ project1
⠋ Cleaning: project4
⠙ Cleaning: project5
[████████████████████████] 5/5 projects completed
All projects completed!

[INFO] === SUMMARY ===
[SUCCESS] Successfully cleaned: 5 project(s)
[SUCCESS] Total storage freed: 1.23 GB
[SUCCESS] All done!
```

### JSON Output

```json
{
  "total_projects": 5,
  "cleaned": 5,
  "failed": 0,
  "total_freed_bytes": 1321205760,
  "results": [
    {
      "path": "/path/to/project1",
      "success": true,
      "freed_bytes": 524288000,
      "error": null
    }
  ]
}
```

## 🏗️ Architecture

The implementation uses:
- **cargo-metadata**: For proper workspace detection using Cargo's own APIs
- **rayon**: For parallel processing
- **indicatif**: For progress bars with real-time project status
- **clap**: For command-line argument parsing
- **walkdir**: For efficient directory traversal

## 🐛 Error Handling

The tool handles:
- ✅ Missing workspace dependencies
- ✅ Malformed Cargo.toml files
- ✅ Failed cargo clean commands (with fallback to direct target removal)
- ✅ Permission errors
- ✅ Missing directories
- ✅ Network issues (when using cargo metadata)

Provides detailed error messages and continues processing other projects even if some fail.

## 📚 See Also

- [IMPROVEMENTS.md](./IMPROVEMENTS.md) - Detailed analysis and improvement recommendations

## 🤝 Contributing

Improvements and contributions are welcome! Please feel free to open issues or submit pull requests.

## 📄 License

Licensed under the Apache-2.0 license.

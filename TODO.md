# TODO

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
- [x] Improve error messages with context and suggestions

## Medium Priority

### Performance
- [ ] Optimize directory size calculation
  - [ ] Use faster method (consider `du` command on Unix)
  - [ ] Cache size results during discovery
  - [ ] Parallel size calculation
- [ ] Add progress indication for size calculation phase

### User Experience
- [ ] Add `--keep` flag to preserve certain build artifacts
- [ ] Add `--only-workspaces` flag to only clean workspace roots
- [ ] Add `--only-standalone` flag to only clean standalone projects
- [x] Add color support detection (auto-disable on non-TTY)

### Code Quality
- [ ] Add proper logging framework (tracing or log crate)
- [ ] Improve documentation with more examples
- [ ] Add integration tests
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

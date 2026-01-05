# TODO

## High Priority

### Code Organization
- [x] Create TODO.md
- [x] Create ARCHITECTURE.md
- [ ] Modularize code: Split main.rs into separate modules
  - [ ] `src/project.rs` - Project discovery and workspace detection
  - [ ] `src/cleaner.rs` - Cleaning logic
  - [ ] `src/output.rs` - Output formatting and display
  - [ ] `src/utils.rs` - Utility functions (format_bytes, get_directory_size)
  - [ ] `src/config.rs` - Configuration file support

### Testing
- [ ] Add unit tests for core functionality
  - [ ] Test project discovery
  - [ ] Test workspace detection
  - [ ] Test size calculation
  - [ ] Test byte formatting
  - [ ] Test exclude pattern matching

### Features
- [ ] Add configuration file support (`.rclean.toml`)
  - [ ] Default exclude patterns
  - [ ] Default job count
  - [ ] Default output format preferences
- [ ] Add interactive confirmation mode (`--interactive` flag)
- [ ] Add `--min-size` flag to only clean projects above threshold
- [ ] Improve error messages with context and suggestions

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
- [ ] Add color support detection (auto-disable on non-TTY)

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


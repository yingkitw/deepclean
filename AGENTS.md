# Agent Development Loop

This document defines the continuous improvement cycle for the **deepclean** crate — a fast, parallel cargo subcommand that reclaims disk space by cleaning Cargo `target/` directories, detecting unused dependencies, and clearing global toolchain caches (npm, Bun, cargo, pip, uv, Homebrew, HuggingFace, Playwright, Puppeteer, and more).

## Project Structure

```
.
├── src/
│   ├── main.rs         # CLI entry point (clap derive), cargo subcommand dispatch, orchestration
│   ├── project.rs      # Project discovery (walkdir), workspace detection (manifest scanning),
│   │                   # exclude-pattern filtering, workspace collapsing
│   ├── cleaner.rs      # Cleaning logic: cargo clean → rm -rf fallback, space-freed accounting
│   ├── deps.rs         # Unused dependency detection: Cargo.toml parsing + source scan,
│   │                   # optional removal via cargo-remove (cargo-edit)
│   ├── caches.rs       # Global cache discovery (16+ toolchains), parallel size computation,
│   │                   # safe/heavy risk tagging, interactive multi-select, rm -rf removal
│   ├── output.rs       # Human-readable and JSON output, progress bars (indicatif),
│   │                   # color-coded messages (colored), summaries
│   ├── config.rs       # .deepclean.toml loading, CLI-arg merging, validation
│   └── utils.rs        # Byte formatting, directory size calculation, size-string parsing
├── tests/
│   ├── caches.rs           # Cache discovery/tagging unit tests
│   ├── caches_integration.rs # End-to-end --caches tests
│   ├── dry_run.rs          # --dry-run safety tests (nothing deleted)
│   ├── filtering.rs        # Exclude patterns and --min-size tests
│   └── multi_project.rs    # Multi-project / workspace discovery tests
├── docs/
│   └── DEPENDENCY_CLEANING.md # Dependency-detection design notes
├── README.md           # User-facing docs, quick start, CLI reference
├── ARCHITECTURE.md     # Module relationships, data flow, design decisions
├── TODO.md             # Roadmap, backlog, done items
├── IMPROVEMENTS.md     # Rust-rewrite rationale and key design decisions
├── Cargo.toml          # Package metadata, deps (rayon, indicatif, clap, colored, glob,
│                       # walkdir, anyhow, serde, serde_json, toml), dev-dep (tempfile)
└── Cargo.lock
```

## The Loop

### 0. Consult MEMORY.md Before Starting
**CRITICAL**: Always begin each task by reading relevant sections of `MEMORY.md`. This prevents:
- Reinventing solutions to already-solved problems
- Repeating known mistakes and anti-patterns
- Missing established conventions for similar features
- Ignoring domain-specific pitfalls (deletion safety, TOCTOU races, symlink loops, permission errors, cross-platform path handling)

**How to consult MEMORY.md**:
1. Search for keywords related to your task (e.g., "workspace detection", "dry-run", "cache discovery", "rayon")
2. Read relevant pattern sections for context and proven approaches
3. Follow established conventions unless there's a clear reason to diverge
4. If MEMORY.md lacks relevant patterns, note this during the harvest step (Step 4)

### 1. Complete Remaining TODO Items
Pick the next highest-priority item from `TODO.md` (or `ARCHITECTURE.md` if the task is architectural). If no high-priority items remain, run the competitive intelligence step to seed new work. Implement with minimal, focused changes. Do not add speculative features.

### 2. Create Tests
For every new capability:
- Add inline `#[cfg(test)] mod tests` in the relevant source file
- Add integration tests in `tests/` using `tempfile` fixtures (never test against real user directories)
- Every deletion path must have a `--dry-run` test proving nothing is removed
- Add a JSON-output assertion if the feature emits machine-readable results
- Fix any failures before proceeding

### 3. Ensure `cargo test` Passes
Run the full test suite:
```bash
cargo test                  # all inline unit tests + integration tests
cargo clippy                # lint pass (warnings acceptable but noted)
```
Fix any failures before proceeding.

### 4. Harvest to MEMORY.md
After each completed feature, extract patterns and best practices:
- **Success patterns**: What worked well and should be repeated
- **Anti-patterns**: What to avoid in future implementations
- **Filesystem domain knowledge**: Deletion safety, TOCTOU races, symlink/hardlink handling, permission errors, hidden-directory conventions, cross-platform cache paths
- **Rust patterns**: deepclean-specific conventions for discovery, rayon parallelism, clap dispatch, and output
- **Testing patterns**: How to build tempfile fixtures, assert on JSON output, regression cases for workspace edge cases

Add these to `MEMORY.md` with clear categories and references to specific files/lines.

**Harvest Quality Checklist**:
- [ ] Added success patterns with code examples where useful
- [ ] Documented anti-patterns with what to avoid instead
- [ ] Included domain-specific gotchas and edge cases (deletion safety, TOCTOU, symlinks)
- [ ] Referenced specific files/lines for future lookup
- [ ] Used existing MEMORY.md categories or created new ones if needed
- [ ] Made entries searchable with relevant keywords
- [ ] Noted any gaps found in MEMORY.md during Step 0 consultation

### 5. Loop Back to Step 1
Return to `TODO.md` and pick the next item. Repeat until the backlog is clear.

### 6. Audit and Optimize
After each batch of features, perform a quality pass:
- **Maintainability**: Are functions small and well-named? Is the module structure logical?
- **Docs alignment**: README/ARCHITECTURE/TODO/MEMORY match the actual implementation (no drift). If docs and code disagree, update the DOC, not the test
- **Leanness**: Remove dead code, unused imports, and speculative abstractions
- **Wiring**: Ensure new CLI flags are added to the `Args` struct, README's CLI table, and JSON output structs
- **Small footprint**: Avoid unnecessary dependencies; prefer standard library or lightweight crates
- **Consistency**: Match existing code style and patterns (Rust 2024 edition, `anyhow` for errors, `serde` for serialization)
- **Safety audit**: Every new deletion path must (a) respect `--dry-run`, (b) respect `--exclude` patterns, (c) fail gracefully per-item without aborting the batch, (d) be covered by a test proving nothing outside the intended path is removed
- **CRAP score**: Evaluate change risk with the CRAP metric (Change Risk Anti-Patterns):
  `CRAP = comp² × (1 − coverage)³ + comp`, where `comp` is cyclomatic complexity and coverage is the fraction of the code exercised by tests.
  - CRAP ≈ `comp`: code is well-covered — safe to change
  - CRAP ≫ `comp`: high complexity + low coverage — refactor by **adding tests first**, then simplifying
  - Keep new code under CRAP ≈ 30 (roughly `comp` ≤ 6 with full coverage); treat CRAP > 100 as a red flag
  - Tools: `cargo-llvm-cov` for coverage, manual inspection (or `cargo-mutants`) for complexity
  - Note: CRAP identifies *where risk lives*; it does not mandate refactoring. For mature modules, always prefer adding coverage over rewriting (see Mature Module Policy below)

### 7. Score the Codebase and Improve the Weakest Area
After the audit pass (Step 6) — or when the backlog runs dry — quantify overall health, then fix the single weakest dimension instead of guessing what to work on next.

**Scorecard** — score each dimension 0–5, evidence-based (never from vibes):

| Dimension | Evidence to check |
|---|---|
| Correctness & coverage | `cargo test` green; coverage % via `cargo-llvm-cov`; every deletion path has a dry-run test |
| Deletion safety | All destructive paths gated behind dry-run/interactive flags; no path outside scope ever removed; symlink handling verified |
| Maintainability | CRAP scores (Step 6), dead code, clippy warning count, function sizes |
| Docs alignment | README/ARCHITECTURE/TODO/MEMORY match the actual implementation (no drift) |
| Wiring & ergonomics | New flags in `Args` + README CLI table + JSON output; help text accurate |
| Footprint | Dependency count/weight, no speculative abstractions |

Total is /30. Record the dated scorecard in `TODO.md` so progress is visible across iterations.

**Improve the weakness**:
1. Pick the single lowest-scoring dimension (tie-break: the one with the highest user impact)
2. Turn the fix into a concrete `TODO.md` item with a success criterion and test plan
3. Execute it through the normal loop (Steps 1–4) — tests and memory harvesting are not optional
4. Respect the Mature Module Policy: weak scores on mature modules are fixed by **adding tests/coverage first**, not rewrites
5. Re-score that dimension after the fix; a step that doesn't move the score didn't happen

### 8. Competitive Intelligence
Research similar disk-cleaning tools (e.g. `cargo-sweep`, `cargo-udeps`, `cargo-machete`, `npkill`, `pnpm store prune`, `brew cleanup`, `uv cache clean`, `pip cache purge`). Identify capabilities they have that this project lacks. Add the most valuable ones to the `TODO.md` brainstorming section. Prioritize features that provide clear competitive advantage.

#### Web Research with the `web2md` MCP Server
All web research in this loop (competitive intelligence, docs lookup, cache-path validation) must go through the **`web2md`** MCP server (`https://web2md.net/api/mcp`) instead of ad-hoc HTML fetching.

**Why**: web2md's `fetch` tool is deterministic (no LLM in the loop), strips boilerplate, and returns token-efficient Markdown plus structured metadata — much cheaper and more reliable for agents than raw HTML.

**Tool**: `fetch` — one tool, one required argument:

```
fetch(url, main_content=false, max_length=null, format=null, ...)
```

**Conventions**:
- **Docs/API pages** (docs.rs, crates.io, GitHub READMEs): `fetch(url, main_content=true)` to skip nav/footers
- **Long pages**: pass `max_length` (bytes); output is cleanly `[truncated]` instead of blowing the context window
- **Link discovery** (finding related tools/papers): `fetch(url, format="links")` returns a structured link list instead of prose
- **Tables** (cache-path references, benchmark comparisons): kept as GFM by default (`include_tables=true`) — prefer this over screenshots
- Use `only_with_metadata=true` when a page must have a `title` + `published_date` (e.g. citing a dated source)
- If web2md is unavailable in the current environment, fall back to any built-in fetch tool — but note it in `MEMORY.md` only if the fetched content produced a durable pattern

### 9. Update Documentation
Keep all project docs aligned with the current implementation. Root docs (required):

- **`README.md`**: Quick start, CLI usage, feature list, supported caches table
- **`ARCHITECTURE.md`**: Module relationships, data flow, design decisions
- **`TODO.md`**: Mark completed items, move them to Done, keep brainstorming current
- **`IMPROVEMENTS.md`**: Rust-rewrite rationale and key design decisions
- **`MEMORY.md`**: Harvested patterns, domain knowledge, technical conventions
- **`docs/DEPENDENCY_CLEANING.md`**: Dependency-detection design notes

Update **`AGENTS.md`** (this file) if the loop itself evolves.

## Memory System (MEMORY.md)

### Purpose
`MEMORY.md` is the institutional knowledge repository that accelerates development by:
- **Preventing wheel reinvention**: Reuse proven patterns instead of guessing
- **Domain knowledge preservation**: Capture filesystem and cache-discovery rules that may be counter-intuitive
- **Onboarding acceleration**: New contributors (human or AI) can understand patterns quickly
- **Quality consistency**: Ensure all features follow established conventions

### Structure
Organize `MEMORY.md` into these sections:

#### 1. Discovery & Workspace Patterns
- `walkdir` traversal conventions (hidden-dir pruning, exclude-pattern ordering, early `filter_entry`)
- Workspace detection via manifest-header scanning (`[workspace]` tables) and workspace collapsing
- Nested-workspace and standalone-project edge cases
- Deduplication of discovered project roots

#### 2. Cleaning & Deletion Safety Patterns
- `cargo clean` → `rm -rf` fallback ordering
- Dry-run gating: every destructive path checks the flag exactly once
- Per-item error isolation: one failure never aborts the batch
- Path validation and symlink handling before removal
- What must never be deleted (source files, `.git`, caches not in the registry)

#### 3. Cache Discovery Patterns
- Cache registry structure (id, category, path resolution, risk tag, note)
- Cross-platform cache paths (`$HOME`-based, XDG, `%LOCALAPPDATA%`)
- Parallel size computation and its I/O cost
- safe vs heavy risk tagging rationale
- Interactive multi-select parsing (numbers, ranges, `all`, `q`)

#### 4. Dependency Detection Patterns
- Cargo.toml parsing with `toml` (features disabled to keep footprint small)
- Source-scan heuristics (`use` statements, macro invocations, rename handling)
- False-positive/negative pitfalls (dev-deps, build-deps, feature-gated usage)

#### 5. Parallelism & Performance Patterns
- `rayon` work-stealing conventions and `--jobs` plumbed through
- Parallel size calculation vs sequential traversal trade-offs
- Progress reporting with `indicatif` from inside rayon closures
- Memory-streaming vs buffering results

#### 6. Output & CLI Patterns
- clap derive conventions, cargo-subcommand arg offset handling
- Human vs JSON output paths (serde struct design, field stability for CI consumers)
- `colored` TTY detection and non-TTY fallback
- Exit-code contract (non-zero if any operation failed)

#### 7. Testing Patterns
- `tempfile` fixture construction (fake projects, fake workspaces, fake caches)
- Dry-run regression tests for every deletion path
- JSON output assertions
- Workspace/exclude-pattern regression cases

### Maintaining MEMORY.md Quality

**When to Update MEMORY.md**:
- After completing any non-trivial feature or bug fix
- When discovering a new pattern or anti-pattern
- After resolving a tricky debugging session (especially deletion accidents, permission issues, or workspace misdetection)
- When establishing new conventions
- When a cache path or tool behavior change is confirmed

**How to Write Good MEMORY.md Entries**:
1. **Be specific**: Reference actual files and line numbers where patterns occur
2. **Show examples**: Include minimal code snippets demonstrating the pattern
3. **Explain why**: Don't just say what—explain the reasoning behind the pattern
4. **Link related patterns**: Cross-reference related entries with `[[PatternName]]`
5. **Keep it searchable**: Use keywords that future developers will search for
6. **Date entries**: Add dates so readers know how current the information is

**Signs MEMORY.md Needs Attention**:
- Same questions or patterns coming up repeatedly in development
- Developers (human or AI) solving problems that should be documented
- Frequent bugs in similar areas (indicates missing anti-pattern documentation)
- New contributors asking the same questions
- Deletion or discovery regressions in similar areas (e.g. repeated workspace misdetection)

**MEMORY.md Hygiene Routine** (run monthly):
- [ ] Review for outdated information (deprecated APIs, changed conventions)
- [ ] Consolidate redundant entries
- [ ] Add missing patterns from recent work
- [ ] Update cross-references if structure changed
- [ ] Verify all file references still exist
- [ ] Check for safety learnings that weren't captured
- [ ] Verify documented cache paths still resolve on supported platforms

## Principles

- **Safety over convenience**: Every destructive operation must be previewable (`--dry-run`), confirmable (`--interactive`), and reversible in practice (tools regenerate caches). Never optimize away a safety gate
- **CRAP score**: see Step 6 — keep change risk low via complexity + coverage
- **Surgical changes**: Touch only what you must; clean up only your own mess
- **Goal-driven**: Every change should have a verifiable success criterion
- **Test before ship**: No feature is complete until it has passing tests
- **Docs are code**: Documentation drift is a bug
- **Deletion fidelity**: Never delete anything outside the explicitly discovered scope; when in doubt, skip and report
- **Memory first**: Consult `MEMORY.md` before starting work—reuse proven patterns, avoid known pitfalls, follow established conventions. If MEMORY.md lacks relevant information, note the gap and fill it during harvest
- **Pattern harvesting**: After success, update `MEMORY.md` with patterns, anti-patterns, and learnings so others benefit from your experience
- **Memory hygiene**: Keep MEMORY.md current, searchable, and cross-referenced. It's only valuable if maintained

## Mature Module Policy

**Mature modules are effectively frozen.** Do not modify them as part of feature work, cleanup, or "drive-by" refactoring — even if the code looks improvable.

**A module is considered mature when ALL of the following hold:**
- It is stable and correct: no open bugs, no known safety issues
- It is well-covered by tests that have not needed changes for several iterations
- Its API is depended on by many other modules (e.g. `project.rs`, `utils.rs`, `output.rs`)
- It has documented, validated reference behavior (MEMORY.md entries, known-good test fixtures)

**Rules**:
1. **Extend, don't edit**: Add new capabilities in new modules or new functions; never reshape a mature module's public API to fit a new feature
2. **Wrap, don't rewrite**: Prefer adapters/new types over altering mature internals
3. **Bug fixes only**: A change to a mature module requires a concrete, reproducible bug or test failure — fix minimally and add a regression test; no opportunistic restructuring in the same commit
4. **Require explicit approval**: Any non-trivial change to a mature module must be justified in advance (risk, motivation, test plan) and confirmed by the user before starting
5. **Prefer coverage over rewriting**: If a mature module has high complexity/low coverage (high CRAP), respond by adding tests, not by rewriting — rewriting risk outweighs cosmetic gains
6. **Update docs, not code**: If behavior is confusing, document it in MEMORY.md rather than "fixing" it

## File Positioning and Value

### README.md
- **Value**: User-facing documentation and project overview
- **Audience**: Users, contributors, stakeholders
- **Position**: Entry point for anyone discovering the project
- **Focus**: Features, quick start, CLI usage, supported caches table, installation

### TODO.md
- **Value**: Feature roadmap and backlog management
- **Audience**: Development team (human and AI agents)
- **Position**: Development planning and prioritization
- **Focus**: What to build next, what's done, competitive intelligence, dated scorecards

### ARCHITECTURE.md
- **Value**: Module relationships, data flow, and design decisions
- **Audience**: Contributors maintaining or extending the crate
- **Position**: Structural reference for the codebase
- **Focus**: Module boundaries, data flow, error-handling and safety strategy

### IMPROVEMENTS.md
- **Value**: Rationale for the Rust rewrite and key design decisions
- **Audience**: Contributors evaluating past changes and crate choices
- **Position**: Historical record of why the project is architected this way
- **Focus**: Bash-implementation problems, Rust benefits, crate selection rationale

### MEMORY.md
- **Value**: Institutional knowledge and pattern library
- **Audience**: Development team (accelerates onboarding and consistency)
- **Position**: Development acceleration and quality consistency
- **Focus**: Proven patterns, domain knowledge, technical conventions
- **Update**: Must be updated after each completed feature to capture patterns and lessons learned

### AGENTS.md (this file)
- **Value**: Development process and workflow definition
- **Audience**: AI agents and human developers following the development loop
- **Position**: Process automation and continuous improvement
- **Focus**: How we work, the loop, memory system, principles
- **Update**: This file should be updated when the development loop itself evolves or when new process patterns emerge

## How These Files Work Together

1. **README.md** tells stakeholders what the project is and how to use it
2. **ARCHITECTURE.md** describes how the modules fit together
3. **TODO.md** tells developers what to build next (driven by competitive intelligence)
4. **AGENTS.md** tells agents how to work through the TODO items with quality and memory
5. **MEMORY.md** captures what we learned so we don't repeat mistakes and provides proven patterns to accelerate development

The loop reinforces these files:
Consult MEMORY.md → Complete TODO → Test → Harvest to MEMORY → Optimize → Score & improve weakest → Research → Update TODO

This creates a flywheel of continuous improvement with institutional knowledge preservation. **MEMORY.md is both the starting point (consult before work) and the destination (harvest after work), creating a virtuous cycle of learning and improvement.**

# Requirements: exlint

**Defined:** 2026-04-28
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## v1 Requirements

### Foundation

- [x] **FND-01**: The repository contains a Rust 2024 workspace with crates for CLI, config, diagnostics, filesystem, cache, core, SDK, Go adapter, TS adapter, graph helpers, rules, and plugin skeleton.
- [x] **FND-02**: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` are available as CI-friendly commands.
- [ ] **FND-03**: The README explains the goal, non-goals, quickstart, custom rule authoring, CI usage, and roadmap.

### CLI

- [x] **CLI-01**: User can run `polint init` to create `.polint.toml` and `.polint/rules`.
- [x] **CLI-02**: User can run `polint new-rule <language> <rule-name>` to scaffold a repo-local Rust rule.
- [x] **CLI-03**: User can run `polint check` with `--profile`, `--format human|json|sarif`, `--no-cache`, and `--fail-on warn|error|none`.
- [ ] **CLI-04**: User can run `polint explain <rule-id>`, `polint test-rules`, `polint profile-rules`, `polint graph imports --format dot`, and `polint graph function <name> --format dot`.
- [ ] **CLI-05**: CLI exit codes are `0` for success, `1` for diagnostics at or above fail threshold, and `2` for fatal tool/config/internal errors.

### Config and Files

- [x] **CFG-01**: `.polint.toml` supports include/exclude globs, profiles, rule paths, severity overrides, and language settings.
- [x] **CFG-02**: `polint check` runs a minimal default when config is missing and suggests `polint init`.
- [x] **FS-01**: File discovery respects `.gitignore`, include globs, exclude globs, and detects Go, TS, TSX, JS, and JSX files.
- [x] **FS-02**: File discovery output is deterministic.

### Core and Diagnostics

- [x] **CORE-01**: Core defines stable IDs and models for files, spans, functions, imports, branch obligations, tests, coverage placeholders, and analysis database.
- [x] **CORE-02**: Core runs rules through a registry, honors capability declarations, deduplicates diagnostics, catches rule panics where practical, and sorts diagnostics deterministically.
- [x] **DIAG-01**: Diagnostics support severity, labels, suggestions/fixes, evidence, stable fingerprints, and human output.
- [x] **DIAG-02**: Diagnostics render as JSON.
- [ ] **DIAG-03**: Diagnostics render as SARIF-like output for CI.

### Go Analysis

- [x] **GO-01**: Go adapter parses Go files with tree-sitter-go and reports parser errors as diagnostics.
- [x] **GO-02**: Go adapter extracts package names, imports, functions, methods, test functions, subtests, and table-test evidence where practical.
- [x] **GO-03**: Go adapter extracts branch obligations for `if`, `switch`, `case/default`, `for`, `range`, and basic error-path conditions.
- [x] **GO-04**: Go adapter computes basic cyclomatic complexity and import graph facts.

### TypeScript Analysis

- [x] **TS-01**: TS adapter parses `.ts`, `.tsx`, `.js`, and `.jsx` files with Oxc and reports parser errors as diagnostics.
- [x] **TS-02**: TS adapter extracts imports/exports, functions, classes, React-ish component functions, JSX attributes, and string literals.
- [x] **TS-03**: TS adapter computes basic cyclomatic complexity and import graph facts.

### SDK and Rules

- [x] **SDK-01**: `polint-sdk` exposes a documented `Rule` trait, `RuleMeta`, `Capabilities`, `RuleCtx`, and prelude.
- [x] **SDK-02**: `RuleCtx` exposes high-level queries for files, functions, imports, graphs, branch obligations, Go tests, TS components, string literals, JSX attributes, and diagnostic reporting.
- [x] **RULE-01**: Built-in example rule `examples/go-cyclomatic-complexity` works and is configurable.
- [x] **RULE-02**: Built-in example rule `examples/ts-cyclomatic-complexity` works and is configurable.
- [x] **RULE-03**: Built-in example rule `examples/go-import-boundaries` works and is configurable.
- [x] **RULE-04**: Built-in example rule `examples/ts-no-raw-colors` detects raw color literals with allow-list support.
- [x] **RULE-05**: Built-in example rule `examples/go-branch-obligations` reports missing nearby test evidence using honest heuristic wording.
- [x] **RULE-06**: Built-in example rule `examples/go-test-suite-size` computes a weighted maintainability score.
- [x] **RULE-07**: Built-in example rule `examples/go-assertion-after-action` warns when Go tests appear to lack assertions.
- [x] **RULE-08**: Built-in example rule `examples/config-query-no-literal` denies configured string/regex literals across supported languages where possible.

### Cache and Performance

- [x] **PERF-01**: Cache hashes file contents, config, and rules, stores parse/fact metadata under `.polint/cache`, and can be disabled with `--no-cache`.
- [x] **PERF-02**: Parsing and rule execution run in parallel where safe while output remains deterministic.
- [x] **PERF-03**: `polint profile-rules` reports per-rule timing.

### Plugins

- [ ] **PLUG-01**: `polint-plugin` contains WIT interface files and Wasmtime loading skeleton.
- [ ] **PLUG-02**: Plugin docs explain that repo-local Wasm rules are experimental and should query host facts by stable IDs.

### Testing

- [x] **TEST-01**: Unit tests cover config parsing, rule discovery, new-rule generation, glob matching, file discovery, spans, diagnostic sorting, Go extraction, TS extraction, rule logic, and cache behavior.
- [x] **TEST-02**: Integration tests cover init, new-rule, check on clean/failing Go and TS fixtures, JSON output, profiles, exit codes, and cache on/off behavior.
- [ ] **TEST-03**: Snapshot tests cover human, JSON, and SARIF-like diagnostics. Phase 6 completed representative rule-family human and JSON snapshots; SARIF-like and broader CI snapshots remain scheduled for Phase 8.
- [x] **TEST-04**: Property tests cover span roundtrips, diagnostic sorting determinism, and file discovery exclusions where useful.

## v2 Requirements

### Future Semantics

- **SEM-01**: Add optional exact Go type information through `go/packages` or `go/analysis`.
- **SEM-02**: Add dynamic branch coverage instrumentation and exact coverage facts.
- **SEM-03**: Compile repo-local Rust rules to Wasm automatically and cache artifacts by source hash, SDK version, and target triple.
- **SEM-04**: Add more language adapters after Go and TS/JS stabilize.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Comprehensive built-in rule pack | The framework exists to make custom repo-local policy easy. |
| Replacement for existing linters/formatters | Existing tools remain best for generic language/style linting. |
| Full Go type checking in v1 | Syntax-level facts are enough for first useful rules. |
| Exact dynamic branch coverage in v1 | Static obligations and heuristic test evidence provide useful first value. |
| Full repo-local Wasm compilation in v1 | SDK, scaffolding, and plugin skeleton are the safer first deliverable. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| FND-01 | Phase 1 | Complete |
| FND-02 | Phase 1 | Complete |
| FND-03 | Phase 10 | Pending |
| CLI-01 | Phase 2 | Complete |
| CLI-02 | Phase 2 | Complete |
| CLI-03 | Phase 2 | Complete |
| CLI-04 | Phase 8 | Pending |
| CLI-05 | Phase 8 | Pending |
| CFG-01 | Phase 2 | Complete |
| CFG-02 | Phase 2 | Complete |
| FS-01 | Phase 2 | Complete |
| FS-02 | Phase 3 | Complete |
| CORE-01 | Phase 3 | Complete |
| CORE-02 | Phase 3 | Complete |
| DIAG-01 | Phase 3 | Complete |
| DIAG-02 | Phase 2 | Complete |
| DIAG-03 | Phase 8 | Pending |
| GO-01 | Phase 4 | Complete |
| GO-02 | Phase 4 | Complete |
| GO-03 | Phase 4 | Complete |
| GO-04 | Phase 4 | Complete |
| TS-01 | Phase 5 | Complete |
| TS-02 | Phase 5 | Complete |
| TS-03 | Phase 5 | Complete |
| SDK-01 | Phase 6 | Complete |
| SDK-02 | Phase 6 | Complete |
| RULE-01 | Phase 6 | Complete |
| RULE-02 | Phase 6 | Complete |
| RULE-03 | Phase 6 | Complete |
| RULE-04 | Phase 6 | Complete |
| RULE-05 | Phase 6 | Complete |
| RULE-06 | Phase 6 | Complete |
| RULE-07 | Phase 6 | Complete |
| RULE-08 | Phase 6 | Complete |
| PERF-01 | Phase 7 | Complete |
| PERF-02 | Phase 7 | Complete |
| PERF-03 | Phase 7 | Complete |
| PLUG-01 | Phase 9 | Pending |
| PLUG-02 | Phase 9 | Pending |
| TEST-01 | Phase 1-9 | Complete |
| TEST-02 | Phase 2-8 | Complete |
| TEST-03 | Phase 3-8 | In Progress - Phase 3 human and JSON diagnostic snapshots verified; SARIF-like and broader rule snapshots remain scheduled for later phases |
| TEST-04 | Phase 3-7 | In Progress - Phase 3 span, diagnostic sorting, and discovery include/exclude property tests verified; cache/performance property scope remains scheduled |

**Coverage:**
- v1 requirements: 43 total
- Mapped to phases: 43
- Unmapped: 0

---
*Requirements defined: 2026-04-28*
*Last updated: 2026-05-01 after Phase 6 verification*

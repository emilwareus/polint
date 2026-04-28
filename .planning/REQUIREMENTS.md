# Requirements: exlint

**Defined:** 2026-04-28
**Core Value:** Make it easy to express a repo-specific engineering policy as a small rule and run it locally, in CI, and with AI coding agents.

## v1 Requirements

### Foundation

- [x] **FND-01**: The repository contains a Rust 2024 workspace with crates for CLI, config, diagnostics, filesystem, cache, core, SDK, Go adapter, TS adapter, graph helpers, rules, and plugin skeleton.
- [x] **FND-02**: `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` are available as CI-friendly commands.
- [ ] **FND-03**: The README explains the goal, non-goals, quickstart, custom rule authoring, CI usage, and roadmap.

### CLI

- [ ] **CLI-01**: User can run `polint init` to create `.polint.toml` and `.polint/rules`.
- [ ] **CLI-02**: User can run `polint new-rule <language> <rule-name>` to scaffold a repo-local Rust rule.
- [ ] **CLI-03**: User can run `polint check` with `--profile`, `--format human|json|sarif`, `--no-cache`, and `--fail-on warn|error|none`.
- [ ] **CLI-04**: User can run `polint explain <rule-id>`, `polint test-rules`, `polint profile-rules`, `polint graph imports --format dot`, and `polint graph function <name> --format dot`.
- [ ] **CLI-05**: CLI exit codes are `0` for success, `1` for diagnostics at or above fail threshold, and `2` for fatal tool/config/internal errors.

### Config and Files

- [ ] **CFG-01**: `.polint.toml` supports include/exclude globs, profiles, rule paths, severity overrides, and language settings.
- [ ] **CFG-02**: `polint check` runs a minimal default when config is missing and suggests `polint init`.
- [ ] **FS-01**: File discovery respects `.gitignore`, include globs, exclude globs, and detects Go, TS, TSX, JS, and JSX files.
- [ ] **FS-02**: File discovery output is deterministic.

### Core and Diagnostics

- [ ] **CORE-01**: Core defines stable IDs and models for files, spans, functions, imports, branch obligations, tests, coverage placeholders, and analysis database.
- [ ] **CORE-02**: Core runs rules through a registry, honors capability declarations, deduplicates diagnostics, catches rule panics where practical, and sorts diagnostics deterministically.
- [ ] **DIAG-01**: Diagnostics support severity, labels, suggestions/fixes, evidence, stable fingerprints, and human output.
- [ ] **DIAG-02**: Diagnostics render as JSON.
- [ ] **DIAG-03**: Diagnostics render as SARIF-like output for CI.

### Go Analysis

- [ ] **GO-01**: Go adapter parses Go files with tree-sitter-go and reports parser errors as diagnostics.
- [ ] **GO-02**: Go adapter extracts package names, imports, functions, methods, test functions, subtests, and table-test evidence where practical.
- [ ] **GO-03**: Go adapter extracts branch obligations for `if`, `switch`, `case/default`, `for`, `range`, and basic error-path conditions.
- [ ] **GO-04**: Go adapter computes basic cyclomatic complexity and import graph facts.

### TypeScript Analysis

- [ ] **TS-01**: TS adapter parses `.ts`, `.tsx`, `.js`, and `.jsx` files with Oxc and reports parser errors as diagnostics.
- [ ] **TS-02**: TS adapter extracts imports/exports, functions, classes, React-ish component functions, JSX attributes, and string literals.
- [ ] **TS-03**: TS adapter computes basic cyclomatic complexity and import graph facts.

### SDK and Rules

- [ ] **SDK-01**: `polint-sdk` exposes a documented `Rule` trait, `RuleMeta`, `Capabilities`, `RuleCtx`, and prelude.
- [ ] **SDK-02**: `RuleCtx` exposes high-level queries for files, functions, imports, graphs, branch obligations, Go tests, TS components, string literals, JSX attributes, and diagnostic reporting.
- [ ] **RULE-01**: Built-in example rule `examples/go-cyclomatic-complexity` works and is configurable.
- [ ] **RULE-02**: Built-in example rule `examples/ts-cyclomatic-complexity` works and is configurable.
- [ ] **RULE-03**: Built-in example rule `examples/go-import-boundaries` works and is configurable.
- [ ] **RULE-04**: Built-in example rule `examples/ts-no-raw-colors` detects raw color literals with allow-list support.
- [ ] **RULE-05**: Built-in example rule `examples/go-branch-obligations` reports missing nearby test evidence using honest heuristic wording.
- [ ] **RULE-06**: Built-in example rule `examples/go-test-suite-size` computes a weighted maintainability score.
- [ ] **RULE-07**: Built-in example rule `examples/go-assertion-after-action` warns when Go tests appear to lack assertions.
- [ ] **RULE-08**: Built-in example rule `examples/config-query-no-literal` denies configured string/regex literals across supported languages where possible.

### Cache and Performance

- [ ] **PERF-01**: Cache hashes file contents, config, and rules, stores parse/fact metadata under `.polint/cache`, and can be disabled with `--no-cache`.
- [ ] **PERF-02**: Parsing and rule execution run in parallel where safe while output remains deterministic.
- [ ] **PERF-03**: `polint profile-rules` reports per-rule timing.

### Plugins

- [ ] **PLUG-01**: `polint-plugin` contains WIT interface files and Wasmtime loading skeleton.
- [ ] **PLUG-02**: Plugin docs explain that repo-local Wasm rules are experimental and should query host facts by stable IDs.

### Testing

- [ ] **TEST-01**: Unit tests cover config parsing, rule discovery, new-rule generation, glob matching, file discovery, spans, diagnostic sorting, Go extraction, TS extraction, rule logic, and cache behavior.
- [ ] **TEST-02**: Integration tests cover init, new-rule, check on clean/failing Go and TS fixtures, JSON output, profiles, exit codes, and cache on/off behavior.
- [ ] **TEST-03**: Snapshot tests cover human, JSON, and SARIF-like diagnostics.
- [ ] **TEST-04**: Property tests cover span roundtrips, diagnostic sorting determinism, and file discovery exclusions where useful.

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
| CLI-01 | Phase 2 | Pending |
| CLI-02 | Phase 2 | Pending |
| CLI-03 | Phase 2 | Pending |
| CLI-04 | Phase 8 | Pending |
| CLI-05 | Phase 8 | Pending |
| CFG-01 | Phase 2 | Pending |
| CFG-02 | Phase 2 | Pending |
| FS-01 | Phase 2 | Pending |
| FS-02 | Phase 3 | Pending |
| CORE-01 | Phase 3 | Pending |
| CORE-02 | Phase 3 | Pending |
| DIAG-01 | Phase 3 | Pending |
| DIAG-02 | Phase 2 | Pending |
| DIAG-03 | Phase 8 | Pending |
| GO-01 | Phase 4 | Pending |
| GO-02 | Phase 4 | Pending |
| GO-03 | Phase 4 | Pending |
| GO-04 | Phase 4 | Pending |
| TS-01 | Phase 5 | Pending |
| TS-02 | Phase 5 | Pending |
| TS-03 | Phase 5 | Pending |
| SDK-01 | Phase 6 | Pending |
| SDK-02 | Phase 6 | Pending |
| RULE-01 | Phase 6 | Pending |
| RULE-02 | Phase 6 | Pending |
| RULE-03 | Phase 6 | Pending |
| RULE-04 | Phase 6 | Pending |
| RULE-05 | Phase 6 | Pending |
| RULE-06 | Phase 6 | Pending |
| RULE-07 | Phase 6 | Pending |
| RULE-08 | Phase 6 | Pending |
| PERF-01 | Phase 7 | Pending |
| PERF-02 | Phase 7 | Pending |
| PERF-03 | Phase 7 | Pending |
| PLUG-01 | Phase 9 | Pending |
| PLUG-02 | Phase 9 | Pending |
| TEST-01 | Phase 1-9 | In Progress - Phase 1 workspace tests verified; broader coverage remains scheduled in later phases |
| TEST-02 | Phase 2-8 | Pending |
| TEST-03 | Phase 3-8 | Pending |
| TEST-04 | Phase 3-7 | Pending |

**Coverage:**
- v1 requirements: 43 total
- Mapped to phases: 43
- Unmapped: 0

---
*Requirements defined: 2026-04-28*
*Last updated: 2026-04-28 after roadmap creation*

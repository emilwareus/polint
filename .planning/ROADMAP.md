# Roadmap: exlint v1

**Created:** 2026-04-28
**Mode:** YOLO
**Granularity:** Fine

## Summary

| # | Phase | Goal | Requirements | Success Criteria |
|---|-------|------|--------------|------------------|
| 1 | Workspace Foundation | Create a compiling Rust workspace and shared project skeleton. | FND-01, FND-02, TEST-01 | 4 |
| 2 | CLI, Config, and Discovery | Make `polint init`, `new-rule`, and `check` work on discovered files. | CLI-01, CLI-02, CLI-03, CFG-01, CFG-02, FS-01, DIAG-02, TEST-02 | 5 |
| 3 | Core Facts and Diagnostics | Add stable IDs, analysis DB, rule runner, deterministic diagnostics, and SDK-facing primitives. | FS-02, CORE-01, CORE-02, DIAG-01, TEST-01, TEST-03, TEST-04 | 5 |
| 4 | Go Adapter | Extract Go facts and implement Go-specific analysis foundations. | GO-01, GO-02, GO-03, GO-04, TEST-01, TEST-02 | 5 |
| 5 | TypeScript Adapter | Extract TS/JS facts and implement TS-specific analysis foundations. | TS-01, TS-02, TS-03, TEST-01, TEST-02 | 5 |
| 6 | SDK and Example Rules | Make custom rule authoring ergonomic and dogfood the SDK through example rules. | SDK-01, SDK-02, RULE-01..RULE-08, TEST-01, TEST-03 | 5 |
| 7 | Cache and Performance | Add safe caching, deterministic parallelism, and profiling. | PERF-01, PERF-02, PERF-03, TEST-01, TEST-04 | 4 |
| 8 | CI Output and Graph Commands | Add SARIF-like output, exit semantics, explain/test/profile/graph commands, and CI fixtures. | CLI-04, CLI-05, DIAG-03, TEST-02, TEST-03 | 5 |
| 9 | Plugin Skeleton | Add WIT files, Wasmtime host skeleton, and experimental plugin docs. | PLUG-01, PLUG-02 | 3 |
| 10 | Docs, Examples, and Release Hardening | 3/4 | In Progress|  |

## Phase Details

### Phase 1: Workspace Foundation

**Goal:** Create a Rust workspace that compiles and establishes the crate boundaries needed for all later phases.

**Requirements:** FND-01, FND-02, TEST-01

**Plans:** 1 plan

Plans:
- [x] 01-01-PLAN.md - Verify and reconcile the existing workspace foundation without recreating it.

**Success criteria:**
1. `Cargo.toml` defines a Rust 2024 workspace with all requested crates.
2. Every crate has a compiling minimal public API and internal tests where useful.
3. `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` run successfully.
4. The dependency versions reflect the `cargo search` research results.

### Phase 2: CLI, Config, and Discovery

**Goal:** Provide the first usable user loop: initialize config, scaffold a rule, discover files, run check, and render human/JSON output.

**Requirements:** CLI-01, CLI-02, CLI-03, CFG-01, CFG-02, FS-01, DIAG-02, TEST-02

**Plans:** 2/2 plans complete

Plans:
- [x] 02-01-PLAN.md - Add focused tests and narrow fixes for the existing CLI/config/discovery loop.
- [x] 02-02-PLAN.md - Run full verification and reconcile Phase 2 status records.

**Success criteria:**
1. `polint init` creates `.polint.toml` and `.polint/rules/`.
2. `polint new-rule go name` and `polint new-rule ts name` create compilable-looking rule skeletons.
3. `polint check` loads config or uses a minimal default if config is missing.
4. File discovery respects `.gitignore`, include/exclude globs, and supported language extensions.
5. Integration tests cover init, new-rule, default check, profiles, and JSON output.

### Phase 3: Core Facts and Diagnostics

**Goal:** Build the stable fact model, analysis database, diagnostic model, registry, runner, and deterministic output behavior.

**Requirements:** FS-02, CORE-01, CORE-02, DIAG-01, TEST-01, TEST-03, TEST-04

**Plans:** 3/3 plans complete

Plans:
- [x] 03-01-PLAN.md - Harden core fact models, spans, rule registry, and deterministic runner behavior.
- [x] 03-02-PLAN.md - Harden diagnostic identity, full contract fields, human rendering, JSON snapshots, sort, and dedupe.
- [x] 03-03-PLAN.md - Prove deterministic discovery and repeated CLI JSON output, then reconcile Phase 3 status.

**Success criteria:**
1. Core IDs, source files, spans, functions, imports, branch obligations, tests, coverage placeholders, and analysis DB compile and are tested.
2. Rule registry and runner execute rules with capability declarations and panic containment where practical.
3. Diagnostics support labels, evidence, fixes, stable fingerprints, human rendering, dedupe, and deterministic sorting.
4. Snapshot tests cover human and JSON diagnostic output.
5. Property/unit tests verify span conversion and ordering invariants.

### Phase 4: Go Adapter

**Goal:** Extract useful Go facts with tree-sitter-go and enable Go example rules to be built on stable facts.

**Requirements:** GO-01, GO-02, GO-03, GO-04, TEST-01, TEST-02

**Plans:** 4/4 plans complete

Plans:
- [x] 04-01-PLAN.md - Add parser-backed Go foundation, parser diagnostics, and core package facts.
- [x] 04-02-PLAN.md - Extract Go imports, declarations, calls, test evidence, and complexity from tree-sitter.
- [x] 04-03-PLAN.md - Extract Go branch obligations and conservative error-path heuristics.
- [x] 04-04-PLAN.md - Prove Go facts through fixtures, CLI integration tests, and workspace verification.

**Success criteria:**
1. Go files parse through tree-sitter-go and parser errors become diagnostics.
2. Package names, imports, functions, methods, tests, subtests, and practical table-test evidence are extracted.
3. Branch obligations are extracted for if/switch/case/default/loop constructs with error-path marking where practical.
4. Basic Go cyclomatic complexity and import graph facts are computed.
5. Fixtures and tests cover clean and failing Go cases.

### Phase 5: TypeScript Adapter

**Goal:** Extract useful TS/JS facts with Oxc and enable TS example rules to be built on stable facts.

**Requirements:** TS-01, TS-02, TS-03, TEST-01, TEST-02

**Plans:** 4/4 plans complete

Plans:
- [x] 05-01-PLAN.md - Add the Oxc parser foundation, borrowed-source parsing, and `parser/ts` diagnostics.
- [x] 05-02-PLAN.md - Extract imports, exports, declarations, classes, components, and calls from Oxc AST nodes.
- [x] 05-03-PLAN.md - Extract string/JSX facts, compute TS complexity, and prove import graph facts.
- [x] 05-04-PLAN.md - Expand TS fixtures, add CLI integration tests, and run full Phase 5 verification.

**Success criteria:**
1. TS, TSX, JS, and JSX files parse through Oxc and parser errors become diagnostics.
2. Imports/exports, functions, classes, component-like functions, JSX attributes, and string literals are extracted.
3. Basic TS/JS cyclomatic complexity and import graph facts are computed.
4. Raw color fixture data is available for the TS rule.
5. Fixtures and tests cover clean and failing TS cases.

### Phase 6: SDK and Example Rules

**Goal:** Make rule authoring pleasant and prove the SDK by implementing the requested example rules.

**Requirements:** SDK-01, SDK-02, RULE-01, RULE-02, RULE-03, RULE-04, RULE-05, RULE-06, RULE-07, RULE-08, TEST-01, TEST-03

**Plans:** 6/6 plans complete

Plans:
- [x] 06-01-PLAN.md - Make the public SDK entry point pleasant and complete for Phase 6 rule authors.
- [x] 06-02-PLAN.md - Add the narrow literal and config foundations needed by the Phase 6 example rules.
- [x] 06-03-PLAN.md - Harden the SDK-facing non-heuristic example rules.
- [x] 06-04-PLAN.md - Harden the Go heuristic example rules without overclaiming semantic proof.
- [x] 06-05-PLAN.md - Prove the requested example rules through CLI integration fixtures.
- [x] 06-06-PLAN.md - Add representative diagnostic snapshots and complete Phase 6 verification.

**Success criteria:**
1. `polint-sdk` exposes a documented `Rule` trait, metadata, capabilities, prelude, and `RuleCtx` helpers.
2. Built-in example rules use the SDK rather than private core shortcuts.
3. All requested example rules emit deterministic, useful diagnostics against fixtures.
4. Heuristic rules use honest wording and include evidence/help.
5. Snapshot tests cover representative diagnostics for each rule family.

### Phase 7: Cache and Performance

**Goal:** Add a safe, disableable cache and deterministic parallel execution.

**Requirements:** PERF-01, PERF-02, PERF-03, TEST-01, TEST-04

**Plans:** 4/4 plans complete

Plans:
- [x] 07-01-PLAN.md - Build the cache foundation and invalidation contract.
- [x] 07-02-PLAN.md - Cache parser/fact extraction outputs without storing source text.
- [x] 07-03-PLAN.md - Add deterministic Rayon-backed file loading, adapter analysis, and rule execution proof.
- [x] 07-04-PLAN.md - Close profiling, no-cache, and repeated-run verification.

**Success criteria:**
1. Cache keys include file hash, config hash, rule hash, and cache/schema version.
2. `--no-cache` fully bypasses cache reads/writes.
3. Parsing and rule execution use Rayon where safe.
4. `polint profile-rules` reports per-rule timings.
5. Tests verify cache on/off behavior and deterministic output across repeated runs.

### Phase 8: CI Output and Graph Commands

**Goal:** Finish CI-facing behavior, graph export commands, and remaining CLI command surface.

**Requirements:** CLI-04, CLI-05, DIAG-03, TEST-02, TEST-03

**Plans:** 4/4 plans complete

Plans:
- [x] 08-01-PLAN.md - Lock CLI command contracts and fail-threshold exit codes.
- [x] 08-02-PLAN.md - Cover SARIF-like CI output fields and snapshots.
- [x] 08-03-PLAN.md - Cover deterministic DOT import and function graph commands.
- [x] 08-04-PLAN.md - Run targeted/full verification and record final evidence.

**Success criteria:**
1. `polint explain`, `test-rules`, `profile-rules`, `graph imports`, and `graph function` are implemented.
2. SARIF-like output includes rule IDs, locations, messages, severities, and fingerprints.
3. `--fail-on warn|error|none` produces the required exit codes.
4. DOT graph export works for import graphs and available function graphs.
5. Integration and snapshot tests cover CI output and exit code behavior.

### Phase 9: Plugin Skeleton

**Goal:** Add a clean experimental Wasm plugin boundary without blocking v1 usefulness.

**Requirements:** PLUG-01, PLUG-02

**Success criteria:**
1. `polint-plugin` contains WIT interface files for rule metadata, capabilities, diagnostics, and host fact queries.
2. Wasmtime host loading skeleton validates plugin paths and reports structured errors.
3. Docs clearly mark Wasm repo-local rules as experimental and describe the intended stable-ID host API.

### Phase 10: Docs, Examples, and Release Hardening

**Goal:** Make the project understandable, testable, and ready for first release.

**Requirements:** FND-03, TEST-01, TEST-02, TEST-03, TEST-04

**Success criteria:**
1. README covers what it is, why it exists, quickstart, config, SDK example, capabilities, CI, and roadmap.
2. `examples/` contains basic, Go custom rule, TS custom rule, Go branch obligation, and TS design token examples.
3. Test fixtures cover Go, TS, and mixed repositories.
4. Full verification commands pass.
5. Remaining TODOs are documented honestly and do not fake missing functionality.

## Coverage Validation

- v1 requirements: 43
- Requirements mapped: 43
- Unmapped requirements: 0

---
*Roadmap created: 2026-04-28; last updated after Phase 4 verification*

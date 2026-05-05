# Phase 8: CI Output and Graph Commands - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 8 finishes CI-facing behavior and the remaining CLI command surface for the existing `polint` binary. It hardens `explain`, `test-rules`, `profile-rules`, `graph imports`, `graph function`, SARIF-like output, fail-threshold exit codes, DOT graph output, and the integration/snapshot proof around those behaviors. This phase should not add new language semantics, full TypeScript/Node module resolution, or dynamic repo-local Rust rule loading.

</domain>

<decisions>
## Implementation Decisions

### Command Surface Contracts

- **D-01:** `[auto]` Treat the currently implemented command names as the v1 Phase 8 surface: `polint explain <rule-id>`, `polint test-rules`, `polint profile-rules`, `polint graph imports --format dot`, and `polint graph function <name> --format dot`.
- **D-02:** `[auto]` Harden behavior around existing command implementations rather than redesigning the CLI. `test-rules` may remain a fixture/profile-oriented harness over the current analysis path; it should not claim dynamic repo-local Rust rule compilation.
- **D-03:** `[auto]` `explain` should be useful and deterministic for built-in example rule IDs. Unknown custom rule IDs should get a clear message and non-fatal behavior unless a stricter contract is already established by tests.
- **D-04:** `[auto]` `profile-rules` should keep the Phase 7 tab-separated timing rows and variable-duration honesty. Phase 8 may add tests/exit-code hardening, but should not add benchmark loops or fixed speedup claims.

### SARIF-Like CI Output

- **D-05:** `[auto]` Keep output explicitly SARIF-like for v1. The renderer should include the fields required by the roadmap and requirements: rule IDs, locations, messages, severities mapped to SARIF levels, and fingerprints.
- **D-06:** `[auto]` Prefer stable, machine-parseable JSON structure over broad SARIF completeness. Add fields only where they improve CI usability or testability without pretending to be a fully certified SARIF implementation.
- **D-07:** `[auto]` SARIF output must stay stdout-only and valid JSON. Human guidance such as missing-config suggestions must not corrupt JSON or SARIF streams.
- **D-08:** `[auto]` Snapshot SARIF-like output at the diagnostics-renderer level and add CLI integration assertions for parseability and key fields.

### Exit Code Semantics

- **D-09:** `[auto]` Close `CLI-05` exactly: exit code `0` for successful runs without diagnostics at or above the selected threshold, `1` for diagnostics at or above `--fail-on`, and `2` for fatal tool/config/internal command errors.
- **D-10:** `[auto]` Apply fail-threshold semantics consistently to `check`, `test-rules`, and `profile-rules` where those commands run diagnostics. Graph and explain commands should return `0` for successful command execution and `2` for fatal errors.
- **D-11:** `[auto]` Keep `--fail-on none` as an explicit CI escape hatch that reports diagnostics while returning `0`.
- **D-12:** `[auto]` Add integration tests that assert exit codes directly for warn/error/none thresholds and fatal command paths.

### Graph Output Contracts

- **D-13:** `[auto]` DOT output is the v1 graph format for Phase 8. Do not add JSON, Mermaid, image export, or graph layout responsibilities in this phase.
- **D-14:** `[auto]` `graph imports` should produce deterministic DOT for syntactic import facts from Go and TS/JS adapters. It does not need production-grade Node/TypeScript module resolution.
- **D-15:** `[auto]` `graph function <name>` should produce deterministic DOT for available call facts around the requested function name. If no facts match, output should remain valid DOT and tests should define the empty graph behavior.
- **D-16:** `[auto]` Use sorted/deterministic graph construction and stable labels so repeated graph command runs are byte-identical for the same fixture.

### CI and Test Proof

- **D-17:** `[auto]` Reuse the existing `assert_cmd`, `tempfile`, parsed JSON, and snapshot patterns. Prefer structured assertions over substring-only checks.
- **D-18:** `[auto]` Add representative SARIF-like snapshots and CLI integration tests for command surface, exit codes, and DOT graph output.
- **D-19:** `[auto]` Keep built-in rules as SDK examples in tests. Do not broaden Phase 8 into a comprehensive lint pack.
- **D-20:** `[auto]` Full workspace verification for this phase must include `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.

### the agent's Discretion

- The exact SARIF-like JSON field ordering and whether to include optional SARIF metadata beyond the roadmap-required fields.
- The exact wording for `explain`, `test-rules`, unknown rule IDs, and empty graph output, as long as it is deterministic and tested.
- Whether graph tests live in `polint-graph` unit tests, CLI integration tests, snapshots, or a combination, based on where failures are most useful.
- How to split Phase 8 plans, provided each plan has narrow acceptance criteria and source/test changes are committed atomically.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope and Requirements

- `.planning/ROADMAP.md` — Phase 8 goal, requirements, and success criteria.
- `.planning/REQUIREMENTS.md` — `CLI-04`, `CLI-05`, `DIAG-03`, `TEST-02`, and `TEST-03`.
- `.planning/PROJECT.md` — Project constraints, non-goals, and current state after Phase 7.

### Prior Decisions To Carry Forward

- `.planning/phases/02-cli-config-and-discovery/02-CONTEXT.md` — Phase 2 CLI boundaries, JSON stdout rule, and deferral of CLI-04/CLI-05 hardening to Phase 8.
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` — Diagnostic contract, deterministic sorting/deduping, and SARIF-like hardening deferral.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` — Go import graph facts are syntax-level and production DOT hardening belongs to Phase 8.
- `.planning/phases/05-typescript-adapter/05-CONTEXT.md` — TS/JS import graph facts are syntactic; production graph commands and full Node/TS resolution are Phase 8 or later.
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` — Built-in rules remain SDK dogfood examples, not a comprehensive rule pack.
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` — `profile-rules` timing rows are deterministic in shape but duration values are variable; broader output hardening belongs to Phase 8.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/polint-cli/src/main.rs` already defines `Explain`, `TestRules`, `ProfileRules`, `Graph::Imports`, `Graph::Function`, `FormatArg::Sarif`, `FailOn`, and `exit_code_for`.
- `crates/polint-diagnostics/src/lib.rs` already has `OutputFormat::Sarif`, `render_sarif`, stable diagnostic fingerprints, deterministic sort/dedupe, and human/JSON snapshots.
- `crates/polint-graph/src/lib.rs` already has `ImportGraph::from_db`, `FunctionGraph::from_db`, and DOT rendering through `petgraph::dot::Dot`.
- `crates/polint-cli/tests/cli.rs` already uses `assert_cmd`, `tempfile`, parsed JSON helpers, and targeted integration fixtures for CLI behavior.
- `crates/polint-rules/tests/snapshots.rs` already snapshots representative human and JSON diagnostics through `polint_diagnostics::render`.

### Established Patterns

- CLI JSON/SARIF output should be parseable stdout and must not be mixed with human-only guidance.
- Diagnostics are sorted and deduped before rendering; tests should assert structured fields and deterministic output.
- CLI integration tests create temporary repos with `.polint.toml` and minimal Go/TS fixtures.
- Graph helpers currently rely on syntactic facts already available in `AnalysisDb`; deeper semantic resolution is intentionally out of scope.
- Phase 7 established that timing output should be tested for shape/order rather than exact elapsed values.

### Integration Points

- `check` routes through `analyze_and_run`, dedupes diagnostics, renders `OutputFormat`, and applies `exit_code_for`.
- `test-rules` currently delegates to `check` after printing a human message; Phase 8 should ensure this does not corrupt machine output modes.
- `profile-rules` runs analysis, prints per-rule rows, and applies `exit_code_for` to parser plus rule diagnostics.
- `graph` builds an analysis DB with profile `full`, `--no-cache`, and `fail_on none`, then emits DOT from `polint-graph`.
- SARIF-like output is centralized in `polint-diagnostics`, so renderer snapshots and CLI parseability tests can share the same contract.

</code_context>

<specifics>
## Specific Ideas

- Keep v1 honest: "SARIF-like" is acceptable; do not overclaim full SARIF certification.
- Treat `test-rules` as the current fixture/custom-rule harness until dynamic repo-local loading work provides a real loader.
- Empty graph output should be deterministic and valid DOT, not a fatal error, unless graph construction itself fails.
- Command hardening should favor small focused tests around existing behavior before adding new abstraction.

</specifics>

<deferred>
## Deferred Ideas

- Production Node/TypeScript import resolution for graph edges remains outside Phase 8 unless a narrow syntactic test requires a small adapter improvement.
- Dynamic repo-local Rust rule compilation/loading remains outside Phase 8; broader dynamic loading remains future work.
- Additional graph formats such as JSON, Mermaid, PNG/SVG, or interactive graph visualization are out of scope.
- GitHub Actions example can be started only if it naturally belongs to CI output proof; broader docs and examples remain Phase 10.

</deferred>

---

*Phase: 08-ci-output-and-graph-commands*
*Context gathered: 2026-05-01*

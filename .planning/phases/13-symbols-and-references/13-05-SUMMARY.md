---
phase: 13-symbols-and-references
plan: 05
subsystem: symbol-graph
tags: [rust, go, go-packages, go-types, symbols, references, sidecar]

requires:
  - phase: 13-symbols-and-references
    provides: symbol graph pipeline, stable IDs, and TS/JS provider patterns from Plans 13-01 through 13-04
  - phase: 12-resolved-imports-and-module-relationships
    provides: setup-missing provider diagnostics and Go tooling command patterns
provides:
  - Go sidecar using go/packages, go/types, and objectpath for typed symbol/reference JSON
  - Rust sidecar invocation with fixed go command args, GOFLAGS removal, schema validation, and repo path validation
  - ExactSemantic Go SymbolFact, DefinitionFact, and ReferenceFact conversion through SymbolGraphBuilder
  - deterministic setup-missing capability support for missing Go setup, command failures, invalid JSON, and invalid sidecar paths
affects: [symbol_graph, go_adapter, call_graph, sdk_facts, cache]

tech-stack:
  added:
    - golang.org/x/tools v0.45.0
  patterns:
    - repo-local Go sidecar compiled through a fixed go.work while package loading resets GOWORK for the target repository
    - sidecar stable keys as semantic input to SymbolGraphBuilder rather than public Go tool identities
    - setup-missing support rows for unavailable Go typed package setup

key-files:
  created:
    - go.work
    - go.work.sum
    - tools/polint-go-symbols/go.mod
    - tools/polint-go-symbols/go.sum
    - tools/polint-go-symbols/main.go
    - tools/polint-go-symbols/internal/symbols/emit.go
    - tools/polint-go-symbols/internal/symbols/emit_test.go
    - .planning/phases/13-symbols-and-references/13-05-SUMMARY.md
  modified:
    - crates/polint/src/symbol_graph/go.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/cache/keys.rs

key-decisions:
  - "Go sidecar JSON is versioned as polint-go-symbols-v1 and contains normalized metadata only, never source text or raw Go tool object identities."
  - "Rust compiles the repo-local sidecar with a fixed GOWORK path, while the sidecar loads the target repository with root go.work when present or GOWORK=off otherwise."
  - "Go package load errors produce capability diagnostics but retain exact facts that go/types returned."

patterns-established:
  - "Go setup boundary: missing go.mod, command failure, invalid JSON, and invalid paths become SetupMissing capability support and setup-missing reference rows."
  - "Go semantic keys: package symbols prefer objectpath in sidecar keys; local symbols include package ID, file, owner chain, name, and position."
  - "Sidecar validation: Rust rejects absolute, repo-escaping, or undiscovered sidecar file paths before fact conversion."

requirements-completed: [SYM-02, SYM-04]

duration: 19m11s
completed: 2026-05-13
---

# Phase 13 Plan 05: Go Symbols And References Summary

**go/packages-backed Go symbols and references with stable objectpath IDs and setup-aware Rust sidecar validation**

## Performance

- **Duration:** 19m11s
- **Started:** 2026-05-13T06:08:42Z
- **Completed:** 2026-05-13T06:27:53Z
- **Tasks:** 3
- **Files modified:** 11

## Accomplishments

- Added `tools/polint-go-symbols`, a small Go sidecar that loads packages with syntax, types, type info, scopes, selections, implicits, and module metadata.
- Replaced the Go symbol provider placeholder with safe sidecar invocation, schema/path validation, Go analyzer settings, and deterministic setup-missing support.
- Converted typed Go symbols, definitions, identifier references, method selector references, and field selectors into exact semantic polint facts with stable IDs.

## Task Commits

Each task was committed atomically:

1. **Task 1: Create the Go typed symbol sidecar** - `a6e5fd8` (test), `be28858` (feat), `2a5eaaa` (refactor)
2. **Task 2: Invoke and validate Go sidecar output from Rust** - `7fb9f7c` (test), `f6b716b` (feat)
3. **Task 3: Convert typed Go facts into stable polint facts** - `48c7252` (test), `d9aa5bb` (feat)

**Plan metadata:** committed after summary self-check.

## Files Created/Modified

- `go.work`, `go.work.sum` - Workspace wrapper that lets the required root-level Go test command see the nested sidecar module.
- `tools/polint-go-symbols/go.mod`, `tools/polint-go-symbols/go.sum` - Sidecar module with `golang.org/x/tools v0.45.0`.
- `tools/polint-go-symbols/main.go` - `symbols --root --patterns --tests --build-tags --json` CLI entrypoint.
- `tools/polint-go-symbols/internal/symbols/emit.go` - go/packages/go/types extraction for package rows, symbols, definitions, references, package errors, objectpath keys, and deterministic JSON.
- `tools/polint-go-symbols/internal/symbols/emit_test.go` - Sidecar behavior tests for typed rows, local keys, objectpath, and source-free JSON.
- `crates/polint/src/symbol_graph/go.rs` - Go config parsing, sidecar command execution, schema/path validation, setup support, diagnostics, and fact conversion.
- `crates/polint/src/symbol_graph/mod.rs` - Removed the obsolete unsupported Go provider helper after Go support was promoted.
- `crates/polint/src/cache/keys.rs` - Added regression coverage for Go symbol settings in `config_hash`.

## Decisions Made

- `languages.go.package_patterns` and `languages.go.build_tags` accept strings or arrays; string values are comma-split. `include_tests` defaults to `true`.
- Go sidecar package loading treats a target root `go.work` as authoritative; otherwise it forces `GOWORK=off` so the sidecar compile workspace cannot leak into target analysis.
- Sidecar keys are an internal semantic input only. Public facts expose polint-owned `SymbolId`, `DefinitionId`, and `ReferenceId`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added root Go workspace for sidecar verification**
- **Found during:** Task 1 RED verification
- **Issue:** `go test ./tools/polint-go-symbols/...` failed from the repository root because the sidecar is a nested Go module and the root was not a Go workspace.
- **Fix:** Added `go.work` and committed the generated `go.work.sum` so the plan's required root-level verification command works deterministically.
- **Files modified:** `go.work`, `go.work.sum`
- **Verification:** `go test ./tools/polint-go-symbols/...`
- **Committed in:** `a6e5fd8`, `be28858`

**2. [Rule 1 - Bug] Fixed sidecar CLI bool parsing and GOWORK leakage**
- **Found during:** Task 3 real sidecar conversion tests
- **Issue:** Go's bool flag parsing did not consume `--tests true`, so later flags were ignored. The fixed GOWORK needed to compile the repo-local sidecar also leaked into `packages.Load`, causing temp target modules to fail package loading.
- **Fix:** Parsed `--tests` as a string bool in the sidecar CLI, set a fixed `GOWORK` only for compiling the sidecar, and made sidecar package loading use the target root `go.work` or `GOWORK=off`.
- **Files modified:** `tools/polint-go-symbols/main.go`, `tools/polint-go-symbols/internal/symbols/emit.go`, `crates/polint/src/symbol_graph/go.rs`
- **Verification:** `go test ./tools/polint-go-symbols/...`; `cargo test -p polint --lib symbol_graph_go --locked`
- **Committed in:** `d9aa5bb`

---

**Total deviations:** 2 auto-fixed (1 Rule 3 blocking, 1 Rule 1 bug)
**Impact on plan:** Both fixes were required to execute the planned verification path and real sidecar integration. No public API scope was expanded.

## Issues Encountered

- The sidecar's initial no-source-text test matched owner metadata (`func Build`) rather than raw source. The test was tightened to reject actual source snippets and sentinel source content.
- `cargo fmt --all -- --check` caught formatting drift in the new Rust tests; `cargo fmt --all` was run before final verification.

## Known Stubs

None. Stub-pattern scanning matched test literals such as `TODO`/`FIXME` in existing config-hash tests and empty-string checks in sidecar code, not incomplete implementation stubs.

## User Setup Required

None - no external service configuration required.

## Verification

- `go test ./tools/polint-go-symbols/...`
- `cargo test -p polint --lib symbol_graph_go_setup --locked`
- `cargo test -p polint --lib symbol_graph_go --locked`
- `cargo test -p polint --lib config_hash_differs_when_go_symbol_settings_change --locked`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Go now has exact semantic symbols and references where repository-root Go setup is available, and deterministic setup-missing behavior where it is not. Plan 13-06 can focus on external-consumer SDK proof, documentation alignment, and cache/restore coverage across both TS/JS and Go.

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-13*

## Self-Check: PASSED

- Verified created files exist: `go.work`, `go.work.sum`, `tools/polint-go-symbols/go.mod`, `tools/polint-go-symbols/go.sum`, `tools/polint-go-symbols/main.go`, `tools/polint-go-symbols/internal/symbols/emit.go`, `tools/polint-go-symbols/internal/symbols/emit_test.go`, `.planning/phases/13-symbols-and-references/13-05-SUMMARY.md`
- Verified task commits exist: `a6e5fd8`, `be28858`, `2a5eaaa`, `7fb9f7c`, `f6b716b`, `48c7252`, `d9aa5bb`

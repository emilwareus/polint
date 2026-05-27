---
phase: 30-direct-call-facts
plan: "07"
type: implementation-summary
subsystem: direct-call-analysis
tags:
  - rust
  - eval
  - direct-calls
  - fixtures
  - call-indexes
dependency_graph:
  requires:
    - 30-01
    - 30-02
    - 30-03
    - 30-04
    - 30-05
    - 30-06
  provides:
    - native direct-call fixture coverage
    - direct-call count and D-10 index invariants
    - deterministic cold/warm/no-cache fixture runner
  affects:
    - phase-30-final-proof
    - analysis.calls
    - eval.native-fixtures
tech_stack:
  added: []
  patterns:
    - internal eval fixtures
    - metadata_debug_json-derived invariants
    - TDD red/green commits
key_files:
  created:
    - tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml
    - tests/eval-fixtures/direct-calls/core/repo/.polint.toml
    - tests/eval-fixtures/direct-calls/core/repo/go.mod
    - tests/eval-fixtures/direct-calls/core/repo/service.go
    - tests/eval-fixtures/direct-calls/core/repo/web/package.json
    - tests/eval-fixtures/direct-calls/core/repo/web/src/app.ts
    - tests/eval-fixtures/direct-calls/core/repo/web/src/helper.ts
  modified:
    - crates/polint/src/analysis/calls/direct.rs
    - crates/polint/src/analysis/calls/extract.rs
    - crates/polint/src/analysis/calls/unresolved.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/observed.rs
decisions:
  - Keep direct-call fixture coverage internal and test-facing; no public CallGraph API was exposed.
  - Use nonzero eval invariants for debug count and index buckets so the fixture guards coverage without hard-coding fragile exact counts.
  - Populate missing call-site owner symbols from existing function/symbol facts before call-store indexing.
metrics:
  duration: "22m10s"
  completed_at: "2026-05-21T09:37:14Z"
  tasks_completed: 2
  files_changed: 14
requirements:
  completed:
    - SAE-SEM-05
---

# Phase 30 Plan 07: Direct Call Fixture Summary

Native Go and TypeScript direct-call eval fixture coverage with deterministic cache comparison and debug count/index invariants.

## Tasks Completed

| Task | Name | Commit | Result |
| ---- | ---- | ------ | ------ |
| 1 RED | Add direct-calls native fixture coverage test | 0663668 | Failing fixture test committed |
| 1 GREEN | Implement direct-calls fixture runner | d50fa0d | Cold, warm, and no-cache fixture runner passes |
| 2 RED | Add final eval coverage guards | c11f1fd | Failing count/index invariant guards committed |
| 2 GREEN | Emit count and index invariants | c947420 | Debug count and D-10 index guards pass |

## Implementation Notes

- Added `tests/eval-fixtures/direct-calls/core` with Go and TypeScript sources that exercise direct references, import bindings, static/member calls, constructor-shaped calls, function values, dynamic properties, reflection, goroutine boundaries, eval, dynamic import, call/apply/bind, and setup-missing evidence.
- Added `run_direct_calls_core_fixture_for_test`, which runs the fixture cold, warm, and no-cache with `symbols`, `references`, `resolved_imports`, and `module_graph` capabilities.
- Converted call debug `counts` and `index_counts` into internal eval invariants such as `direct_calls.counts.by_status.Resolved.nonzero` and `direct_calls.index_counts.outgoing_by_symbol.nonzero`.
- Filled missing call-site owner symbols from existing symbol/definition facts before building call-store indexes, so symbol-based outgoing call indexes are exercised by the fixture.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Classified MIR unknown direct-call evidence**
- **Found during:** Task 1 GREEN
- **Issue:** Native Go/TS MIR lowering produced `MirValue::Unknown` evidence for several real call shapes, which prevented expected direct `CallTarget` rows from appearing.
- **Fix:** Added evidence-based callee classification for identifiers, member/static member calls, constructors, eval, dynamic import, call/apply/bind, setup-missing, and function-value shapes.
- **Files modified:** `crates/polint/src/analysis/calls/extract.rs`
- **Commit:** d50fa0d

**2. [Rule 1 - Bug] Stopped broad import-binding overclassification**
- **Found during:** Task 1 GREEN
- **Issue:** Direct resolution treated all references in files with resolved imports as import bindings, which hid local direct/static targets behind the wrong algorithm.
- **Fix:** Restricted import binding classification to module-linked targets, import symbols, or semantic import-name matches.
- **Files modified:** `crates/polint/src/analysis/calls/direct.rs`
- **Commit:** d50fa0d

**3. [Rule 1 - Bug] Preserved unsupported direct-call evidence**
- **Found during:** Task 1 GREEN
- **Issue:** Reflection, goroutine, eval, dynamic import, and call/apply/bind evidence could be resolved before unresolved/unsupported rows were emitted.
- **Fix:** Skipped direct target resolution for overlapping unsupported semantic rows and matched unsupported rows to same-file span-overlapping call sites before generic fallbacks.
- **Files modified:** `crates/polint/src/analysis/calls/direct.rs`, `crates/polint/src/analysis/calls/unresolved.rs`
- **Commit:** d50fa0d

**4. [Rule 2 - Missing Critical Functionality] Added imported TS fixture helper**
- **Found during:** Task 1 RED/GREEN
- **Issue:** The direct-call fixture needed a real imported TypeScript function to validate import-binding behavior instead of relying only on local calls.
- **Fix:** Added `web/src/helper.ts` and imported it from the fixture app.
- **Files modified:** `tests/eval-fixtures/direct-calls/core/repo/web/src/helper.ts`, `tests/eval-fixtures/direct-calls/core/repo/web/src/app.ts`
- **Commit:** 0663668, d50fa0d

**5. [Rule 1 - Bug] Populated symbol-backed outgoing call indexes**
- **Found during:** Task 2 GREEN
- **Issue:** The D-10 `outgoing_by_symbol` index stayed empty when call sites lacked owner symbols even though matching function/symbol facts existed.
- **Fix:** Derived missing call-site owner symbols from matching symbols or definitions before constructing the call store.
- **Files modified:** `crates/polint/src/core/mod.rs`
- **Commit:** c947420

## Verification

- `cargo test -p polint --lib eval::fixtures::direct_calls_core --locked`
- `cargo test -p polint --lib eval::direct_call_rows --locked`
- `cargo fmt --all -- --check`
- `rg -n "by_language|by_call_kind|by_algorithm|by_status|by_unresolved_reason|by_provider" crates/polint/src/eval/fixtures.rs crates/polint/src/eval/mod.rs tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml`
- `rg -n "outgoing_by_function|outgoing_by_symbol|incoming_by_symbol|incoming_by_function|unresolved_by_reason|unresolved_by_status" crates/polint/src/eval/fixtures.rs crates/polint/src/eval/mod.rs tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml`
- `rg -n "Resolved|Unresolved|Unsupported|SetupMissing|DirectReference|ImportBinding|StaticMember" tests/eval-fixtures/direct-calls/core/expected.polint-eval.toml`

## Known Stubs

None. Stub scan only found existing test literal strings and formatting helpers, not newly introduced placeholders or unwired fixture data.

## Threat Flags

None. Changes are internal analysis/eval fixture surfaces and do not introduce new network endpoints, auth paths, or external file-access contracts.

## Self-Check: PASSED

- Created summary and fixture files exist.
- Task commits found: 0663668, d50fa0d, c11f1fd, c947420.

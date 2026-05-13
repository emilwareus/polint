---
phase: 13-symbols-and-references
plan: 03
subsystem: symbol-graph
tags: [rust, symbols, references, pipeline, capabilities, diagnostics]

requires:
  - phase: 13-symbols-and-references
    provides: stable symbol/reference facts, SDK views, and builder foundations from Plans 13-01 and 13-02
  - phase: 12-resolved-imports-and-module-relationships
    provides: module graph provider pattern and resolved import relationship context
provides:
  - module graph derivation trigger for symbol/reference capability requests
  - derive_requested_symbols orchestration entrypoint with support overlay and provider diagnostics
  - crate-private TS/JS and Go symbol provider contracts
  - runner and parent CLI sequencing for symbol derivation before metrics and rules
  - deterministic capability diagnostic ordering before rule diagnostics at identical report locations
affects:
  - 13-symbols-and-references
  - 14-direct-and-resolved-call-graph-facts
  - symbol_graph
  - language_providers
  - diagnostics

tech-stack:
  added: []
  patterns:
    - setup-aware derived provider entrypoint following the module_graph support overlay model
    - crate-private language contracts returning normalized support/diagnostic output
    - explicit unsupported facts/support rows until semantic providers are promoted

key-files:
  created:
    - crates/polint/src/symbol_graph/ts.rs
    - crates/polint/src/symbol_graph/go.rs
    - .planning/phases/13-symbols-and-references/13-03-SUMMARY.md
  modified:
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/src/diagnostics/mod.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Symbol/reference requests trigger module graph derivation so rules do not need to request module graph views for relationship context."
  - "TS/JS and Go symbol provider contracts are crate-private and emit Unsupported support/fact rows until semantic extraction plans promote real support."
  - "Capability diagnostics sort before normal rule diagnostics at the same report location to preserve provider-before-rules visibility."

patterns-established:
  - "Provider support overlay: symbol graph support rows merge after module graph rows and before rule blocking."
  - "Language contract boundary: provider modules accept builder/db/config/plan inputs and expose no parser or sidecar internals."
  - "Capability-first reporting: setup/support diagnostics are visible before requesting rules can run with unavailable facts."

requirements-completed: [SYM-01, SYM-04]

duration: 19m
completed: 2026-05-13
---

# Phase 13 Plan 03: Symbol Derivation Pipeline Summary

**Symbol/reference derivation stage sequenced after module graph with setup-aware support overlays**

## Performance

- **Duration:** 19 min
- **Started:** 2026-05-13T05:16:50Z
- **Completed:** 2026-05-13T05:35:35Z
- **Tasks:** 3
- **Files modified:** 9

## Accomplishments

- Expanded module graph scheduling so `symbols` and `references` requests derive resolved import/module context automatically.
- Added `symbol_graph::derive_requested_symbols`, `SymbolGraphDerivation`, deterministic diagnostics/support merging, and crate-private TS/JS and Go provider contracts.
- Wired local rule-host and parent/no-host CLI paths to run symbol derivation after module graph derivation and before metrics/rules.
- Added CLI proof that symbol-requesting rules are blocked while providers are Unsupported and provider diagnostics precede normal rule diagnostics.

## Task Commits

Each task was committed atomically:

1. **Task 1: Make module graph derive for symbol/reference requests** - `d38a4d4` (test), `34bcbe1` (feat)
2. **Task 2: Add symbol graph derivation orchestration** - `8dba100` (test), `a8a9c7c` (feat)
3. **Task 3: Run symbol derivation before rules in CLI paths** - `dfb3993` (test), `5b13aa0` (feat)

**Plan metadata:** committed after summary self-check.

## Files Created/Modified

- `crates/polint/src/module_graph/mod.rs` - Adds `symbols` and `references` to module graph trigger capabilities and tests the relationship context behavior.
- `crates/polint/src/symbol_graph/mod.rs` - Adds symbol derivation orchestration, support overlay merging, provider diagnostics, and unit tests.
- `crates/polint/src/symbol_graph/ts.rs` - Adds crate-private TS/JS provider contract with deterministic Unsupported support/fact rows.
- `crates/polint/src/symbol_graph/go.rs` - Adds crate-private Go provider contract with deterministic Unsupported support/fact rows.
- `crates/polint/src/runner/mod.rs` - Runs symbol derivation after module graph derivation and passes merged support to rules.
- `crates/polint/src/cli/mod.rs` - Runs symbol derivation in the parent/no-local-rule analysis path.
- `crates/polint/src/diagnostics/mod.rs` - Prioritizes `polint/capability` diagnostics when report locations are identical.
- `crates/polint/tests/cli.rs` - Adds external-consumer CLI coverage for symbol capability blocking and provider diagnostic ordering.
- `.planning/phases/13-symbols-and-references/13-03-SUMMARY.md` - Captures execution output and verification results.

## Decisions Made

- Symbol graph provider bodies remain explicitly Unsupported in this plan; Plans 13-04 and 13-05 can replace the bodies without changing the orchestration contract.
- Provider diagnostics include capability, language, status, reason, docs path, and hint evidence so unsupported data is visible instead of silently empty.
- Capability diagnostics now sort ahead of normal rule diagnostics at the same file/range, matching provider-gate semantics in JSON output.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Prioritized capability diagnostics at identical report locations**
- **Found during:** Task 3 (Run symbol derivation before rules in CLI paths)
- **Issue:** The existing report sort ordered `local/*` rule diagnostics before `polint/capability` diagnostics when both used `<workspace>:1:1`, which violated the provider-before-rule diagnostic proof.
- **Fix:** Added a deterministic diagnostic sort priority so `polint/capability` diagnostics come first at the same file/range while preserving the rest of the stable sort key.
- **Files modified:** `crates/polint/src/diagnostics/mod.rs`
- **Verification:** `cargo test -p polint --test cli capability_planning --locked`; `cargo fmt --all -- --check`
- **Committed in:** `5b13aa0`

---

**Total deviations:** 1 auto-fixed (1 Rule 1 bug)
**Impact on plan:** The fix is limited to report ordering needed for capability/support visibility. No API surface was widened.

## Issues Encountered

- `cargo test -p polint --test cli capability_planning --locked` emits dead-code warnings for symbol graph builder/query APIs that are intentionally staged before later semantic provider plans consume them. The suite passes.
- The full CLI capability suite is slow because it compiles multiple temp local rule hosts.

## Verification

- `cargo test -p polint --lib module_graph_derives_for_symbol_capabilities --locked` passed.
- `cargo test -p polint --lib symbol_graph_derivation --locked` passed.
- `cargo test -p polint --test cli capability_planning --locked` passed: 8 tests.
- `cargo fmt --all -- --check` passed.
- Acceptance scans confirmed `derive_requested_symbols` is wired in runner and CLI paths, `symbol_graph.support_view` is passed to rules, TS/Go language entrypoints are crate-private, and no public language-tool API leaked from `ts.rs`/`go.rs`.

## Known Stubs

None. The unsupported provider rows are intentional capability state for this plan, not placeholder data; follow-up Plans 13-04 and 13-05 replace the language bodies with semantic extraction.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

The pipeline is ready for semantic language providers. Plan 13-04 can implement TS/JS extraction behind `derive_ts_symbols`, and Plan 13-05 can implement Go extraction behind `derive_go_symbols`, while keeping the runner sequencing and support merge unchanged.

---
*Phase: 13-symbols-and-references*
*Completed: 2026-05-13*

## Self-Check: PASSED

- Verified created files exist: `crates/polint/src/symbol_graph/ts.rs`, `crates/polint/src/symbol_graph/go.rs`, `.planning/phases/13-symbols-and-references/13-03-SUMMARY.md`
- Verified task commits exist: `d38a4d4`, `34bcbe1`, `8dba100`, `a8a9c7c`, `dfb3993`, `5b13aa0`

---
phase: 49-js-ts-function-token-propagation-driver
plan: 03
subsystem: eval
tags: [js, ts, solver, function-token, eval, determinism, public-surface]

requires:
  - phase: 49-js-ts-function-token-propagation-driver
    plan: 01
    provides: "JS token budgets, [solver.js] config, cache-key participation, and TokenFlowRequired handoff classifier"
  - phase: 49-js-ts-function-token-propagation-driver
    plan: 02
    provides: "real private TsTokensPolicy with closed inputs, deterministic token fixpoint, budget sentinel, and derived-edge dispatch"
provides:
  - "native TS token eval fixture gate"
  - "token-explosion BudgetExceeded and bounded-output proof"
  - "TS token determinism fixture"
  - "updated polyglot Go+TS canary for active intra-TS token edges"
  - "final Phase 49 verification and roadmap/state closeout"
affects: [JS-04, phase-50-readiness, solver, eval]

tech-stack:
  added: []
  patterns:
    - "Eval gates inspect crate-private solver output directly rather than promoting token internals to SDK or CLI"
    - "Synthetic closed inputs are used only where current TS frontend producers cannot yet create the needed stress shape from source"

key-files:
  created:
    - "crates/polint/src/eval/ts_tokens.rs"
    - "tests/eval-fixtures/ts-tokens/alias-parameter-return/"
    - "tests/eval-fixtures/ts-tokens/token-explosion/"
    - "tests/eval-fixtures/determinism/ts_tokens/"
  modified:
    - "crates/polint/src/eval/mod.rs"
    - "tests/eval-fixtures/polyglot-canary/go-ts/expected.polint-eval.toml"
    - "tests/eval-fixtures/polyglot-canary/go-ts/repo/tokens.ts"
    - ".planning/ROADMAP.md"
    - ".planning/STATE.md"

requirements-progress: [JS-04]

duration: 60min
completed: 2026-06-03T18:47:26Z
---

# Phase 49 Plan 03: TS Token Verification Summary

Phase 49 Plan 03 closed JS-04 with executable evidence around the real `TsTokensPolicy`: native TS token fixtures, token-explosion budget proof, polyglot non-interference, deterministic output checks, public-surface leak coverage, clippy, and the full `polint` test suite.

## Accomplishments

- Added `eval::ts_tokens` as a crate-private gate over TS token fixtures and solver output.
- Added `tests/eval-fixtures/ts-tokens/alias-parameter-return/` with direct alias, assignment, parameter callback, returned function, closure, and computed-property boundary cases.
- Added `tests/eval-fixtures/ts-tokens/token-explosion/` with a tight `[solver.js]` config and a closed-input stress assertion proving `BudgetStatus::BudgetExceeded`, bounded output, and no sentinel emitted as a target function.
- Added `tests/eval-fixtures/determinism/ts_tokens/` plus solver-input permutation coverage proving byte-identical TS token output under seeded ordering changes.
- Updated the polyglot Go+TS canary from the obsolete TS-stub invariant to the Phase 49 invariant: TS token edges are present, stay intra-TS, and do not interfere with Go RTA output.
- Recorded local Jelly-oriented evidence through the self-contained TS token fixture: direct token propagation adds precise callable targets without property/computed flooding. Phase 54 still owns external Jelly corpus floors.

## Task Commit

1. Tasks 1-4: TS token eval fixtures, token-explosion proof, polyglot/determinism updates, and full verification - `fda5d58e`

## Verification

- `cargo test -p polint eval::ts_tokens` - passed, 4 tests
- `cargo test -p polint ts_tokens_fixture_is_byte_identical_under_ten_seeded_permutations` - passed
- `cargo test -p polint ts_tokens_solver_output_is_byte_identical_under_permuted_inputs` - passed
- `cargo test -p polint --test public_surface_leak` - passed, 5 tests
- `cargo clippy -p polint --all-targets` - passed
- `cargo test -p polint` - passed: lib 2026, CLI 140, public-surface leak 5, doctest 1
- Pre-commit `make lint` passed on the implementation commit.

## Deviations

- Current semantic-graph producers emit TS token `CopyEdge`s for direct function aliases. They do not yet emit native parameter, return, or closure token copy edges from source. Plan 02 solver tests prove those flows once represented as `CopyEdge`s; Plan 03 native gates assert the current frontend handoff and boundary behavior honestly.
- The token-explosion proof uses synthetic closed `TsTokenInputs` derived from real fixture function nodes because the current frontend cannot yet produce many function tokens into one variable from source. This still exercises the real solver lattice, budget latch, sentinel, and dispatch behavior.
- External Jelly corpus execution is not required for this phase locally. The fixture evidence records precise recall improvement without property flooding; Phase 54 owns hard benchmark promotion floors.
- The Plan 03 implementation commit updated `.planning/ROADMAP.md` plan listing, but final completion state is recorded in this summary closeout commit.

## Phase 50 Readiness

Phase 49 is complete. Phase 50 can build the JS/TS object, property, prototype, and `this` model on top of a working function-token driver, while keeping property/prototype/`this` unresolved reasons out of Phase 49 token dispatch.

## Self-Check: PASSED

- Summary artifact: `.planning/phases/49-js-ts-function-token-propagation-driver/49-03-SUMMARY.md`
- Required implementation commit: `fda5d58e`
- Key invariant: no public SDK/runner/CLI surface promoted; `ALLOWED_PRELUDE` unchanged.

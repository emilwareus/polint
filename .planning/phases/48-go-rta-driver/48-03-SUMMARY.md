---
phase: 48-go-rta-driver
plan: 03
subsystem: testing
tags: [go, rta, call-graph, solver, fixtures, eval, determinism, polyglot, interface-dispatch, address-taken, budget]

# Dependency graph
requires:
  - phase: 48-go-rta-driver (Plan 01)
    provides: "Go-frontend RTA-signal facts (instantiated-type, address-taken, dynamic-dispatch) lowered + cache-keyed"
  - phase: 48-go-rta-driver (Plan 02)
    provides: "analysis::solver::go_rta fixpoint + dispatch resolver, GoRtaInputs::from_db, SolverEngine::run_to_solver_output routing, GoRtaSubBudget + [solver].go config threading, BudgetExceeded latching"
  - phase: 43-reachability-roots-per-suite-scoring-mode
    provides: "ReachabilityRootFact seed + reachable-graph marking contract; the N=10 determinism gate (assert_n10_byte_identical / assert_reachability_marking_exercised)"
  - phase: 42-benchmark-identity-renderers-dedup
    provides: "public-surface-leak gate (ALLOWED_PRELUDE locked snapshot)"
provides:
  - "Three native Go RTA fixtures under tests/eval-fixtures/go-rta/ (iteration-cap, interface-dispatch, address-taken) + a crate-private eval::go_rta gate that drives them through SolverEngine::run_to_solver_output and asserts BudgetExceeded, the instantiated-type filter, and func-value resolution"
  - "The polyglot Go+TS canary fixture (tests/eval-fixtures/polyglot-canary/go-ts/) + its gate test proving Go RTA edges resolve AND TS behavior is unchanged (no cross-language interference; TsTokensPolicy still a stub)"
  - "The determinism/go_rta fixture wired into the inherited N=10 determinism gate (byte-identical across 10 seeded permutations + reachable-graph marking exercised) — the RTA-derived solver edges join the observed JSON deterministically"
  - "Rule-1 frontend correctness fixes the verification surfaced: whole-program set-fact dedup, bare method-set names, and method node-mapping by span-containment — WITHOUT which interface dispatch resolved zero real Go edges"
affects: [phase-49-ts-tokens, phase-52-refined-calls-projection, phase-54-bench-promotion-gate, GO-05, go_rta]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-area acceptance gate that rebuilds the closed solver snapshot (GoRtaInputs::from_db) + points-to constraints and drives them through the production engine (SolverEngine::run_to_solver_output), reading the resulting SolverOutput directly — the always-runnable proof while solver edges are not yet projected to the observable call-graph (Phase 52)"
    - "Fixture-config-driven budget bite: the gate reads the fixture's own [solver.go] config via to_go_sub_budget so the iteration-cap fixture's tight cap threads end to end exactly as the kernel would"
    - "Whole-reachable-program SET facts (callsite / address_taken / instantiated_type / dynamic_dispatch) dedup by stable key in normalized() — an identity-duplicate row is the same set member, not a conflict"
  patterns-established:
    - "RTA acceptance fixtures assert the instantiated-type FILTER (a declared-but-not-instantiated type is excluded) + honest func-value resolution (every func-value edge targets an address-taken function) — no fixture rewards edge flooding"

key-files:
  created:
    - "crates/polint/src/eval/go_rta.rs"
    - "tests/eval-fixtures/go-rta/iteration-cap/{expected.polint-eval.toml,repo/{.polint.toml,go.mod,main.go}}"
    - "tests/eval-fixtures/go-rta/interface-dispatch/{expected.polint-eval.toml,repo/{go.mod,main.go}}"
    - "tests/eval-fixtures/go-rta/address-taken/{expected.polint-eval.toml,repo/{go.mod,main.go}}"
    - "tests/eval-fixtures/polyglot-canary/go-ts/{expected.polint-eval.toml,repo/{.polint.toml,go.mod,main.go,tokens.ts,package.json,tsconfig.json}}"
    - "tests/eval-fixtures/determinism/go_rta/{expected.polint-eval.toml,repo/{go.mod,main.go}}"
  modified:
    - "crates/polint/src/eval/mod.rs"
    - "crates/polint/src/eval/model.rs"
    - "crates/polint/src/eval/determinism_gate.rs"
    - "crates/polint/src/go/semantic/store.rs"
    - "crates/polint/src/analysis/solver/go_rta/inputs.rs"
    - "crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go"
    - ".planning/ROADMAP.md"

key-decisions:
  - "The eval::go_rta gate sources RTA edges from the kernel-built AnalysisDb by rebuilding GoRtaInputs::from_db + driving SolverEngine::run_to_solver_output — NOT through graph_edges_from_kernel_output (which reads refined_call_edges/call_targets, not solver_derived_edges; Phase 52/GRAPH-05 wires solver edges into the observable refined-call projection). This is the always-runnable, clone-free proof."
  - "Iteration-cap BudgetExceeded is driven by max_candidates_per_callsite = 1 (one interface invoke with three instantiated implementers), NOT max_rta_rounds: all exported Go functions are reachability roots (RootKind::Exported), so a multi-round reachability chain cannot be built — the per-callsite candidate cap is the deterministic, config-threaded trigger."
  - "The four go-rta/polyglot manifests carry NO [[expected]] rows: the solver-derived edges + budget signal are crate-private and consumed during storage, so they are not observable manifest facts, and the native-fixture suite-coverage runner treats every [[expected]] row as a must-match assertion. The eval::go_rta gate is the executable proof; the manifests document the invariants in comments (the 'invariant' artifact token is retained)."
  - "Added FixtureArea::GoRta and FixtureArea::PolyglotCanary (closed-enum extension mirroring existing variants) so the new fixture areas parse under deny_unknown_fields; neither is a required-coverage area."

patterns-established:
  - "Verification fixtures that exercise REAL Go interface dispatch surface latent frontend stable-key/identity bugs the synthetic unit tests miss (same-named methods on different receivers, shared callees, SSA point-spans vs tree-sitter declaration spans)."

requirements-completed: [GO-05]

# Metrics
duration: 150min
completed: 2026-06-02
---

# Phase 48 Plan 03: Go RTA Verification — Fixtures + Gates Summary

**Three native Go RTA fixtures (iteration-cap BudgetExceeded, interface-dispatch instantiated-type filter, address-taken func-value resolution) + a polyglot Go+TS canary + a go_rta determinism fixture, driven by a crate-private `eval::go_rta` gate, prove GO-05's four success criteria — and surfaced three Rule-1 frontend correctness bugs (set-fact dedup, bare method-set names, method node-mapping by span-containment) without which RTA resolved zero real Go interface edges.**

## Performance

- **Duration:** ~150 min
- **Started:** 2026-06-02
- **Completed:** 2026-06-02
- **Tasks:** 3
- **Files modified:** 23 (created 23 fixture/gate files; modified 7 source/doc files)

## Accomplishments
- **Iteration-cap (D-14, criterion 2):** `tests/eval-fixtures/go-rta/iteration-cap/` — one `s.Area()` interface invoke with three instantiated implementers (Circle/Square/Triangle), capped by the fixture's own `[solver.go] max_candidates_per_callsite = 1`, latches `BudgetStatus::BudgetExceeded`. The gate asserts the config threads into the `GoRtaSubBudget` AND the budget signal is observed.
- **Interface-dispatch (D-15, criterion 4):** Dog instantiated → `main -> (Dog).Speak` resolves; Cat declared + in its method-set but never instantiated → excluded. The gate proves the instantiated-type FILTER (RTA, not CHA) and that every resolved edge honors the never-exact precision ceiling.
- **Address-taken (D-15, criterion 4):** an opaque func() value in `main` (selected from a slice so SSA cannot resolve it) resolves to the address-taken `func()` set {handler, other} via signature match; noise (func(int)) is excluded. The gate proves every func-value RTA edge targets an address-taken function (no flooding).
- **Polyglot Go+TS canary (D-16, criterion 3):** `tests/eval-fixtures/polyglot-canary/go-ts/` — the Go RTA driver resolves the `(Dog).Speak` interface edge while the TS half is analyzed (TS functions exist) but the solver derives NO TS-endpoint edge (`TsTokensPolicy` is a stub) — proving no cross-language interference through the shared solver core. Created here; Phase 54 promotes it to a hard gate.
- **Determinism (D-17):** `tests/eval-fixtures/determinism/go_rta/` wired into the inherited N=10 gate — byte-identical across 10 seeded permutations (the RTA-derived solver edges join the observed JSON) + reachable-graph marking exercised (root + call site + unreachable orphan mark).
- **Inherited gates green:** public-surface-leak gate green with ALLOWED_PRELUDE unchanged (all `go_rta` types `pub(crate)`); the x/tools `go_x_tools_callgraph` adapter still compiles and degrades gracefully (local clone absent); `provider_order_for_test()` snapshots and points-to fixtures unchanged (full `cargo test -p polint` green).

## Observed RTA baseline (for Phase 54's promotion gate)

Self-contained fixtures (the x/tools clone is absent locally, so the suite-level recall numbers are unavailable here; the suite scores automatically in CI when the clone is present):

| Fixture | dynamic callsites | RTA edges resolved | budget status |
|---------|-------------------|--------------------|---------------|
| interface-dispatch | 1 | 1 (Dog.Speak; Cat excluded) | WithinBudget |
| address-taken | 2 | 3 (func()→{handler,other}; func(int)→{noise}) | WithinBudget |
| iteration-cap | 1 | 1 (pre-cap edge kept) | BudgetExceeded |
| polyglot-canary | 1 | 1 (Dog.Speak; 0 TS edges) | WithinBudget |

Precision is held honest: the instantiated-type filter excludes the non-instantiated Cat (no false positive), and func-value edges target only address-taken functions. The hard per-suite precision floor (Go ≥60%), F-score β=0.5, and canary-as-hard-gate land in Phase 54 (BENCH-01).

## Task Commits

Each task was committed atomically:

1. **Task 1: go-rta native fixtures + eval::go_rta gate (D-14/D-15)** — `32a8b7b6` (feat)
2. **Task 2: polyglot Go+TS canary + go_rta determinism fixture (D-16/D-17)** — `b9d4ca47` (feat)
3. **Task 3: drop unsatisfiable [[expected]] invariant rows from RTA manifests (full-suite green sweep)** — `435cfa02` (fix)

**Plan metadata:** committed with STATE/ROADMAP/REQUIREMENTS updates (docs: complete plan)

## Files Created/Modified
- `crates/polint/src/eval/go_rta.rs` — crate-private `#[cfg(test)]` acceptance gate (4 tests) driving the fixtures through `SolverEngine::run_to_solver_output`.
- `crates/polint/src/eval/mod.rs` — registered `#[cfg(test)] pub(crate) mod go_rta`.
- `crates/polint/src/eval/model.rs` — added `FixtureArea::GoRta` / `PolyglotCanary` variants.
- `crates/polint/src/eval/determinism_gate.rs` — added the `go_rta` byte-identical + reachable-graph-marking tests (the go_reachable analogues).
- `crates/polint/src/go/semantic/store.rs` — Rule-1: dedup whole-program set facts (callsite/address_taken/instantiated_type/dynamic_dispatch) by stable key in `normalized()`.
- `crates/polint/src/analysis/solver/go_rta/inputs.rs` — Rule-1: match Go methods to nodes by file+name+span-containment (SSA point-span within tree-sitter declaration) + index the bare method name; regression tests added.
- `crates/polint/go-sidecar/.../internal/semantic/emit.go` — Rule-1: method-set carries bare method names (`Obj().Name()`), not full signatures, so interface-invoke matching works.
- `tests/eval-fixtures/{go-rta/*, polyglot-canary/go-ts, determinism/go_rta}/` — the fixture trees.
- `.planning/ROADMAP.md` — Phase 48 marked 3/3 Complete.

## Decisions Made
- **Gate sources edges from the DB, not the call-graph projection.** `graph_edges_from_kernel_output` reads `refined_call_edges`/`call_targets`, NOT `solver_derived_edges` — Phase 52 (GRAPH-05) wires solver edges into the observable refined-call projection. So the gate rebuilds `GoRtaInputs::from_db` + drives `SolverEngine::run_to_solver_output` and reads the `SolverOutput` directly. This is the clone-free, always-runnable proof and matches Plan 03 Task 1's instruction ("source the RTA edges from `db.solver_derived_edges()` via the kernel output").
- **Iteration-cap uses the per-callsite candidate cap, not the round cap.** All exported Go functions are reachability roots (`RootKind::Exported`), so a transitive multi-round chain (main → A → B) cannot be built (everything is reachable in round 0). The deterministic, config-threaded trigger is `max_candidates_per_callsite = 1` against one interface invoke with three instantiated implementers.
- **Manifests carry no `[[expected]]` rows.** The solver edges + budget signal are crate-private; the suite-coverage runner treats `[[expected]]` rows as must-match assertions, so documentation-only invariant rows produced false negatives. The gate is the proof; the manifests document the invariant in comments.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Whole-program set facts collided on duplicate stable keys**
- **Found during:** Task 1 (running the first go-rta fixtures through the kernel)
- **Issue:** The Go semantic provider failed validation (`duplicate ... callsite/address_taken stable key`) on fixtures with same-named methods on different receivers (compiler-generated pointer-method wrappers `(*Cat).Speak` with synthetic NoPos calls) and shared callees (`fmt.Println` address-taken from two functions). The callsite / address_taken / instantiated_type / dynamic_dispatch facts are whole-reachable-program SET harvests where the same official identity legitimately recurs, but they were emitted as duplicate rows that `validate_unique` rejected — so NO Go facts flowed at all.
- **Fix:** Dedup these four families by stable key (keep first) after the stable-key sort in `GoSemanticFactsOutput::normalized()`. An identity-duplicate row is the same set member, not a conflict. Packages/functions/method-sets are NOT deduped (a real duplicate there is a genuine conflict the validator must still reject).
- **Files modified:** crates/polint/src/go/semantic/store.rs (+ a dedup regression test)
- **Verification:** go::semantic (50) + the new store dedup test green; the fixtures now load.
- **Committed in:** `32a8b7b6` (Task 1 commit)

**2. [Rule 1 - Bug] Interface dispatch resolved zero real Go edges (method node-mapping + method-set content)**
- **Found during:** Task 1 (interface-dispatch gate produced an empty edge set)
- **Issue:** Two compounding frontend bugs meant RTA never resolved a real Go interface edge: (a) `GoRtaInputs::from_db`'s `matching_core_function` required EXACT span equality, but the SSA frontend reports a zero-width point span for methods while tree-sitter reports the full declaration span, so methods never mapped to their semantic nodes (`methods_by_receiver` was empty); and (b) the method-set carried full signature strings (`func (Dog).Speak() string`) instead of the bare invoked method name (`Speak`), so the interface-invoke method-set membership check never matched.
- **Fix:** (a) match Go methods by file+name+language with span-CONTAINMENT (the SSA point-span lies within the tree-sitter declaration span) in BOTH `matching_core_function` and `qualified_for_function_id`, and index the BARE method name (`bare_method_name`) in `methods_by_receiver`; (b) emit `methodSet.At(i).Obj().Name()` (the bare method identifier) in the sidecar `emitMethodSets`.
- **Files modified:** crates/polint/src/analysis/solver/go_rta/inputs.rs (+ regression tests), crates/polint/go-sidecar/.../internal/semantic/emit.go
- **Verification:** interface-dispatch / address-taken / iteration-cap / polyglot gates all resolve real edges; go::semantic (50) + inputs (4) green; full suite green.
- **Committed in:** `32a8b7b6` (Task 1 commit)

**3. [Rule 1 - Bug] Documentation-only `[[expected]]` invariant rows produced false negatives**
- **Found during:** Task 3 (full `cargo test -p polint` sweep)
- **Issue:** The native-fixture suite-coverage runner (`eval_native_fixture_suite_covers_required_categories`) runs every checked-in fixture and asserts `false_negatives == 0`. The go-rta/polyglot manifests' documentation `[[expected]] invariant` rows are not observable facts (the solver signal is crate-private), so they were counted as misses.
- **Fix:** Removed the `[[expected]]` rows from the four manifests; the `eval::go_rta` gate is the executable proof, and the invariant names stay documented in manifest comments (the `contains: "invariant"` artifact token is retained).
- **Files modified:** the four go-rta/polyglot expected.polint-eval.toml manifests
- **Verification:** the suite-coverage test + full `cargo test -p polint` green.
- **Committed in:** `435cfa02` (Task 3 commit)

---

**Total deviations:** 3 auto-fixed (3 Rule-1 bugs in the Plan 01/02 Go frontend + RTA input join that only realistic interface-dispatch fixtures could surface)
**Impact on plan:** All three were correctness requirements for GO-05's core mechanism (interface invoke by method-set). Without them the RTA driver derived zero real Go edges despite passing its synthetic unit tests (which used already-bare method names and exact spans). No scope creep — the fixes are confined to the Go-frontend set-fact normalization, the RTA input join, and the method-set emission; no provider-order, points-to fixture, or ALLOWED_PRELUDE change.

## Issues Encountered
- **The synthetic unit tests masked the integration bugs.** Plan 02's `inputs.rs`/`fixpoint.rs` unit tests constructed `GoRtaInputs` with already-bare method names (`"Read"`) and exact spans, so the method node-mapping and method-set-content bugs only manifested against real `go/packages` SSA + tree-sitter facts. The verification fixtures are exactly what exposed them.
- **Reachability seeds every exported Go function as a root**, which defeated the original multi-round iteration-cap design; pivoted to the per-callsite candidate-cap trigger.
- **The x/tools native suite (D-15)** scores RTA edges only when the local `golang-tools` clone is present (absent here, `ignored_by_git`); the adapter `graph_edges_from_kernel_output` reads `refined_call_edges`/`call_targets`, so it will observe RTA edges only after Phase 52 (GRAPH-05) projects solver edges into refined calls. The self-contained fixtures are the always-runnable proof for Phase 48.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- **GO-05 is complete** — the RTA driver resolves Go interface-invoke and func-value dispatch with the instantiated-type filter, honest budget exhaustion, determinism, and the polyglot non-regression canary, all proven by always-runnable fixtures.
- **Phase 49 (JS-04)** can mirror this verification pattern for the TS token driver; the polyglot canary already asserts the TS stub stays empty, so making `TsTokensPolicy` real must keep the Go half of the canary green.
- **Phase 52 (GRAPH-05)** must project the `polint.solver` RTA edges into `refined_calls` so `graph_edges_from_kernel_output` (and the x/tools oracle-rta suite) observe them; the self-contained `eval::go_rta` gate can then be cross-checked against the suite-level recall.
- **Phase 54 (BENCH-01)** has the recall baseline above and will promote the polyglot canary to a hard gate + enforce the per-suite precision floor.

## Threat Flags

None — no new network endpoints, auth paths, file-access patterns, or trust-boundary schema changes. The fixtures are checked-in test inputs run through the existing hardened kernel/sidecar; all `go_rta` types stay `pub(crate)` and ALLOWED_PRELUDE is unchanged (T-48-03-03 mitigated). The polyglot canary explicitly asserts no cross-language solver-state leakage (T-48-03-02 mitigated).

## Self-Check: PASSED

- SUMMARY: `.planning/phases/48-go-rta-driver/48-03-SUMMARY.md` — FOUND
- Commits: `32a8b7b6` (Task 1), `b9d4ca47` (Task 2), `435cfa02` (Task 3) — all FOUND
- Created fixtures (go-rta/{iteration-cap,interface-dispatch,address-taken}, polyglot-canary/go-ts, determinism/go_rta) + the eval::go_rta gate — all FOUND
- Full `cargo test -p polint` green: 1950 lib + 140 integration + 5 public_surface_leak + 1 doc, 0 failures; `cargo clippy -p polint --all-targets` no warnings; determinism + leak + provider-order snapshots unchanged.

---
*Phase: 48-go-rta-driver*
*Completed: 2026-06-02*

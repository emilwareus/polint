---
phase: 48-go-rta-driver
plan: 01
subsystem: api
tags: [go, ssa, rta, call-graph, go-frontend, sidecar, semantic-graph, cache-key]

# Dependency graph
requires:
  - phase: 46-go-semantic-frontend-sidecar
    provides: "polint-go-frontend SSA sidecar (ssautil.AllPackages + prog.Build), GoSemanticMethodSet/Callsite facts, NDJSON protocol, length-prefixed stable keys"
  - phase: 47-unified-solver-core-derived-edge-provenance
    provides: "GoRtaPolicy honest stub, SolverBudget/BudgetStatus, DerivedEdgeFact + provenance, reserved SolverEngine seam, polint.solver provider slot"
  - phase: 43-reachability-roots-per-suite-scoring-mode
    provides: "ReachabilityRootFact seed set + reachable-graph marking contract; determinism gate"
provides:
  - "Go sidecar emits instantiated_type (MakeInterface rapid-type), address_taken (MakeClosure/func-value), and dynamic_dispatch (interface/func-value discriminant) NDJSON rows"
  - "Three crate-private GoSemantic* RTA-signal facts (GoSemanticAddressTakenFact, GoSemanticInstantiatedTypeFact, GoSemanticDynamicDispatchFact) lowered, normalized, validated, cache-keyed, and exposed via AnalysisDb accessors"
  - "SchemaVersion / GO_SEMANTIC_SCHEMA / GO_SEMANTIC_SCHEMA_LABEL bumped to -2; provider parameter digest folds the new fact-family identifiers"
affects: [phase-48-plan-02-go-rta-fixpoint, GO-05, go_rta, solver]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SSA instruction-walk harvest of RTA rapid-type/address-taken/dispatch signals over the already-built sidecar SSA program"
    - "Schema-pin lockstep: Go SchemaVersion bump requires Rust GO_SEMANTIC_SCHEMA + allowed_kinds + all NDJSON test fixtures bumped in lockstep (decode_ndjson_str strictly pins the schema string)"
    - "Honest-discriminant validation guard: dynamic_dispatch rows must carry interface_type+method or signature, and a non-empty callsite_stable_key, else fail closed as InvalidFact"

key-files:
  created: []
  modified:
    - "crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go"
    - "crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit_test.go"
    - "crates/polint/src/go/semantic/protocol.rs"
    - "crates/polint/src/go/semantic/facts.rs"
    - "crates/polint/src/go/semantic/lower.rs"
    - "crates/polint/src/go/semantic/store.rs"
    - "crates/polint/src/go/semantic/validate.rs"
    - "crates/polint/src/go/semantic/cache_key.rs"
    - "crates/polint/src/go/semantic/client.rs"
    - "crates/polint/src/go/semantic/tests.rs"
    - "crates/polint/src/go/semantic/provider.rs"
    - "crates/polint/src/core/mod.rs"

key-decisions:
  - "Harvest MakeInterface ONLY for the rapid-type set; deliberately exclude the *ssa.Alloc/MakeMap/MakeSlice/MakeChan families because allocation alone does not make a type dynamically dispatchable under x/tools RTA — only interface conversion does — so adding them would over-approximate and flood precision."
  - "The Rust decoder strictly pins GO_SEMANTIC_SCHEMA, so the Go -> -2 bump forced bumping GO_SEMANTIC_SCHEMA + adding the 3 new allowed_kinds + bumping ALL NDJSON test fixtures (protocol/lower/tests/provider/client) to -2 in lockstep."
  - "New GoSemantic*Id newtypes live in go/semantic/facts.rs (not analysis/ids.rs) and carry no Default/serde, mirroring GoSemanticMethodSetId; the assert_small_id_contract list was NOT perturbed."
  - "Bumped GO_SEMANTIC_SCHEMA_LABEL to go-semantic-facts-2 and added address_taken_v1/instantiated_type_v1/dynamic_dispatch_v1 to the provider parameter digest (D-12); left the analysis_kernel SchemaVersion numeric version at 1 since the label name already encodes the vocabulary bump."

patterns-established:
  - "RTA-signal emission is additive over the existing SSA walk; deterministic via slice iteration with maps used only for membership dedup, doubly safe under the Rust store's stable-key sort + dense-id assignment."
  - "dynamic_dispatch detail joins back to its GoSemanticCallsiteFact via callsite_stable_key equal to the callsite row's own stable_key."

requirements-completed: [GO-05]

# Metrics
duration: 25min
completed: 2026-06-02
---

# Phase 48 Plan 01: Go-frontend RTA-signal emission Summary

**The Go sidecar now harvests the three RTA inputs (instantiated runtime types from `*ssa.MakeInterface`, address-taken functions from `*ssa.MakeClosure`/func-value operands, and dynamic-callsite dispatch detail) and lowers them into crate-private, stable-keyed, cache-participating `GoSemantic*` facts exposed via `AnalysisDb` — the load-bearing inputs Plan 2's `go_rta` fixpoint consumes.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-06-02T17:59:48Z
- **Completed:** 2026-06-02T18:24:48Z
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments
- Go sidecar `emit.go` emits `instantiated_type`, `address_taken`, and `dynamic_dispatch` rows from the already-built SSA program; `SchemaVersion == "polint-go-semantic-2"`; the `unresolved_dynamic` reason is now honest present-tense ("interface or func-value dynamic dispatch").
- Three crate-private RTA-signal facts + `*Id` newtypes added, lowered (honest `None` discriminants), normalized (dense IDs only after stable-key sort), validated (uniqueness + honest-discriminant guard + non-empty `callsite_stable_key`), cache-keyed, and exposed via three new `AnalysisDb` accessors.
- Schema bumped in lockstep across the Go sidecar and the Rust client (`GO_SEMANTIC_SCHEMA`, `allowed_kinds`, `GO_SEMANTIC_SCHEMA_LABEL` → `-2`); the provider parameter digest folds the three new fact-family identifiers so a vocabulary change invalidates downstream (D-12).
- Public-surface-leak gate green (all new types `pub(crate)`, `ALLOWED_PRELUDE` unchanged); determinism gate green (10-shuffle byte-identical); `polint.solver` provider-order slot unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Harvest RTA SSA signals in the Go sidecar emitter + bump SchemaVersion** - `b91fbea6` (feat)
2. **Task 2: Add the three Go RTA-signal facts + IDs + protocol frame fields + lowering arms** - `5b89665a` (feat)
3. **Task 3: Thread new facts through store/validate/cache-key/DB + bump the schema label** - `71c4acd9` (feat)

**Plan metadata:** `<META_HASH>` (docs: complete plan)

## Files Created/Modified
- `crates/polint/go-sidecar/.../internal/semantic/emit.go` - Harvest MakeInterface instantiated types, MakeClosure/func-value address-taken, dynamic-dispatch detail; SchemaVersion -> -2; honest reason string.
- `crates/polint/go-sidecar/.../internal/semantic/emit_test.go` - Go tests for the three new row families, schema version, and dispatch->callsite join.
- `crates/polint/src/go/semantic/protocol.rs` - Pin GO_SEMANTIC_SCHEMA to -2; add function/interface_type/callsite_stable_key frame fields; allow the three new row kinds.
- `crates/polint/src/go/semantic/facts.rs` - Three new RTA-signal facts + their *Id newtypes (mirroring GoSemanticMethodSet).
- `crates/polint/src/go/semantic/lower.rs` - Three lower_* builders + match arms; new lowering tests.
- `crates/polint/src/go/semantic/store.rs` - Three new fact vectors on GoSemanticFactsOutput + normalized() dense-id blocks.
- `crates/polint/src/go/semantic/validate.rs` - validate_unique for the three families + honest-discriminant guard for dynamic_dispatch; new validation tests.
- `crates/polint/src/go/semantic/cache_key.rs` - GO_SEMANTIC_SCHEMA_LABEL -> go-semantic-facts-2; new fact-family identifiers in the parts list; trip-wire + pre-phase48-differs tests.
- `crates/polint/src/go/semantic/client.rs` - Schema literal -> -2 in the missing-terminator fixture (Rule 1 fix).
- `crates/polint/src/go/semantic/{tests.rs,provider.rs}` - Schema literal -> -2 in NDJSON test fixtures.
- `crates/polint/src/core/mod.rs` - Store the three new fact vectors in replace_go_semantic_facts + three new go_semantic_* accessors.

## Decisions Made
- **Rapid-type set = MakeInterface only.** Documented in `emitInstantiatedTypes` with the x/tools RTA rationale: allocation (`*ssa.Alloc`/`MakeMap`/`MakeSlice`/`MakeChan`) does not by itself make a type dynamically dispatchable under RTA; only interface conversion does. Harvesting alloc families would over-approximate the rapid-type set and flood precision without lifting recall. This is the minimal, honest surface the plan's D-05 calls for.
- **Schema-pin lockstep.** `decode_ndjson_str` strictly rejects any schema != `GO_SEMANTIC_SCHEMA`, so the Go `SchemaVersion` bump to `-2` required bumping the Rust constant, registering the three new `allowed_kinds`, and updating every hard-coded `polint-go-semantic-1` NDJSON literal across protocol/lower/tests/provider/client to `-2`.
- **IDs stay in facts.rs.** New `GoSemantic*Id` newtypes follow the existing `GoSemanticMethodSetId` pattern (no `Default`/serde, defined in `go/semantic/facts.rs`), so the `assert_small_id_contract` list in `analysis/ids.rs` was confirmed unperturbed.
- **Label bump, not numeric version bump.** `GO_SEMANTIC_SCHEMA_LABEL` → `go-semantic-facts-2` carries the vocabulary growth into the manifest `primary_schema_label()` and the cache-input digest; the `analysis_kernel` `SchemaVersion.version` was left at `1` (redundant to double-encode the bump, lower snapshot churn).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Bumped the schema literal in client.rs's missing-terminator test**
- **Found during:** Task 3 (full `cargo test -p polint` run)
- **Issue:** `go::semantic::client::tests::missing_terminator_from_fake_sidecar_is_typed_protocol_error` used a hard-coded `polint-go-semantic-1` literal not caught by the Task-2 fixture sweep. Once the decoder pinned `-2`, the frame decoded to `UnsupportedSchema("polint-go-semantic-1")` instead of reaching the intended `MissingEnd` assertion.
- **Fix:** Updated the fixture literal to `polint-go-semantic-2` so the frame is accepted and the test exercises the missing-terminator path as designed.
- **Files modified:** crates/polint/src/go/semantic/client.rs
- **Verification:** `cargo test -p polint --lib go::semantic` → 50 passed (incl. this test); full `cargo test -p polint` green.
- **Committed in:** `71c4acd9` (Task 3 commit)

---

**Total deviations:** 1 auto-fixed (1 bug — schema-pin lockstep completeness)
**Impact on plan:** The fix was a direct consequence of the intended schema-pin change in Task 2; no scope creep. All other work matched the plan exactly.

## Issues Encountered
- An early Go test fixture used the method *expression* `T.M` (type `func(T)`) where a `func()` was required, which failed type-checking and produced no SSA (no rows). Resolved by switching to the method *value* `t.M`, which both type-checks and exercises the `*ssa.MakeClosure` (bound-method) address-taken path.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 2 (`go_rta` RTA fixpoint policy) can now consume `db.go_semantic_address_taken()`, `db.go_semantic_instantiated_types()`, and `db.go_semantic_dynamic_dispatch()` (joined to callsites via `callsite_stable_key`) plus the existing method-sets and reachability roots.
- No semantic-graph constraint emission or solver work was done here (correctly deferred to Plan 2). The `polint.solver` provider slot and provider-order snapshots are unchanged.

## Self-Check: PASSED

- SUMMARY: `.planning/phases/48-go-rta-driver/48-01-SUMMARY.md` — FOUND
- Commits: `b91fbea6` (Task 1), `5b89665a` (Task 2), `71c4acd9` (Task 3) — all FOUND
- Key modified files (emit.go, facts.rs, lower.rs, cache_key.rs, core/mod.rs) — all FOUND
- Full `cargo test -p polint` exit code 0, 0 failures; public-surface-leak + determinism gates green; provider-order snapshots unchanged.

---
*Phase: 48-go-rta-driver*
*Completed: 2026-06-02*

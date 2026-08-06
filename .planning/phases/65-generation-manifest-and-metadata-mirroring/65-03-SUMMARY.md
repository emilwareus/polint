---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 03
subsystem: analysis-kernel
tags: [provider-outcomes, validation, dependency-closure, capability-blockers, cache-parity]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    provides: R1 crash-safe generations and R2 canonical run-manifest identity
provides:
  - Closed in-memory provider outcomes with success-only authenticated output identity
  - Plan-first hard-provider scheduling and post-validation fixed-point dependency closure
  - Language-aware and row-sensitive runtime hard-capability blockers enforced by production rule dispatch
  - Semantic outcome and cache telemetry separation with cold/warm parity proof
affects: [phase-65-r4-r6, semantic-store, provider-mirroring, capability-enforcement]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Treat provider execution as provisional until structured validation and fixed-point dependency sealing complete"
    - "Keep semantic provider truth separate from cache telemetry and presentation strings"
    - "Forward private runtime blocker sets before RuleCtx construction without widening public capability status"
    - "A selected provider that fails readiness returns neutral output and cannot enter an omission-only fallback"

key-files:
  created:
    - crates/polint/src/analysis_kernel/outcome.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/incremental/stats.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/go/semantic/provider.rs
    - crates/polint/src/eval/performance.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/semantic_graph_snapshot.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/tests/public_surface_leak.rs

key-decisions:
  - "The hard-provider table models only outputs consumed by current orchestration and remains separate from descriptive manifest inputs and DependencyIndex"
  - "Only a validated Succeeded outcome carries reusable output identity; every other terminal state carries typed stage/reason and exact blockers"
  - "Planning/setup capability support remains unchanged publicly; sealed runtime blockers are private and enforced before rule context construction"
  - "Cache hits, misses, warnings, and counters are telemetry only and cannot certify or revoke semantic provider success"
  - "Events require syntax only for languages present in the run and require scheduled call/refinement providers only when their rows can influence matching"

patterns-established:
  - "Validation-first sealing: apply owned/global validation downgrades, then propagate direct hard-provider failures to a manifest-order fixed point"
  - "Plan-first dispatch: never invoke a scheduled provider or rule when its sealed hard closure is unavailable"
  - "Authenticated identity handoff: downstream providers receive only output digests from provisionally usable producer identities, never absent sentinels"

requirements-completed: []

# Metrics
duration: 3h 25m
completed: 2026-07-29
---

# Phase 65 Plan 03: Provider Closure R3 Summary

**Every established kernel run now seals deterministic provider truth after structured validation, blocks language-aware and row-sensitive failed closures before rule dispatch, and keeps cache telemetry outside semantic identity.**

This completes only the accepted restart slice R3. R1-R3 are accepted completed
slices, while Phase 65 remains open. STORE-04, STORE-05, META-01, and META-04
remain open. R4 is next; R5 and R6 remain later slices.

## Performance

- **Duration:** ~3h 25m for the original Plan 03 implementation and review cycle
- **Started:** 2026-07-29T07:42:01Z
- **Completed:** 2026-07-29T11:07:48Z
- **Tasks:** 3
- **Implementation/test files modified:** 14
- **Original Plan 03 implementation/review delta:** 2,500 additions, 733 deletions
- **Final accepted cumulative R3 product/test delta from `c453748c`:** exactly 14 files, 2,500 additions, 831 deletions
- **Durable schema families:** 0
- **Persisted provider families:** 0

## Accomplishments

- Added a closed six-state provider outcome tracker seeded from the static manifest inventory and explicit plan selection, with typed failure stage/reason, success-only output identity, exact sorted blockers, and deterministic manifest-order sealing.
- Replaced absent-digest producer handoffs with plan-first usability checks and an audited direct hard-provider table; unavailable producers skip their consumers while independent branches continue.
- Added structured authoritative validation ownership and global fallback. Validation downgrades provisional successes before fixed-point dependency closure removes downstream reusable trust.
- Recorded replacement, Go setup, client, and lowering failures as typed orchestration evidence rather than inferring truth from diagnostics or optional digests.
- Split sealed semantic outcomes from provider cache telemetry throughout run reports, performance/observed projections, semantic-graph fixtures, and symbol-graph warm-cache proof.
- Derived deterministic runtime capability diagnostics and private blocked-rule IDs before store maintenance, then forwarded those blockers through the production runner before `RuleCtx` construction.
- Closed all six cumulative review findings: the four provider-closure defects from the original review plus WR-02's restored applicable-syntax production regression and WR-03's corrected diagnostic non-leakage assertion.
- Proved cold and warm runs have identical outcomes, identities, blockers, diagnostics, and dispatch decisions while telemetry differs; corrupt cache payloads recompute and cache-write warnings do not revoke valid success.
- Closed D-16 and D-23/D-25 through Plan 04's authenticated validation ownership and full production cold/warm projection, then passed the R3-only decision and must-have audit.
- Finished cumulative review iteration 9 clean with zero critical, warning, or informational findings while retaining all earlier review history.
- Kept public SDK, runner signatures, capability status, CLI/JSON/diagnostic contracts, durable store schema, tracking files, and `.github/workflows/ci.yml` unchanged; no sub-five-minute CI work was absorbed.

## Task Commits

Each task was committed atomically:

1. **Task 1: Define the closed provider outcome and telemetry boundary** - `3a00f148` (feat)
2. **Task 2: Seal execution truth after structured authoritative validation** - `cd5aed91` (feat)
3. **Task 3: Enforce rule skipping and prove cold/warm and public parity** - `91db8313` (feat)

**Plan metadata:** `c453748c` (docs: R3 provider outcome plan)

## Review Remediation Commits

The bounded review fixes were committed independently:

1. **Seal the applicable syntax-provider closure for Events** - `fde65ff0` (fix)
2. **Make structured validation ownership authoritative** - `a1da836e` (fix)
3. **Strengthen private-vocabulary and regression proof** - `f7f593ac` (test)
4. **Seal scheduled Events enrichments and correct reachability identity sequencing** - `28689ca7` (fix)
5. **Prevent dependency-blocked Calls from entering fallback execution** - `f86dab67` (fix)
6. **Restore the applicable-syntax production regression** - `34979d2a` (test)
7. **Correct the diagnostic non-leakage assertion** - `e8a1f800` (test)

**Historical Plan 03 review artifact:** `67fc39f9`

**Later fix-history artifacts:** `adc40fc3`, `d926c901`

**Final acceptance artifacts:** `f1d249b3` (cumulative review iteration 9,
clean) and `21346639` (R3-only reverification, passed)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/outcome.rs` - Closed outcomes, authenticated identity, hard dependencies, transition checks, validation downgrades, and fixed-point sealing.
- `crates/polint/src/analysis_kernel/mod.rs` - Plan-selected scheduling, provider projection, structured sealing, language/row-sensitive runtime capability closure, readiness-safe Calls dispatch, and cold/warm regressions.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated private exports for separate outcome and telemetry vocabulary.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Manifest-ordered sealed outcomes alongside separately ordered provider telemetry.
- `crates/polint/src/analysis_kernel/incremental/stats.rs` - Provider-keyed cache telemetry and telemetry-only aggregation.
- `crates/polint/src/analysis_kernel/validation.rs` - Deterministic structured issues with authoritative provider/family ownership, global fallback, and one-way diagnostic rendering.
- `crates/polint/src/core/mod.rs` - Typed provider failure ledger and private runtime-blocked rule wrapper.
- `crates/polint/src/go/semantic/provider.rs` - Explicit Go setup, client, and lowering outcome signals.
- `crates/polint/src/eval/performance.rs` - One-way closed-status presentation joined independently with telemetry.
- `crates/polint/src/eval/observed.rs` - Semantic outcome and telemetry projections with separate invariants.
- `crates/polint/src/eval/semantic_graph_snapshot.rs` - Success-only semantic-graph identity extraction.
- `crates/polint/src/symbol_graph/mod.rs` - Warm-cache assertions split between identity and telemetry.
- `crates/polint/src/runner/mod.rs` - Production kernel-output dispatch adapter plus mixed-plan Events/refinement blocker proof.
- `crates/polint/tests/public_surface_leak.rs` - Negative coverage for private outcome, identity, failure, and validation vocabulary.

## Decisions Made

- Kept hard dependencies as a small static orchestration audit rather than treating descriptive manifest inputs or broad dependency-index rows as authenticated producer edges.
- Retained provisional success until authoritative validation, then discarded output identity whenever validation or dependency closure changed the final state.
- Left `CapabilitySupportStatus` as the planning/setup contract and carried execution-time failure through a private sorted blocker set plus existing-code `polint/capability` diagnostics.
- Kept store maintenance after provider sealing so store enabled/disabled state cannot mutate outcomes, blockers, or policy behavior.
- Used existing deterministic provider summaries only for successful no-cache or empty-language computation; missing producer identity never becomes an absent digest.
- Filtered the Events syntax closure to languages present in `AnalysisDb`, then added call/refinement outcomes only when scheduled rows can affect Events matching; planned-absent and rowless enrichment remains optional.
- Made failed readiness terminal for selected providers. Omission-only fallback paths cannot run after `begin_provider` records dependency blocking.

## Deviations from Plan

No scope deviation. The implementation and all review remediation stayed
within the original three tasks, exactly fourteen declared product/test files,
the 2,500-line cap, the private API boundary, and zero durable schema/provider
families.

The bounded review and reverification cycle corrected six implementation or
verification defects without
expanding scope:

- **CR-01:** completed the language-applicable syntax closure for `events`.
- **WR-01:** moved validation downgrade ownership fully into structured issues.
- **CR-02:** sealed row-sensitive scheduled Events enrichments and corrected the reachability identity handoff exposed by the mixed plan.
- **CR-03:** prevented a dependency-blocked Calls provider from falling through into provider execution.
- **WR-02:** restored production-dispatch proof that only the applicable language's syntax failure blocks Events.
- **WR-03:** aligned the non-leakage assertion with the rendered diagnostic projection while preserving private structured ownership.

## Issues Encountered

- Removing absent-digest substitutions initially pushed the handwritten diff above the cap. The audited dependency table was compacted with function-local provider aliases without changing any edge; bounded remediation then used the remaining allowance and finished exactly at 2,500 additions.
- Strict clippy identified redundant terminal digest and test clones after the ownership refactor. Moving those values at their final use resolved the warnings without changing behavior.
- The original review required three remediation iterations. CR-01 and WR-01 closed first, CR-02 exposed the mixed-plan reachability/enrichment path, and CR-03 exposed blocked-provider fallback execution. Later reverification exposed WR-02 and WR-03; both closed before the final cumulative review finished clean at iteration 9.

## Verification

Original Plan 03 implementation and initial remediation evidence:

- `cargo test -p polint --lib analysis_kernel::outcome::tests --locked`: 6 passed.
- `cargo test -p polint --lib eval::performance::tests --locked`: 6 passed.
- `cargo test -p polint --lib analysis_kernel::validation::tests --locked`: 9 passed.
- `cargo test -p polint --lib analysis_kernel::tests::provider_outcomes --locked`: 3 passed, including blocked Calls after upstream execution failure, real cold/warm parity, corrupt-cache recomputation, and cache-write warning proof.
- `cargo test -p polint --lib core::tests::run_rules_skips_rules_with_runtime_provider_blockers --locked`: 1 passed.
- `cargo test -p polint --lib runner::tests::production_dispatch_blocks_events_from_rejected_scheduled_refinement --locked`: 1 passed; the renamed mixed-plan regression blocks Events and Calls while an unrelated rule executes.
- `cargo test -p polint --lib analysis_kernel::tests::provider_outcomes::blocked_calls_skip_derivation_after_upstream_execution_failure --locked`: 1 passed.
- Events-only pipeline, deep Calls-plan, provider-backed Events matching, refined-call validation, and structured validation-ownership regressions: 1 passed each.
- `cargo test -p polint --test public_surface_leak semantic_store_markers_do_not_leak_into_supported_public_surfaces --locked`: 1 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`: 1 passed; store modes preserved byte-identical JSON and exit semantics.
- Every focused target completed below sixty seconds; the slowest took 31.61 seconds including recompilation.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`: passed.
- `cargo check --workspace --all-features --locked`: passed.
- Normal pre-commit `make lint` passed workspace formatting and strict clippy across all targets and features for all three task commits and the initial five remediation commits.
- Private-name scans, removal of `ProviderOutputMeta`/semantic `ProviderStatsRow` consumers, absence of kernel `Digest::absent` handoffs, and `git diff --check`: passed.

Final independent acceptance evidence:

- Cumulative review iteration 9: clean with 0 critical, 0 warning, and 0 informational findings, committed in `f1d249b3`.
- R3-only reverification: 7/7 must-haves, 27/27 decisions, and 0 high-risk security gaps, committed in `21346639`.
- The full validation module passed 28/28; the WR-02 applicable-syntax and WR-03 non-leakage regressions passed; and all fifteen Plan 04 verification commands passed separately.
- Final scope audit from `c453748c`: exactly 14 declared implementation/test files, 2,500 additions, 831 deletions, zero durable schema families, zero persisted provider families, and no public/store/CI/tracking expansion.

## User Setup Required

None - provider outcomes and runtime blockers are crate-private and require no
external configuration. Behavioral automation is the appropriate acceptance
method for this trust-boundary work; no human UAT is needed.

## Next Phase Readiness

- R1-R3 now provide crash-safe generation truth, canonical run identity, and truthful in-memory provider closure for later persistence work.
- R4 can mirror exactly one audited provider family against the sealed success-only identity boundary.
- R5-R6 remain later slices; no broader provider persistence, metadata query, or generation lifecycle work was absorbed into R3.
- Phase 65 and STORE-04, STORE-05, META-01, and META-04 remain open.
- The deferred sub-five-minute CI redesign remains untouched and outside R3.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-29 (R3 only; phase remains open)*

## Self-Check: PASSED

All fourteen bounded implementation/test files and this summary exist, all
three task commits and seven remediation commits are in history, cumulative
review iteration 9 is clean, R3-only reverification passed 7/7 must-haves and
27/27 decisions with no high-risk security gaps, and no Phase 65, requirement,
STATE, ROADMAP, or REQUIREMENTS completion marker was written.

---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 09
subsystem: analysis-kernel
tags: [cache-identity, analysis-settings, scc-closure, invalidation, kernel-tests]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 08
    provides: Scoped semantic-provider identities and purpose-checked LayerKey constructors
provides:
  - Exact scoped identities for data-flow, evidence, reachability, refined calls, direct summaries, and SCC closure
  - Real-kernel cold/warm proof that rule behavior preserves analysis reuse while declared inputs invalidate linked providers
  - Repository-wide source guard against complete config, rule, or plan identity entering production analysis keys
affects: [phase-65-dependency-vocabulary, phase-65-store-commit-plan, phase-67-summary-reuse]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provider identity contains only typed settings, declared capability rows, and exact upstream/model/extension dependencies"
    - "Whole-kernel mutation tests pair changed complete identities with provider hit/recompute and SCC backdating assertions"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-09-SUMMARY.md
  modified:
    - crates/polint/src/analysis/data_flow/cache_key.rs
    - crates/polint/src/analysis/data_flow/provider.rs
    - crates/polint/src/analysis/evidence/cache_key.rs
    - crates/polint/src/analysis/evidence/provider.rs
    - crates/polint/src/analysis/reachability/provider.rs
    - crates/polint/src/analysis/refined_calls/cache_key.rs
    - crates/polint/src/analysis/refined_calls/provider.rs
    - crates/polint/src/analysis/summaries/provider.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/analysis/provider.rs
    - crates/polint/src/analysis/cfg/provider.rs
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis/domains/provider.rs
    - crates/polint/src/analysis/entrypoints/provider.rs
    - crates/polint/src/analysis/types/cache_key.rs
    - crates/polint/src/analysis/types/provider.rs

key-decisions:
  - "SCC closure identity is a typed AnalysisSettings digest over direct-summary settings, relevant capability state, closure budget/query version, and direct-summary/call outputs"
  - "Downstream providers consume model and extension effects through declared producer outputs rather than every InputSnapshot model, extension, or tool row"
  - "Capability support and setup transitions are exercised through cfg(test) AnalysisPlan mutation helpers without widening production or public APIs"

patterns-established:
  - "Rule-result boundary: severity, file selectors, allow/deny/max/import settings, description, and custom rule settings change complete identities but preserve shared analysis identities"
  - "Linked invalidation boundary: requested capabilities, setup/support, provider settings/budgets, model contents, extension code, and declared extension inputs invalidate only consumers"

requirements-completed: [META-01, META-04]

# Metrics
duration: 2h 6m
completed: 2026-07-13
---

# Phase 65 Plan 09: Production Analysis Identity Split Summary

**Every production policy-analysis and SCC cache identity now selects exact declared inputs, with real cold/warm kernel runs proving rule-only reuse and linked invalidation.**

## Performance

- **Duration:** 2h 6m (including interrupted-executor recovery)
- **Started:** 2026-07-13T13:56:55Z
- **Completed:** 2026-07-13T16:03:20Z
- **Tasks:** 2
- **Files modified:** 18 implementation files

## Accomplishments

- Replaced remaining complete-config contributions in data-flow, evidence, reachability, and refined-call identities with provider-scoped settings plus exact lifecycle, capability, budget, model, extension, and upstream inputs.
- Re-keyed direct summaries and SCC closure with typed analysis settings and requirements; SCC cache schema v2 now includes closure budget/query identity and direct-summary/call outputs instead of complete config, rule, or plan digests.
- Added a real-kernel cold/warm matrix covering all nine rule-only mutations and declared capability, support/setup, roots, solver budget, model, extension-code, and declared-extension-input changes.
- Added a repository-wide source assertion for provider builders, LayerKey construction, scoped cache settings, SCC closure, capability analysis identity, and snapshot requirement projections.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate remaining policy-analysis provider keys** - `02da38da` (fix)
2. **Task 2: Prove the production key split end to end** - `e4ba4c16` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/{data_flow,evidence,reachability,refined_calls}/` - Exact policy-provider parameter/output identity and paired mutation controls.
- `crates/polint/src/analysis/summaries/provider.rs` - Direct-summary scoped identity, SCC cache schema v2, and SCC cold/warm preserve/invalidate controls.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Purpose-checked direct-summary LayerKey settings and capability inputs.
- `crates/polint/src/analysis_kernel/mod.rs` - Real-cache kernel mutation matrix and production-key source audit.
- `crates/polint/src/analysis_plan.rs` - Test-only capability support/setup mutation helpers.
- `crates/polint/src/analysis/{provider,cfg/provider,calls/provider,domains/provider,entrypoints/provider,types/}` - Removed six undeclared full model/extension/tool aggregate seams.

## Decisions Made

- SCC closure reuses the direct-summary analysis-settings scope because it consumes direct-summary semantics, then adds its own fixpoint budget, query version, relevant capability projection, and upstream output identities.
- Unreferenced model, extension, and tool snapshot rows do not enter downstream provider output digests. Providers that actually read those inputs own the identity, and downstream invalidation travels through their output digest.
- Complete `ConfigIdentity`, rule identity, and plan identity remain available for run/snapshot/diagnostic boundaries and are asserted to change in rule-only controls, but they never enter shared analysis keys.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Closed the direct-summary LayerKey seam in its canonical private key module**

- **Found during:** Task 2 production-key audit.
- **Issue:** The action required the last direct-summary LayerKey to reject complete config identity, but `analysis_kernel/incremental/keys.rs` was omitted from the frontmatter file list.
- **Fix:** Added typed analysis-settings and analysis-requirements inputs and routed the constructor through `new_with_analysis_settings`.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/keys.rs`
- **Verification:** The specialized-constructor source guard and both real-kernel tests pass.
- **Committed in:** `e4ba4c16`

**2. [Rule 2 - Missing Critical] Removed six undeclared aggregate input seams found by the repository-wide audit**

- **Found during:** Task 2 source assertion and unreferenced-sibling controls.
- **Issue:** Semantic MIR, CFG, calls, abstract domains, entrypoints, and type/value/alias still folded every model, extension, and tool snapshot row into their output identity despite not reading those inputs.
- **Fix:** Removed those aggregate rows and changed the shared mutation controls to require preservation for unreferenced model/extension/tool changes.
- **Files modified:** `crates/polint/src/analysis/provider.rs`, `crates/polint/src/analysis/cfg/provider.rs`, `crates/polint/src/analysis/calls/provider.rs`, `crates/polint/src/analysis/domains/provider.rs`, `crates/polint/src/analysis/entrypoints/provider.rs`, `crates/polint/src/analysis/types/cache_key.rs`, `crates/polint/src/analysis/types/provider.rs`
- **Verification:** Nine declared-input provider controls and the real-kernel model/extension sibling assertions pass.
- **Committed in:** `e4ba4c16`

**3. [Rule 3 - Blocking] Added test-only AnalysisPlan mutation seams for real support/setup transitions**

- **Found during:** Task 2 whole-kernel matrix implementation.
- **Issue:** The matrix needed to exercise capability support and setup transitions through a valid plan without exposing or hand-editing production internals.
- **Fix:** Added two `cfg(test)` `pub(crate)` helpers that rebuild canonical plan identity after the mutation.
- **Files modified:** `crates/polint/src/analysis_plan.rs`
- **Verification:** The declared-input kernel test proves both transitions invalidate calls and SCC closure; strict visibility/lint gates pass.
- **Committed in:** `e4ba4c16`

**4. [Rule 1 - Bug] Replaced delivery-history wording in touched shipped comments and logs**

- **Found during:** Task 2 comment-policy audit.
- **Issue:** Existing touched kernel comments and trace messages described delivery phases and decision identifiers rather than enduring execution behavior.
- **Fix:** Reworded them in terms of execution stages, ordering, and consumed inputs.
- **Files modified:** `crates/polint/src/analysis_kernel/mod.rs`
- **Verification:** Added-line comment audit finds no phase, plan, task, milestone, or decision-number wording.
- **Committed in:** `e4ba4c16`

---

**Total deviations:** 4 auto-fixed (1 missing-critical dependency audit, 2 blocking private seams, 1 shipped-comment bug).
**Impact on plan:** Expansion stayed private and bounded to the canonical direct-summary key, six audited aggregate seams, and a `cfg(test)` plan helper. No SDK, runner, CLI, config, or public output contract changed.

## Issues Encountered

- The literal plan filter `analysis::summaries::provider::tests` selects zero tests because the file uses `direct_summaries_provider` and `scc_closure_provider` named test modules. The effective `analysis::summaries::provider` filter ran all 6 provider tests successfully.
- Strict Clippy found one redundant test-fixture clone during recovery; moving the final owned `LoadedConfig` value removed it, after which `make lint` passed.

## User Setup Required

None - all changes are private analysis-kernel behavior.

## Verification

- Real kernel: `rule_only_changes_preserve_analysis_hits` and `declared_analysis_inputs_invalidate_linked_providers` passed serially.
- Policy providers: data-flow 42, evidence 31, reachability 9, and refined calls 35 tests passed.
- Summary providers: effective module filter passed all 6 tests; the literal plan filter selected 0 as documented above.
- Declared-input controls: 9 scoped-provider controls and 4 policy-provider controls passed.
- Repository source audit found only test names/fixture fields among complete rule-digest search results; no production analysis key consumed complete config, rule, or plan identity.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `make lint`: passed, including workspace/all-target/all-feature Clippy with warnings denied.
- Task 2 commit hook reran `make lint` and passed.

## Next Phase Readiness

- Production analysis identity is scoped and ready for the typed dependency-input vocabulary in Plan 10.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

Both task commits exist; all 18 implementation files and this summary exist; focused tests, real-kernel mutation controls, source audits, formatting, all-feature compilation, strict Clippy, and commit hooks pass.

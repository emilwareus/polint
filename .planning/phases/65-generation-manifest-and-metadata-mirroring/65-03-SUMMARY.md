---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 03
subsystem: analysis-kernel
tags: [config-identity, analysis-plan, capabilities, provider-settings, digests, incremental-store]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 02
    provides: Typed provider/query semantic projections and canonical digest purposes
provides:
  - Closed provider-scoped analysis-setting identities for every current provider family
  - Typed requested-capability analysis requirements separated from full rule behavior
  - A production plan-aware input-snapshot constructor without a v1 wire-schema change
affects: [phase-65-layer-metadata, phase-65-input-snapshot-migration, phase-65-store-commit-plan]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Full manifest identity and scoped analysis identity are built from separate deterministic projections"
    - "Production constructors accept semantic source objects while digest-only seams remain test-only"

key-files:
  created: []
  modified:
    - crates/polint/src/cache/keys.rs
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "AnalysisSettingsScope is a closed per-provider vocabulary, and each scope hashes only explicitly declared configuration inputs"
  - "Capability analysis requirements include capability, language, support, setup, and policy-query version but exclude requester and rule-presentation metadata"
  - "The complete config, rule, and plan digests remain available for store manifests and diagnostics"
  - "The input snapshot remains polint-input-snapshot-1 while production begins carrying the borrowed AnalysisPlan"

patterns-established:
  - "Scoped setting projection: rule-only configuration changes preserve every provider analysis-setting digest"
  - "Dual plan identity: complete behavior identity and analysis dependency identity are mutation-tested independently"
  - "Plan-aware boundary: derive typed capability/settings sources from the same plan/config accepted by production"

requirements-completed: [STORE-04, META-01, META-04]

# Metrics
duration: 25min
completed: 2026-07-12
---

# Phase 65 Plan 03: Scoped Configuration and Capability Identity Summary

**Full manifest/rule identity is now distinct from provider-scoped settings and typed capability-analysis identity, and production snapshot construction receives the real analysis plan without changing the v1 wire schema.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-12T19:38:24Z
- **Completed:** 2026-07-12T20:02:55Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added a closed 23-scope `AnalysisSettingsScope` vocabulary covering every current provider manifest and deterministic per-scope hashes built with the existing typed digest machinery.
- Preserved complete `config_hash` and rule/plan identities while proving that severity, file filters, rule limits, forbidden imports, descriptions, and arbitrary custom settings do not contaminate provider analysis identities.
- Added typed requested-capability snapshots and a canonical analysis-requirements digest over capability, language, support, setup, and policy-query version only.
- Added `InputSnapshot::from_run_inputs_with_plan` and routed `AnalysisKernel::run` through it so capability/settings sources derive from the same `LoadedConfig` and `AnalysisPlan` used by the run.
- Kept the serialized input snapshot at `polint-input-snapshot-1`; the capability-erasing digest constructor is now cfg(test)-only and used by explicitly empty-plan fixtures.

## Task Commits

1. **Task 1: Define full and provider-scoped configuration identities** - `cf700865` (feat)
2. **Task 2: Add a semantically complete plan-aware constructor seam** - `1dfb74d0` (feat)

## Files Created/Modified

- `crates/polint/src/cache/keys.rs` - Closed provider scopes, deterministic scoped setting projections, and exact-scope mutation tests.
- `crates/polint/src/analysis_plan.rs` - Typed setup/capability snapshots, analysis-requirements identity, complete plan identity, and isolation tests.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Canonical identity-source rows, plan-aware constructor, v1 schema proof, and test-only legacy seam.
- `crates/polint/src/analysis_kernel/mod.rs` - Sole production snapshot call now passes the borrowed `AnalysisPlan`.

## Decisions Made

- Modeled provider settings as a closed enum rather than arbitrary labels so adding a provider requires an explicit scope and projection decision.
- Kept effective solver and object-model behavior in their owning scopes; unrelated provider scopes remain byte-stable when those knobs change.
- Excluded requesting rule IDs and all rule presentation/options fields from capability analysis identity while retaining them in store-facing/full behavior identity.
- Included rendered reason, hint, documentation, and setup detail in the complete plan digest but excluded them from the analysis-requirements digest.
- Deferred serialized capability/settings rows until the planned snapshot migration; this plan introduces the semantic source seam without pretending the v1 payload contains new fields.

## Deviations from Plan

### Process Deviations

**1. Preparatory Task 2 integration landed in the Task 1 commit**

- **Found during:** Task 1 strict-lint verification.
- **Issue:** The new crate-private identity-source APIs needed a production consumer to satisfy the workspace's dead-code policy before the plan-aware constructor was introduced.
- **Resolution:** `cf700865` included a temporary production derivation and kind assertions in `analysis_kernel/mod.rs`. Task 2 then moved that derivation into `from_run_inputs_with_plan` and completed the intended production routing in `1dfb74d0`.
- **Impact:** The two requested task commits remain present, but the first commit contains a preparatory change in one Task 2 file. No history was rewritten, and final behavior/files match the plan.

---

**Total deviations:** 1 process deviation (task-commit composition only)
**Impact on plan:** No scope, API, schema, or product-behavior deviation.

## Issues Encountered

- The plan's exact `analysis_kernel::incremental::input_snapshot::tests` filter selected zero tests because this module organizes coverage under named `source_config_rule_model_extension` and `lifecycle` submodules. The exact command still passed, and those concrete suites were run separately with 8 and 10 passing tests respectively.

## User Setup Required

None - these are private deterministic identity and constructor changes with no new configuration, service, or public SDK requirements.

## Verification

- Cache-key tests: 18 passed, including all 23 provider scopes and exact relevant/unrelated setting mutations.
- Analysis-plan tests: 22 passed, including full-rule isolation and capability/language/support/setup mutations.
- Input snapshot source/config/rule/model/extension tests: 8 passed, including typed source rows and unchanged v1 JSON shape.
- Input snapshot lifecycle tests: 10 passed.
- Semantic-store and output-parity tests: 3 passed.
- Exact planned input-snapshot `tests` filter: passed with 0 selected tests; concrete named suites above provide the coverage.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`: passed directly and through both task commit hooks, including every workspace example rule crate.
- Acceptance audit: `AnalysisKernel::run` passes `input.plan`; the legacy constructor is cfg(test)-only; non-test compilation proves it has no production caller; the schema constant remains `polint-input-snapshot-1`.
- Source/API audit: no delivery-history chronology was added to shipped source, all new runtime surfaces are crate-private, and no public SDK/re-export boundary widened.
- Threat review: closed scope projections prevent full-config cache contamination; typed support/setup states prevent opaque identity spoofing; no source text, environment value, absolute path, network, authentication, SQL, file-write, or payload-body surface was added.

## Next Phase Readiness

- Snapshot migration can serialize the already-derived capability/settings sources without reopening identity semantics.
- Layer keys can consume precise provider/capability dependencies while store manifests retain complete diagnostic identity.
- The cfg(test) digest-only constructor is isolated for removal during the planned caller migration.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All four planned source files and this summary exist; task commits `cf700865` and `1dfb74d0` are present; every focused mutation, schema, lifecycle, semantic-store parity, compilation, formatting, strict-Clippy, constructor-boundary, visibility, and threat check listed above passes.

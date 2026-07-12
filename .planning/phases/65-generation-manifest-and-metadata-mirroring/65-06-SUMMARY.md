---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 06
subsystem: analysis-kernel
tags: [input-snapshot, identities, capabilities, serialization, determinism]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 05
    provides: Plan-aware snapshot construction at every production and fixture call site
provides:
  - InputSnapshot v2 with typed workspace, full-config, scoped-setting, and requested-capability rows
  - Purpose-specific full semantic snapshot digest that excludes hints and telemetry
  - Strict deterministic v2 codecs with future, unknown-label, and wrong-purpose rejection
affects: [phase-65-layer-identities, phase-65-dependency-vocabulary, phase-65-store-commit-plan]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Durable snapshots retain full mirror identity and separate provider-scoped dependency identity"
    - "Canonical aggregates hash labeled semantic sub-rows and exclude non-semantic presentation or runtime state"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/eval/observed.rs
    - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml

key-decisions:
  - "InputSnapshot v2 directly owns opaque workspace/full-config identities, all scoped analysis-setting rows, typed requested-capability rows, and the rule-independent requirements identity"
  - "The full snapshot digest includes every semantic row through labeled sub-digests but excludes mtime hints, rendered details, order, counters, durations, and timestamps"
  - "V2 decoding is strict: missing/unknown fields, future schemas, unknown typed labels, mismatched config rows, and wrong-purpose identity digests fail closed"

patterns-established:
  - "Mirror/scoped split: requester IDs and full rule behavior remain durable truth while analysis dependency digests omit rule-only behavior"
  - "Explicit empty fixtures: capability-free snapshot literals name typed empty settings/capabilities and the canonical absent requirements identity"

requirements-completed: [STORE-04, META-01, META-04]

# Metrics
duration: 22min
completed: 2026-07-12
---

# Phase 65 Plan 06: Input Snapshot v2 Summary

**InputSnapshot v2 now publishes complete typed manifest identity alongside precise provider settings and capability-analysis dependencies, with strict codecs and deterministic semantic aggregation.**

## Performance

- **Duration:** 22 min
- **Started:** 2026-07-12T20:54:53Z
- **Completed:** 2026-07-12T21:16:40Z
- **Tasks:** 1
- **Files modified:** 11

## Accomplishments

- Bumped the internal wire contract to `polint-input-snapshot-2` and serialized opaque workspace/full-config identities, 23 sorted provider-scoped settings, typed requested capabilities, mirror-only requester/rule behavior, and the rule-independent analysis requirements identity.
- Added a purpose-separated `InputSnapshot` semantic digest over labeled canonical sub-rows. File mtime hints, component details, row order, counters, durations, timestamps, source bodies, and machine-local roots cannot affect it.
- Added strict v2 round trips that reject future schemas, unknown fields/providers/statuses, mismatched config identity, and wrong digest purposes without defaults or compatibility fallbacks.
- Updated all six direct literals, eval invariants, the checked-in cache fixture, and CLI private-vocabulary assertions atomically while leaving data-flow/evidence/type/refined-call production key builders field-selective.

## Task Commits

1. **Task 1: Add v2 identity rows and update every literal/schema consumer** - `4db5c5d9` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - V2 fields, strict codecs, canonical aggregate, and mutation/privacy fixtures.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Validated deserialization for opaque workspace/config identities and an infallible complete-config constructor.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated crate-private re-exports for the new snapshot rows.
- `crates/polint/src/analysis/evidence/cache_key.rs` - Explicit capability-free v2 test identity fields.
- `crates/polint/src/analysis/evidence/provider.rs` - Explicit capability-free v2 provider fixture.
- `crates/polint/src/analysis/extensions/provider.rs` - Two explicit capability-free v2 extension fixtures.
- `crates/polint/src/analysis/types/cache_key.rs` - Explicit capability-free v2 type-cache fixture.
- `crates/polint/src/eval/performance.rs` - Explicit capability-free v2 performance fixture.
- `crates/polint/src/eval/observed.rs` - Full-pipeline invariants for typed workspace/config/settings/capability identity.
- `crates/polint/tests/cli.rs` - V2 and identity-row public no-leak markers.
- `tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml` - V2 full-pipeline identity expectations.

## Decisions Made

- Reused the opaque `WorkspaceIdentity` and `ConfigIdentity` types instead of serializing unvalidated generic digests, and validated their digest purposes during decode.
- Kept full config/rule components and capability rule behavior in the snapshot for manifest truth while preserving independent analysis-settings and requirements identities for later provider-scoped invalidation.
- Built the full aggregate from purpose-labeled row digests and an order-insensitive outer aggregate, avoiding delimiter ambiguity and insertion-order dependence.
- Kept mtime presence serialized as a hint for diagnostics but structurally absent from semantic digest construction.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added typed identity deserialization in the canonical digest module**

- **Found during:** Task 1 v2 round-trip implementation.
- **Issue:** The plan's new `WorkspaceIdentity` and `ConfigIdentity` fields were serialize-only, so a strict typed v2 round trip could not compile or reject wrong-purpose wire digests. The plan's file list did not include their defining module.
- **Fix:** Expanded scope by one file, `incremental/digest.rs`, adding validated `Deserialize` implementations and focused wrong-kind rejection fixtures. No other unplanned file was added.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/digest.rs`
- **Verification:** Both opaque identities round-trip through v2, and config-as-workspace plus settings-as-config fixtures fail with typed purpose errors; strict Clippy passes.
- **Committed in:** `4db5c5d9`

---

**Total deviations:** 1 auto-fixed (1 blocking implementation dependency)
**Impact on plan:** One private canonical-codec file was necessarily added; no public API, product behavior, provider dependency scope, or persistence boundary expanded.

## Issues Encountered

- The plan's exact `analysis_kernel::incremental::input_snapshot::tests` filter selects zero tests because the module uses named test submodules. The exact command passed, and the concrete `analysis_kernel::incremental::input_snapshot` suite passed 23 tests.

## User Setup Required

None - the schema and identity migration is entirely private and requires no external service or configuration action.

## Verification

- Input snapshot v2 codec, mutation, determinism, and privacy suite: 23 passed.
- Opaque identity codec/purpose suite: 11 passed, including both wrong-kind decode fixtures.
- Literal consumer suites: evidence cache 3 passed; evidence provider 6 passed; extensions provider 4 passed; type cache key 4 passed.
- Eval performance compatibility: 6 passed; observed full-pipeline cache/input invariants: 4 passed.
- CLI internal-vocabulary/public-JSON compatibility: 1 passed with byte-identical repeated public JSON.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `make lint`: passed formatting and strict workspace/all-target/all-feature Clippy with warnings denied; the task commit hook reran the same gate successfully.
- Acceptance audit: all schema pins are v2 except the explicit future-v99 rejection fixture; exactly six literals provide every new field; no v2 serde default exists; field-selective provider consumers use none of the new aggregate identity shortcuts; semantic digest source contains no mtime/detail/counter/duration/timestamp input.
- Threat review: root and source sentinels do not serialize; opaque identities discard roots; source text remains digest-only; strict typed decoders reject spoofed provider/status/digest-purpose values; public CLI JSON is unchanged.

## Next Phase Readiness

- Scoped layer/cache identity plans can consume the exact `analysis_settings` and requested-capability rows without using the full config or full snapshot as a convenience key.
- Later store commit planning can mirror the complete v2 snapshot and consume its purpose-specific semantic digest.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All eleven implementation files and this summary exist; task commit `4db5c5d9` is present; every planned and concrete focused suite, v2/future/unknown/wrong-purpose codec fixture, acceptance audit, all-feature compilation, formatting, strict Clippy, public-JSON, privacy, and field-selective-consumer gate listed above passes.

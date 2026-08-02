---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 05
subsystem: analysis-kernel
tags: [metrics-identity, semantic-store, provider-mirror, sqlite, tamper-refusal]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    provides: R1 generation lifecycle, R2 run manifests, and R3 sealed provider outcomes
provides:
  - Canonical source/function inputs and produced-output identity for `polint.metrics`
  - Schema-v4 relational metadata mirror for exactly one provider family
  - Atomic manifest-plus-provider publication with authenticated reopen and private exact matching
  - R4 mutation, rollback, bounds, tamper, cache-parity, and privacy exit proof
affects: [phase-65-r5-r6, semantic-store, metrics-cache, provider-metadata]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "One normalized typed projection drives cache identity, dependency edges, output identity, durable encoding, and matching"
    - "Provider metadata is published and read back in the existing immediate generation transaction"
    - "SQLite cells and catalog state remain untrusted until bounded typed validation and witness recomputation succeed"

key-files:
  created:
    - crates/polint/src/analysis_kernel/metrics_projection.rs
    - crates/polint/src/analysis_kernel/store/provider_mirror.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/outcome.rs
    - crates/polint/src/analysis_kernel/store/generation.rs
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/src/metrics.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/tests/public_surface_leak.rs

key-decisions:
  - "Only canonical source and non-synthetic function rows are metrics inputs; config, rules, syntax identities, tools, models, extensions, telemetry, calls, is_test, and is_exported remain excluded"
  - "Only Succeeded carries identity and source/function dependencies; all legal non-success states persist as non-reusable history"
  - "Provider matching authenticates workspace ownership but intentionally does not substitute full run-manifest equality for metrics dependency equality"
  - "Normal AnalysisKernel execution remains maintenance-only; publication, reads, and matching stay private and are not wired into production reuse"
  - "Catalog SQL must pass exact-cardinality, TEXT-storage, and declaration-relative byte bounds before Rust decoding"
  - "Non-success provider rows require zero source/function counts even when SQLite CHECK enforcement was bypassed"

patterns-established:
  - "Success-only durable metadata: exact identity and consumed inputs exist together or not at all"
  - "Dense ordinal plus exact member/count/witness validation prevents reordered, missing, duplicated, or cross-owned rows from becoming trusted truth"

requirements-completed: []

# Metrics
duration: 2h 8m
completed: 2026-08-02
---

# Phase 65 Plan 05: R4 Metrics Provider Metadata Mirror Summary

**`polint.metrics` metadata can now be atomically written, closed, reopened, exactly validated and matched, while malformed or tampered state fails closed and normal analysis performs no persistence or reuse.**

This accepts only R4. R5-R6, Phase 65, STORE-04, STORE-05, META-01, and
META-04 remain open.

## Performance

- **Duration through review fix:** 2h 8m
- **Started:** 2026-08-02T16:46:08+02:00
- **Initial summary completed:** 2026-08-02T18:25:30+02:00
- **Review fix completed:** 2026-08-02T18:53:54+02:00
- **Tasks:** 3
- **Review-fix commits:** 1
- **Product/test files modified:** 12 of 13 allowed
- **Cumulative product/test delta from `013ff41c3`:** 2,281 additions, 386 deletions
- **New durable schema families:** 1 relational metrics-provider mirror family
- **Tables in that family:** 5
- **Persisted provider families:** 1 (`polint.metrics`)

## Accomplishments

- Added one canonical, normalized, multiplicity-preserving metrics projection
  that validates source/function relationships and all three produced metric
  families before deriving identity.
- Removed config and upstream syntax identity from the metrics-only cache key
  and dependency edges, added explicit absent config/tool/model/extension
  slots, and made cold, warm, and cache-disabled output identities equal.
- Added schema v4 with exact static manifest, sealed outcome, identity,
  blockers, source rows, and function rows across one five-table relational
  family owned by generation IDs.
- Extended the existing immediate publication transaction to write and
  boundedly read back the run manifest and provider mirror before completion
  and selection. All 23 before/after failure seams preserve prior truth.
- Preserved and refused populated v3 stores without mutation; empty v0-v3
  stores migrate transactionally to v4.
- Proved a real TypeScript kernel success survives reserve, publish, close,
  reopen, active read, and `MetricsMatch::Exact`. All six legal non-success
  forms across five statuses round-trip without identity/dependencies and
  always miss reuse.
- Added consumed-input invalidation and excluded-state preservation matrices,
  53 relational/semantic/storage-class/bounds/catalog/FK tamper cases, dense
  ordinal validation, and private-vocabulary leak coverage.
- Closed the independent review's two trust-boundary findings by bounding
  `sqlite_master.sql` before Rust decoding and independently enforcing zero
  dependency counts for every non-success outcome in both the reader and
  current-schema validation.
- Kept the public SDK, runner signature, CLI/config/output/docs/examples,
  generated skill text, CI workflow, and normal-kernel behavior unchanged.

## Implementation and Review-Fix Commits

The three planned tasks and subsequent review remediation were committed
atomically with the normal formatting and strict workspace Clippy hook:

1. **Task 1: Canonicalize metrics identity and dependency inputs** - `5eb94d3a` (`feat`)
2. **Task 2: Add schema-v4 metrics mirror and atomic publication** - `379bb942` (`feat`)
3. **Task 3: Prove R4 exit conditions and privacy** - `df0c80ea` (`test`)
4. **Review remediation: Close catalog-bound and non-success row-shape findings** - `4b925a08` (`fix`)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/metrics_projection.rs` - Canonical
  source/function inputs, validated produced metric output, sealed projection,
  and hard-dependency-authenticated blockers.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Narrow metrics key
  containing only consumed source/function inputs and explicit absences.
- `crates/polint/src/metrics.rs` - Shared canonical key, dependency, payload
  validation, and produced-output digest path for cold/warm/disabled modes.
- `crates/polint/src/analysis_kernel/mod.rs` - Fallible canonical metrics
  derivation while retaining the sole production store call to `maintain`.
- `crates/polint/src/analysis_kernel/outcome.rs` - Closed durable codecs and a
  constructor that revalidates sealed outcome parts.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Exact schema-v4
  authentication, bounded catalog SQL, independent non-success count checks,
  one provider family, and empty-v3-only migration.
- `crates/polint/src/analysis_kernel/store/provider_mirror.rs` - Bounded SQL
  encoder/decoder, exact static metadata, dense ordinals, witnesses, and typed
  canonical reconstruction, including pre-child-read non-success count
  rejection.
- `crates/polint/src/analysis_kernel/store/generation.rs` - One atomic
  manifest-plus-provider transaction and authenticated active read/match.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Private typed publication,
  active projection, and metrics match facade.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Real success, all legal
  non-success forms, mutation matrices, 23 rollback seams, 53 tamper-matrix
  cases, a 100,000-byte catalog regression, and coherent non-success dependency
  tamper coverage.
- `crates/polint/src/runner/mod.rs` - Cold/warm/cache-disabled semantic parity
  plus retained corrupt-cache and write-warning proof.
- `crates/polint/tests/public_surface_leak.rs` - Negative coverage for all new
  projection, publication, match, schema, blocker, and dependency vocabulary.

## Decisions Made

- Stored only metadata needed to authenticate `polint.metrics`; metric facts,
  FactMeta, validation events, cache statistics, full input snapshots, and
  generic dependency/query/summary structures remain absent.
- Kept one forward canonical source/function representation. Cache keys,
  dependency edges, persistence, reopen, and matching derive from it rather
  than maintaining independent semantic copies.
- Required dense ordinals for blocker/source/function rows. Exact duplicate
  function values remain meaningful multiplicity and are therefore retained.
- Bounded catalog SQL and every decoded header/child string, count, row, and
  aggregate before Rust allocation; schema checks are defense in depth rather
  than trusted input validation.
- Authenticated dependency blockers against the actual hard dependencies of
  `polint.metrics`, not merely against the wider provider inventory.
- Returned `SemanticMiss` for non-success and any consumed-input difference;
  full run config remains separate manifest truth and does not poison the
  narrower provider match.

## Deviations from Plan

No scope deviation. The implementation used exactly three tasks, 12 declared
product/test paths, 2,281 additions, one schema family, and one persisted
provider family. No CI, state, roadmap, requirements, public surface, or
normal-run persistence/reuse expansion occurred.

The implementation review loop found and fixed three in-scope trust-boundary
gaps before the initial summary:

- sparse blocker/source/function ordinals could preserve decoded value order;
  dense ordinal preflight now rejects them;
- optional provider-header strings were type-checked but not byte-bounded
  before decode; every optional cell is now bounded;
- blockers were accepted from any known provider; they are now restricted to
  the actual hard dependencies of `polint.metrics`.

The subsequent independent cumulative review found one Critical and one
Warning at `b0ad9c18`:

- `sqlite_master.sql` was decoded into an owned `String` and normalized before
  its storage class or byte length was bounded;
- coherent source/function rows on non-success outcomes could pass count checks
  and then be silently discarded during reconstruction.

Fix commit `4b925a08` retained the same three-file store scope. It now
authenticates exact catalog cardinality, TEXT storage, and a byte ceiling equal
to the expected declaration plus 4,096 formatting bytes before decoding, with
the bounded predicate repeated on the decode query. It also decodes outcome
status immediately, rejects non-`Succeeded` headers with nonzero source or
function counts before child reads, and restates that invariant in current-v4
row validation. The independent re-review of the full cumulative range is
clean: 0 Critical, 0 Warning, and 0 informational findings.

## Verification

All required commands were run separately against the final implementation:

- Metrics projection: 2/2 passed; 0.63s wall.
- Metrics module: 29/29 passed; 0.95s wall.
- Metrics key: 1/1 passed; 22.58s wall including compilation.
- Closed outcomes: 6/6 passed; 0.14s wall.
- Migrations: 22/22 passed; 0.14s wall on the final review fix.
- Oversized catalog SQL regression: 1/1 passed; 17.64s wall including
  compilation.
- Legal non-success dependency-row regression: 1/1 passed; 0.41s wall.
- Provider mirror exit suite: 5/5 passed; 1.14s wall.
- Full semantic-store suite: 65/65 passed; 1.98s wall.
- Generation lifecycle and 23 rollback seams: 13/13 passed; 1.14s wall.
- Cold/warm/cache-disabled runner parity: 1/1 passed; 0.47s wall.
- Store-mode JSON/exit parity: 1/1 passed; 0.48s wall.
- Public-surface leak proof: 7/7 passed; 16.85s wall including compilation.
- `cargo fmt --all -- --check`: passed; 1.08s wall.
- Strict workspace/all-target/all-feature Clippy: passed; 14.25s wall.
- Workspace/all-target/all-feature check: passed; 5.53s wall.
- Diff, exact allowed-file, addition/file-cap, protected-file,
  forbidden-persistence, one-family/one-provider, visibility, and
  maintenance-only wiring audits: passed; final product/test scope is 12 files
  with 2,281 additions and 386 deletions.

Every required focused target completed below sixty seconds. Tests use only
isolated temp repositories and local SQLite state; no network, sleep, process
environment mutation, or global serialization was added.

## Independent Gate Status

- **Initial cumulative R4 code review:** ISSUES FOUND at `b0ad9c18`; one
  Critical for unbounded catalog SQL decoding and one Warning for malformed
  non-success dependency rows.
- **Independent remediation re-review:** CLEAN at `4b925a08`; 0 Critical, 0
  Warning, and 0 informational findings. The final pass covered canonical
  identity, schema/migration exactness, publication rollback, active selection,
  bounds, tamper handling, privacy, and budget.
- **Fresh R4-only verifier:** PASS, D-01 through D-46 (46/46). The verifier
  certifies only provider metadata write/reopen/exact-match/tamper refusal for
  `polint.metrics`; it does not certify R5, R6, Phase 65, STORE-04, STORE-05,
  META-01, or META-04.

## User Setup Required

None. The mirror, publication, and matching seams are crate-private and are
not enabled in normal analysis.

## Next Phase Readiness

- R5 may consider the next separately audited provider family on top of the
  accepted R1-R4 contracts.
- R6 continues to own production publication, measured reuse, and any
  user-facing lifecycle behavior.
- Phase 65 and STORE-04, STORE-05, META-01, and META-04 remain open.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-08-02 (R4 accepted only; Phase 65 remains open)*

## Self-Check: PASSED - R4 ACCEPTED

The 12 authorized product/test files and this summary exist; all three task
commits and review-fix commit are present; all required tests, format, strict
Clippy, workspace check, diff, scope, persistence, privacy, and
maintenance-only audits pass; the independent re-review is clean; the R4-only
verifier passes all 46 decisions; and no CI, STATE, ROADMAP, REQUIREMENTS,
phase-completion, or requirement-completion marker changed.

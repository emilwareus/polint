---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 19
subsystem: database
tags: [sqlite, semantic-store, privacy, parity, performance]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 18
    provides: Validated semantic-store control plane with exact persisted invalidation and policy-neutral outcomes
provides:
  - Exhaustive private-vocabulary leak probes with an unchanged 115-name SDK prelude
  - Byte-identical check and review behavior across every semantic-store state
  - Real enabled-store byte, diagnostics-parity, peak-RSS, and cold-time gates
  - Compact deterministic metadata planning and SQLite publication within the locked regression boundary
affects: [phase-65-audit, semantic-store-payload-work, metadata-performance]

# Tech tracking
tech-stack:
  added: [fnv 1.0.7, lz4_flex 0.13.1]
  patterns:
    - "Compute canonical fingerprints from length-prefixed fragments without allocating intermediate digest strings"
    - "Prepare owned canonical rows before parallel finalization so large source stores can be dropped before persistence"
    - "Keep performance-only dependency optimization in the test profile while preserving production behavior"

key-files:
  created: []
  modified:
    - crates/polint/tests/public_surface_leak.rs
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/store/commit_plan.rs
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/generation.rs
    - crates/polint/src/eval/bench/runner.rs
    - crates/polint/src/eval/bench/gate.rs

key-decisions:
  - "The 1.20 peak-RSS ratio, 1.25 cold-time ratio, 16 MiB RSS floor, and 50 ms cold floor remain immutable release gates"
  - "Stable fact keys may be compressed internally, but their decoded values, canonical ordering, fingerprints, and public behavior must remain identical"
  - "Type/value/alias identity consumes declared lifecycle and upstream provider inputs, not unrelated raw tool or extension snapshots"
  - "SQLite publication uses deterministic ordinals, batched dependency writes, 16 KiB pages, and no close-time checkpoint on new private stores"

patterns-established:
  - "Staged compaction: canonicalize and release ownership first, then fingerprint/compress rows in parallel with independent run finalization"
  - "Performance truth: measure a real complete enabled generation, require nonzero persisted bytes, and gate the same diagnostics digest"
  - "Private-boundary proof: pair every forbidden marker family with a working negative control and scan real public artifacts"

requirements-completed: [STORE-04, STORE-05, META-01, META-04]

# Metrics
duration: 20h 9min
completed: 2026-07-14
---

# Phase 65 Plan 19: Semantic Store Regression Boundary Summary

**The private semantic store now preserves byte-identical policy behavior and deterministic identities while a real 120,352,592-byte enabled generation stays inside the locked memory and cold-start budgets.**

## Performance

- **Duration:** 20h 9 min
- **Started:** 2026-07-14T01:02:24Z
- **Completed:** 2026-07-14T21:11:21Z
- **Tasks:** 1
- **Files modified:** 34 implementation files

## Accomplishments

- Expanded the public-surface suite across SDK, runner, CLI, crate root, documentation, examples, generated skill text, and real check/review output. Every precise private marker family has a working negative control, the outside-consumer probe compiles, and `ALLOWED_PRELUDE` remains exactly 115 names.
- Reproved byte-identical JSON and exit semantics for disabled, committed, mismatch, recovery, failed, pending, busy, future, invalid, corrupt, and unsafe store states. Telemetry-only counter, status, duration, and timestamp changes remain outside semantic identities, selection, and invalidation.
- Made the enabled benchmark publish a real complete metadata generation, report 120,352,592 nonzero store bytes, and compare the exact diagnostics digest before evaluating resource budgets.
- Reduced peak ownership and persistence overhead through direct deterministic fingerprints, compact stable fact keys, staged fact canonicalization, owned draining, batched dependency writes, ordinal fact rows, and direct logical-size accounting.
- Reduced cold SQLite publication cost with 16 KiB pages, append-oriented `WITHOUT ROWID` fact storage, deferred WAL checkpointing, and optimized test-profile builds of the hashing, codec, and SQLite dependencies without weakening any threshold.

## Task Commits

Each task was committed atomically:

1. **Task 1: Enforce private surface, parity, determinism, performance, and full verification** - `b68d9409` (perf)

## Files Created/Modified

- `crates/polint/tests/public_surface_leak.rs` - Precise semantic-store marker families, negative controls, public artifact scans, and the locked 115-name allowlist assertion.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Allocation-light FNV fingerprints and canonical unordered aggregation with equivalence tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Compact stable-key codec, staged fact-row preparation/finalization, and digest-equivalence coverage.
- `crates/polint/src/analysis_kernel/mod.rs` - Owned validated handoff that overlaps independent canonical finalization only after large metadata owners are released.
- `crates/polint/src/analysis_kernel/incremental/{dependency_index,run_report}.rs` - Compact key handling and lower-allocation canonical dependency/run construction.
- `crates/polint/src/analysis_kernel/store/commit_plan.rs` - Owned canonical planning, deterministic ordinals, compact row accounting, and prepared lookup structures.
- `crates/polint/src/analysis_kernel/store/connection.rs` - New-store page sizing and WAL checkpoint policy with locked connection-policy tests.
- `crates/polint/src/analysis_kernel/store/generation.rs` - Batched deterministic publication, owned draining, direct logical sizes, and compact-key reconstruction.
- `crates/polint/src/analysis_kernel/store/{migrations,schema,tests}.rs` - Ordinal `WITHOUT ROWID` fact storage plus round-trip, lifecycle, and invalidation coverage.
- `crates/polint/src/eval/bench/{runner,gate}.rs` - Isolated real enabled-store measurement, nonzero byte reporting, diagnostics parity, and immutable resource gates.
- `Cargo.toml`, `Cargo.lock`, and `crates/polint/Cargo.toml` - Internal fingerprint/codec dependencies and narrow test-profile optimization.

## Decisions Made

- Kept the supported rule-author surface unchanged. All new types, codecs, measurements, outcomes, and persistence details remain crate-private; no public CLI, SDK, generated-skill command, or example workflow was activated.
- Preserved the existing digest wire values while replacing allocation-heavy construction with a model-owned fingerprint path. Equivalence tests lock scalar, fragmented, builder, and unordered aggregation behavior.
- Compressed only sufficiently large stable fact keys and retained an explicit codec prefix plus strict decode failure behavior. Small keys stay plain, and every active reader reconstructs the original stable key before semantic use.
- Assigned fact ordinals after canonical stable-key ordering and persisted those ordinals as the private primary key. This permits compact append-oriented storage without changing semantic uniqueness or reconstruction order.
- Corrected type/value/alias cache-key tests to the provider's declared input contract: lifecycle and upstream extension output digests invalidate; unrelated raw tool and extension snapshot rows do not.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Closed the real enabled-store RSS and cold-time regressions**

- **Found during:** Task 1 locked performance gate
- **Issue:** The first real complete-generation measurement exceeded the immutable resource boundary. The initial file list did not cover the private digest, metadata, planning, connection, schema, and publication hot paths responsible for the cost.
- **Fix:** Expanded only the private implementation surface to eliminate duplicate ownership and intermediate allocations, compact stable keys, batch deterministic writes, and tune new-store SQLite layout. The ratios and floors were never relaxed.
- **Files modified:** `crates/polint/src/analysis_kernel/{incremental,store}/`, `crates/polint/src/analysis_kernel/{metadata,mod}.rs`, benchmark files, and Cargo manifests.
- **Verification:** A representative serialized sample reported peak-RSS ratio 0.9239 and cold-time ratio 1.2216; the final exact ignored gate passed in 59.36 seconds with the original 1.20/1.25 ratios and 16 MiB/50 ms floors.
- **Committed in:** `b68d9409`

**2. [Rule 1 - Bug] Aligned stale type/value/alias cache-key expectations with declared provider inputs**

- **Found during:** Task 1 full-workspace verification
- **Issue:** Two tests expected unrelated raw tool and extension snapshot rows to invalidate a provider whose input-scope contract deliberately excludes them; restoring those blanket reads violated the source contract.
- **Fix:** Kept production scoped to lifecycle components and upstream provider digests, then added explicit equality tests for unconsumed raw tool and extension inputs.
- **Files modified:** `crates/polint/src/analysis/types/cache_key.rs`
- **Verification:** Five focused cache-key tests, the exact provider input-scope test, linked-provider invalidation test, and the full workspace suite passed.
- **Committed in:** `b68d9409`

---

**Total deviations:** 2 auto-fixed (1 blocking performance issue, 1 stale contract test)
**Impact on plan:** Both fixes were required to satisfy the locked gate and declared-input truthfulness. All expanded code remains private metadata/control-plane work; no payload persistence, graph adjacency, query/search CLI, pruning, or later-phase feature was added.

## Issues Encountered

- Several candidate optimizations were measured and rejected because they worsened either peak RSS or cold time, including shared dependency keys, multi-value fact inserts, and broader JSON test-profile optimization. Only improvements that preserved parity and passed both locked ratios were retained.
- The first final-chain attempt exhausted the filesystem after accumulated Cargo targets reached 198 GiB, causing 127 cascading CLI failures with `No space left on device`. Only regenerable target artifacts were cleaned, reclaiming 218.2 GiB; the exact chain was then rerun from command one in a clean build and exited zero.

## User Setup Required

None - the semantic store remains private and disabled by default, with no new configuration, CLI, SDK, or external service.

## Verification

- The exact required command chain exited zero from its first command through `cargo test --workspace --all-features --locked` after the clean rebuild.
- Public-surface leakage passed 7/7 twice, including the outside-consumer build, precise marker negative controls, public artifact scans, and exact 115-name allowlist.
- Semantic-store coverage passed 33/33; byte/exit parity, isolated real-store byte/digest parity, and the serialized ignored performance gate each passed.
- The locked gate retained peak RSS ratio 1.20, cold-time ratio 1.25, 16 MiB RSS floor, and 50 ms cold floor. The persisted store measured 120,352,592 bytes with an equal diagnostics digest.
- `cargo fmt --all -- --check`, all-target/all-feature Clippy with `-D warnings`, `make lint`, and the implementation commit hook passed.
- Full workspace results included 2,582 library tests passed with 2 intentional ignores, 167/167 CLI tests, 7/7 public-surface tests, 2/2 benchmark tests, 11/11 macro tests, and all doctests passing.
- Final audits found no whitespace errors, temporary timing hooks, bare public additions, unsafe code, payload bodies/blobs, graph adjacency, search/pruning implementation, shipped phase-history comments, or Phase 66+ markers.

## Next Phase Readiness

- Phase 65 is ready for milestone-level audit and completion; its privacy, parity, deterministic-identity, enabled-performance, lint, and workspace gates are all closed.
- Later payload/query work can build on the private metadata boundary, but no later-phase implementation was started here.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-14*

## Self-Check: PASSED

Implementation commit `b68d9409` exists. The exact verification chain, immutable performance boundary, public-surface suite, full workspace tests, scope fence, hygiene audit, and pre-commit hook all pass; the worktree contains only this summary awaiting its separate documentation commit.

---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-14T23:52:07Z
depth: standard
files_reviewed: 63
files_reviewed_list:
  - crates/polint/src/analysis/calls/provider.rs
  - crates/polint/src/analysis/calls/validate.rs
  - crates/polint/src/analysis/cfg/provider.rs
  - crates/polint/src/analysis/data_flow/cache_key.rs
  - crates/polint/src/analysis/data_flow/provider.rs
  - crates/polint/src/analysis/demand/query.rs
  - crates/polint/src/analysis/domains/provider.rs
  - crates/polint/src/analysis/entrypoints/provider.rs
  - crates/polint/src/analysis/entrypoints/validate.rs
  - crates/polint/src/analysis/evidence/cache_key.rs
  - crates/polint/src/analysis/evidence/provider.rs
  - crates/polint/src/analysis/extensions/cache_key.rs
  - crates/polint/src/analysis/identity/provider.rs
  - crates/polint/src/analysis/provider.rs
  - crates/polint/src/analysis/reachability/provider.rs
  - crates/polint/src/analysis/refined_calls/cache_key.rs
  - crates/polint/src/analysis/refined_calls/provider.rs
  - crates/polint/src/analysis/semantic_graph/provider.rs
  - crates/polint/src/analysis/solver/provider.rs
  - crates/polint/src/analysis/summaries/closure.rs
  - crates/polint/src/analysis/summaries/provider.rs
  - crates/polint/src/analysis/types/cache_key.rs
  - crates/polint/src/analysis/types/provider.rs
  - crates/polint/src/analysis_kernel/debug.rs
  - crates/polint/src/analysis_kernel/incremental/change_set.rs
  - crates/polint/src/analysis_kernel/incremental/demand.rs
  - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
  - crates/polint/src/analysis_kernel/incremental/dependency_input.rs
  - crates/polint/src/analysis_kernel/incremental/digest.rs
  - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
  - crates/polint/src/analysis_kernel/incremental/invalidation.rs
  - crates/polint/src/analysis_kernel/incremental/keys.rs
  - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/incremental/quarantine.rs
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/analysis_kernel/incremental/stats.rs
  - crates/polint/src/analysis_kernel/metadata.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/provider.rs
  - crates/polint/src/analysis_kernel/store/commit_plan.rs
  - crates/polint/src/analysis_kernel/store/connection.rs
  - crates/polint/src/analysis_kernel/store/generation.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/store/mod.rs
  - crates/polint/src/analysis_kernel/store/schema.rs
  - crates/polint/src/analysis_kernel/store/tests.rs
  - crates/polint/src/analysis_kernel/validation.rs
  - crates/polint/src/analysis_plan.rs
  - crates/polint/src/cache/keys.rs
  - crates/polint/src/eval/bench/gate.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/performance.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/semantic/provider.rs
  - crates/polint/src/metrics.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/tests/cli.rs
  - crates/polint/tests/public_surface_leak.rs
  - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 65 Code Review Report

**Reviewed:** 2026-07-14T23:52:07Z
**Depth:** standard
**Files Reviewed:** 63
**Diff:** `b72cea44..24b09b87`
**Fix commits:** `d7d89a2d`, `b909b52b`, `d73c7c48`, `24b09b87`
**Status:** clean

## Summary

This final independent standard-depth pass re-reviewed the persisted 63-file
scope, all Phase 65 context, plans, and summaries, the original review at
`c491082a`, the prior clean review at `05ca2fe3`, the uncommitted iteration-2 fix
report, all four fix commits, and the affected callers and tests. WR-01, WR-02,
WR-03, and PERF-01 are resolved. No new correctness, security, determinism,
performance, API-visibility, or maintainability finding remains in scope.

## Resolution Evidence

### WR-01: Resolved — optimized publication accepts only validated sealed handoffs

`FinalizedCanonicalFactRows` has private fields and is produced only by
`PreparedCompactStableRows::finish_validated` after deterministic sort,
deduplication, fingerprinting, and stable-key compaction
(`metadata.rs:1106-1165,1173-1205`). The optimized run finalizer consumes that
sealed value and validates the full run identity and semantic invariants before
the store boundary (`run_report.rs:270-275,340-401`).

`StoreCommitPlan::from_owned_validated_run` validates schemas, copied identities,
paths, statuses, required events, provider/query relationships, result
boundaries, dependency endpoints, fact contents, and canonical ordering before
creating the private `ValidatedStoreCommitPlan`
(`commit_plan.rs:566-600,1134-1152,1728-1808`). Generation reservation and
publication accept that wrapper, not a boolean or unsealed prevalidated bypass
(`generation.rs:247-264`). The 77-test store group includes the production-path
malformed-handoff matrix and proves empty stable keys, unknown producers, and
absolute paths cannot reserve, complete, or activate a candidate.

### WR-02: Resolved — identical generations receive complete typed validation

`match_active_generation` retains the read-only connection and matched handle
after exact lifecycle, workspace, identity, and dependency-schema matching
(`generation.rs:331-401`). The new run metadata is dropped before persisted
projection, but validation is unchanged: `read_generation_projection` decodes
every semantic row family, reconstructs `StoreCommitPlan`, runs
`plan.validate()`, rebuilds the typed dependency index, and checks its schema
(`generation.rs:403-421,2966-3067`). Any projection or validation error maps to a
controlled store status; the identical branch returns it with no statistics and
has no publication fallback (`store/mod.rs:243-273`).

The active-row tamper matrix changes an input scalar and deletes representative
fact, query-input, and dependency rows. Every identical rerun returns
`RebuildNeeded(InvalidMetadata)`, never `Ready`
(`store/tests.rs:2120-2183`).

### WR-03: Resolved — current stores require the exact migration-owned schema

Current-schema validation builds a reference database from the owned migrations,
reads every non-internal `sqlite_schema` object in a deterministic order,
normalizes only formatting whitespace with quote awareness, and compares the
complete object inventory and SQL definitions
(`migrations.rs:1039-1122`). Existing lifecycle, foreign-key, column, digest-kind,
and forbidden-payload checks remain independent defenses.

Negative tests reject weakened table constraints, same-name triggers with the
wrong program, same-name indexes over the wrong columns, extra payload-bearing
tables, and unknown version-zero schema objects while preserving existing data
(`migrations.rs:1963-2046`).

### PERF-01: Resolved — validation memory is bounded without weakening proof

The optimized dependency proof is a construction invariant rather than a value
accepted from an unsealed caller. `CanonicalDependencyIndexProof` and all of its
fields are private; its only constructor receives the exact sorted,
deduplicated persistence index and the digest computed from that complete edge
vector at the same construction site. The index, digest, and proof are then
moved together in a prepared value whose fields are also private
(`run_report.rs:67-105,310-335`). Validation binds schema version, edge count,
and the exact content digest before identities are recomputed
(`run_report.rs:340-400,493-610`). There is no constructor, mutable accessor, or
call site that can substitute an independently assembled proof. General
unsealed handoffs never use this proof path and still reconstruct the complete
dependency index and compare it for equality (`run_report.rs:573-588`).

Identical-generation matching and projection are split so the new
`ValidatedRunMetadata` lifetime ends before the complete persisted typed plan is
materialized. Projection failure remains fail-closed as described under WR-02.
At the kernel boundary, fact preparation finishes before metadata/dependency
projection begins, preventing the largest temporary allocations from
overlapping (`analysis_kernel/mod.rs:973-1007`). Concurrency remains inside the
bounded stages: fact rows use parallel sort and thread-local parallel
fingerprint/compression work, while semantic and dependency row digests use
read-only parallel iteration (`metadata.rs:1133-1149,1195-1225` and
`run_report.rs:1386-1455`). The comparator is total, output order is preserved,
and unordered digest aggregation is deterministic; the compact-row round-trip
and 24-permutation run-metadata tests both passed independently.

The performance fix changes only four private implementation files. A direct
parent-to-fix diff shows no changes to eval gates, baseline defaults, fixtures,
Cargo manifests, or supported public modules. The immutable limits remain RSS
`1.20`, cold time `1.25`, RSS floor `16 MiB`, and cold floor `50 ms`. Source-diff
and tree scans found no diagnostic instrumentation, child-stderr plumbing,
temporary probes, unsafe additions, or bare public additions. The public-surface
leak suite passed all seven tests.

## Independent Verification

- `cargo test -p polint --lib analysis_kernel::incremental::run_report::tests::canonical_dependency_proof_binds_schema_count_and_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::metadata::tests::compact_stable_rows_preserve_semantics_and_round_trip_storage --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::incremental::run_report::tests::validated_run_metadata_is_identical_across_twenty_four_permutations --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1`
  — 77 passed, 0 failed in 4.82s.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1`
  — 5 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity::all_store_modes_preserve_byte_identical_json_and_exit_semantics --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed; JSON bytes and exit semantics matched.
- `cargo test -p polint --lib eval::bench::runner::tests::semantic_store::isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed; enabled mode retained real store bytes and diagnostic
  digest parity.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1`
  — 7 passed, 0 failed in 51.68s.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `git diff --check b72cea44..HEAD` over the exact 63-file scope and
  `git diff --check` for the pre-review worktree — passed.
- `git diff --quiet 24b09b87^ 24b09b87 -- crates/polint/src/eval/baseline.rs crates/polint/src/eval/bench/gate.rs crates/polint/src/eval/bench/runner.rs crates/polint/src/eval/performance.rs tests/eval-fixtures crates/polint/tests Cargo.toml crates/polint/Cargo.toml`
  — exit 0; thresholds, baselines, fixtures, tests, and manifests were unchanged.

## Independent Locked Boundary Sample

Before the sample, the process table contained no Cargo, rustc, polint, or eval
process, and no other check ran concurrently. The exact command was:

```text
cargo test -p polint --lib eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary --locked -- --exact --ignored --test-threads=1 --nocapture
```

It passed 1/1 in 64.93s with peak-RSS delta `901,873,664` bytes, RSS ratio
`0.9764 <= 1.2000`, cold time `10,541 ms`, cold ratio
`1.2151 <= 1.2500`, exact store size `120,352,592` bytes, and matching
diagnostics digest `28cac8a32a5bb2a9`.

## Residual Risk

- Cold time has less headroom than peak RSS in this sample (`1.2151` against
  `1.2500`), so the unchanged locked gate should remain part of future store
  work.
- The low-allocation dependency proof deliberately relies on private sealed
  construction. Future changes must preserve the co-construction and privacy of
  the canonical index, digest, and proof; independently assembled handoffs must
  continue through full reconstruction.
- Identical reuse intentionally pays for full typed projection and validation.
  The fix reduces overlapping lifetimes; it does not weaken that fail-closed I/O
  and validation cost.
- Sequential top-level staging removes coarse overlap to bound peak memory while
  preserving Rayon work inside each stage. Future attempts to restore overlap
  must be re-proved against deterministic byte parity and the locked RSS gate.

## Worktree State

Before this review artifact was written, the exact worktree state was:

```text
 M .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW-FIX.md
```

At handoff, the only expected modifications are this overwritten review and the
pre-existing iteration-2 fix report:

```text
 M .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW-FIX.md
 M .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md
```

`65-REVIEW-FIX.md` was read but not edited. No source file was modified.

---

_Reviewed: 2026-07-14T23:52:07Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_

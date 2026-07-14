---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-14T22:45:03Z
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

**Reviewed:** 2026-07-14T22:45:03Z
**Depth:** standard
**Files Reviewed:** 63
**Diff:** `b72cea44..d73c7c48`
**Fix commits:** `d7d89a2d`, `b909b52b`, `d73c7c48`
**Status:** clean

## Summary

The iteration-2 standard-depth pass re-read the requested 63-file scope, all 19
phase plans and summaries, the committed review at `c491082a`, the uncommitted
fix report, the three fix commits, and the affected callers and tests. The three
prior warning-level findings are resolved. No new correctness, security,
determinism, performance, API-visibility, or maintainability issue was found in
the reviewed scope.

## Prior Warning Resolution

### WR-01: Resolved — optimized writes require sealed facts and a validated store plan

`FinalizedCanonicalFactRows` is a private sealed handoff produced only by
`finish_validated` after canonical fact preparation
(`metadata.rs:1173-1192`). The optimized metadata finalizer validates the full
handoff before it reaches the store. `StoreCommitPlan::from_owned_validated_run`
then validates the complete plan and returns the private
`ValidatedStoreCommitPlan` wrapper (`commit_plan.rs:33,566-593`); generation
commit entry points require that wrapper (`generation.rs:239-247`). The former
boolean/prevalidated bypass is gone, so reservation and activation cannot occur
before validation succeeds.

The regression at `store/tests.rs:1809` exercises an empty stable fact key, an
unknown producer, and an absolute path through the production handoff. Each is
rejected without reserving, completing, or activating a candidate, while the
previous active generation remains intact.

### WR-02: Resolved — identical reuse reconstructs and validates active rows

After matching lifecycle/header identity, `active_generation_statistics`
reconstructs the active generation through `read_generation_projection`
(`generation.rs:321-385,2944`) and therefore runs the same typed row decoding and
complete plan validation used by ordinary active reads. It no longer trusts only
header values and selected statistics digests.

The tamper matrix at `store/tests.rs:2120` changes an input scalar and fact,
query-child, and dependency-child rows. An identical rerun fails closed with
`RebuildNeeded(InvalidMetadata)` instead of returning `Ready`.

### WR-03: Resolved — current schema validation compares exact canonical definitions

`validate_current_schema` now builds the expected schema in an in-memory database
from the owned migrations, reads every non-SQLite object from `sqlite_schema`,
normalizes definitions with quote-aware whitespace handling, and compares the
complete ordered `(type, name, table, SQL)` inventory. This covers table
constraints, foreign keys, checks, defaults, column order, `WITHOUT ROWID`, index
columns, trigger bodies, and unexpected objects rather than checking names alone.

Negative tests at `migrations.rs:1984-2026` reject a weakened required table, a
same-name trigger with the wrong program, a same-name index with the wrong
columns, and an extra payload-bearing table. Version-zero unknown schemas also
remain fail-closed without losing existing data.

## Full-Scope Review Notes

- The validated-type boundaries are private to `analysis_kernel`; no supported
  `polint::sdk` or `polint::runner` surface was widened.
- Canonical ordering, digest construction, dependency endpoint validation,
  generation publication, and recovery paths remain deterministic and
  fail-closed.
- Exact schema matching is appropriate for this migration-owned private store;
  it rejects semantically altered or unrecognized same-version databases before
  payload access.
- Full projection validation makes the identical-reuse path deliberately more
  expensive, but it closes the integrity gap without changing public behavior.

## Verification

- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1`
  — 77 passed, 0 failed.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1`
  — 7 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity::all_store_modes_preserve_byte_identical_json_and_exit_semantics --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed.
- `cargo test -p polint --lib eval::bench::runner::tests::semantic_store::isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed; enabled mode reported nonzero store bytes and the same
  diagnostics digest.
- `cargo test -p polint --lib eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary --locked -- --exact --ignored --test-threads=1 --nocapture`
  — the first sample exceeded the peak-RSS ratio (`1.2406 > 1.2000`) while the
  cold-time ratio passed (`1.2324 <= 1.2500`); an immediate identical rerun passed
  both locked boundaries (RSS `1.1317`, cold time `1.1740`), reported
  `120352592` store bytes, and preserved digest `28cac8a32a5bb2a9`. The enabled
  absolute RSS measurement was nearly unchanged across samples (about 1.05 GB),
  so the isolated failure was attributable to comparison-baseline variance and
  was not reproducible as an implementation regression.
- `cargo fmt --all -- --check` — passed.
- `git diff --check c491082a..HEAD` and `git diff --check b72cea44..HEAD` — passed.
- No source files or the untracked `65-REVIEW-FIX.md` were modified by this
  review.

---

_Reviewed: 2026-07-14T22:45:03Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_

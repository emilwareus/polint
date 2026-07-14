---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-14T21:47:45Z
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
  warning: 3
  info: 0
  total: 3
status: issues_found
---

# Phase 65 Code Review Report

**Reviewed:** 2026-07-14T21:47:45Z
**Depth:** standard
**Files Reviewed:** 63
**Diff:** `b72cea44..63cfd9f6`
**Status:** issues_found

## Summary

The standard-depth pass covered the requested 63-file scope, all 19 phase plans
and summaries, and the full phase diff. The identity, dependency, privacy,
generation-lifecycle, batching, and public-visibility work is generally coherent,
and the focused test suites pass. Three fail-closed guarantees are incomplete,
however: the optimized production write path no longer proves the plan it
publishes, the identical-generation shortcut does not validate the active
semantic projection, and current-version schema validation trusts object names
rather than the schema semantics behind those names.

The semantic store remains default-disabled, which limits current exposure, but
each issue can make an enabled store accept or reuse state that the full typed
reader would reject.

## Narrative Findings (AI reviewer)

The three findings below are independent warning-level correctness and robustness
gaps in the enabled semantic-store path.

## Warnings

### WR-01: The optimized kernel write path can activate a plan that was never integrity-validated

**Files:**

- `crates/polint/src/analysis_kernel/incremental/run_report.rs:225-248,307-356`
- `crates/polint/src/analysis_kernel/metadata.rs:607-665,1106-1215`
- `crates/polint/src/analysis_kernel/store/commit_plan.rs:544-580,1109-1125,1701-1741`
- `crates/polint/src/analysis_kernel/store/generation.rs:233-253,792-821`
- `crates/polint/src/analysis_kernel/store/migrations.rs:439-452`
- `crates/polint/src/analysis_kernel/store/mod.rs:232-281`

**Issue:** The ordinary constructor calls `ValidatedRunMetadata::validate_integrity`
and `StoreSemanticPlan::validate_without_stats`, but the production kernel uses
the optimized `prepare_finalized_canonical_run` / `finish_prepared_canonical_run`
pair with `verify_reconstruction = false`. It then calls
`from_owned_prevalidated_run`, whose comment substitutes provenance for validation,
and finally `commit_owned_prevalidated_generation`, which passes
`validate_plan = false`. Post-write validation checks row counts, a subset of
declared child counts, stats readback, and validation-event equality; it never
reconstructs or otherwise proves the skipped identity, path, relationship,
endpoint, fact, and canonical-order invariants before marking the generation
complete and active.

This is not merely duplicate defensive work. For example,
`prepare_compact_stable_rows` accepts an empty fact stable key and the compact
codec persists it as a non-empty LZ4 blob. `StoreSemanticPlan::validate_facts`
would reject that key, while `StableFactKey::from_storage` also rejects its
declared decoded length of zero. The optimized path skips the former, the SQL
`stable_key <> ''` check sees a non-empty encoded blob, and the post-write checks
do not decode it. A provider defect can therefore publish a generation that the
full active-generation reader cannot read.

**Fix:** Preserve the allocation-saving handoff, but make the proof explicit.
Construct a sealed, private validated-handoff type only after an allocation-light
integrity pass has checked every invariant currently covered by
`validate_integrity` and `validate_without_stats`, and require that type at the
unvalidated commit entry point. Alternatively, run the existing plan validation
before reservation until equivalent streaming validation exists. Add a
kernel-path regression test that injects malformed fact metadata (at minimum an
empty stable key, plus an invalid producer or path) and proves the candidate
never becomes complete or active.

### WR-02: Identical-generation reuse trusts headers and three stats digests without validating active rows

**Files:**

- `crates/polint/src/analysis_kernel/store/mod.rs:232-263`
- `crates/polint/src/analysis_kernel/store/generation.rs:314-410,2919-3057`

**Issue:** Before materializing a commit plan, `commit_validated_run` calls
`active_generation_statistics` and returns `Ready` when it finds matching header
identities. That shortcut verifies lifecycle state, the generation header, and
only the input, dependency, and validation digests copied into `generation_stats`.
It counts all payload rows solely to produce a reported size; it does not compare
the per-family counts to the stats, decode semantic fields, recompute aggregates,
or validate child declarations and dependency endpoints. In contrast, the normal
active reader reconstructs every row family and calls `plan.validate()`.

Consequently, same-version corruption that leaves the header and three selected
stats columns intact is treated as a valid cache hit. For example, changing an
`input_files.source_digest_value`, deleting a query child, or altering a fact row
can leave all shortcut comparisons true. An unchanged subsequent run then returns
`Ready` without detecting that the active projection no longer matches the
identity it claims.

**Fix:** Do not treat the header as a content-authenticated projection. Before
reusing an identical active generation, perform the typed read/plan validation,
or add a genuinely equivalent cheap integrity check that compares every family
count and recomputes row-bound semantic aggregates from persisted values. Add
tamper tests for a scalar input field and representative fact/query/dependency
child rows; an identical rerun must return controlled `RebuildNeeded`, never
`Ready`.

### WR-03: Current-version schema validation accepts weakened constraints and arbitrary extra schema objects

**Files:**

- `crates/polint/src/analysis_kernel/store/migrations.rs:947-1023,1026-1072,1074-1277`
- `crates/polint/src/analysis_kernel/store/tests.rs:1978-2021`

**Issue:** `validate_current_schema` proves that required table, index, and
trigger *names* exist. For tables, `validate_required_columns` then compares only
an unordered set of column names. It does not verify declared types, `NOT NULL`,
primary/foreign keys, `CHECK` constraints, defaults, column order,
`WITHOUT ROWID`, index definitions, or trigger bodies. The table inventory is
also not exact: only eleven specifically named payload-like tables or columns are
forbidden, so an arbitrary extra table such as `cached_sources(contents BLOB)`
passes the metadata-only gate.

The existing active-pointer tamper test illustrates the blind spot: it replaces
`store_manifest_active_must_be_complete` with a same-name no-op trigger. Schema
validation notices the invalid *data state* created afterward, but the weakened
trigger itself satisfies the current schema check. A same-version database can
therefore pass validation while omitting the relational enforcement publication
and recovery depend on, and the promised strict metadata-only boundary is not
actually established.

**Fix:** Validate an exact schema inventory and canonical object definitions.
Use `table_xinfo`, `foreign_key_list`, `index_list` / `index_xinfo`, and normalized
`sqlite_master.sql` (or a versioned canonical schema hash) to verify table
constraints, index columns, trigger programs, and allowed auxiliary objects.
Add negative tests that recreate a required table without its checks/FKs, replace
a required trigger or index with a same-name wrong definition, and add an
unrecognized payload-bearing table.

## Verification

- `cargo test -p polint analysis_kernel::store --lib` — 71 passed.
- `cargo test -p polint --test public_surface_leak` — 7 passed.
- `git diff --check b72cea..HEAD` — passed.
- No supported public API widening was found in the reviewed scope.
- No source files were modified by this review.

---

_Reviewed: 2026-07-14T21:47:45Z_
_Reviewer: the agent (gsd-code-reviewer)_
_Depth: standard_

---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-15T08:43:47Z
depth: deep
files_reviewed: 71
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - crates/polint/Cargo.toml
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
  - crates/polint/src/analysis/extensions/provider.rs
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
  - crates/polint/src/core/mod.rs
  - crates/polint/src/eval/bench/gate.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/observed.rs
  - crates/polint/src/eval/performance.rs
  - crates/polint/src/go/adapter.rs
  - crates/polint/src/go/semantic/provider.rs
  - crates/polint/src/go/tests.rs
  - crates/polint/src/metrics.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/tests.rs
  - crates/polint/tests/cli.rs
  - crates/polint/tests/public_surface_leak.rs
  - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml
  - tests/fixtures/public-surface-leak-probe/Cargo.lock
findings:
  critical: 0
  warning: 9
  info: 1
  total: 10
status: issues_found
---

# Phase 65 Deep Code Review Report

**Reviewed:** 2026-07-15T08:43:47Z
**Depth:** deep, split across three independent reviewers
**Diff:** `origin/main...68b7fdea`
**Status:** issues found

## Summary

Three read-only reviewers independently traced the store lifecycle, incremental
identity/dependency graph, and cross-platform/public integration surfaces. The
syntax-layer dependency omission was independently found by two reviewers. The
findings below were then checked against the implementation and the Phase 65
contracts before entering the fix loop. Previously fixed review findings were
not reopened.

## Warning Findings

### WR-05: Active-generation validation does not bind persisted rows to stored identities

**Files:** `store/generation.rs:380-416,2966-3067`,
`store/commit_plan.rs:1164-1225,1811-2035`, `store/mod.rs:243-265`,
`store/tests.rs:2120-2183`

`read_generation_projection` decodes every semantic row, but
`StoreGenerationStats::from_plan` copies family identities from the generation
header and `validate_stats` compares those copies back to the same header. A
same-length mutation such as flipping one hex digit in
`input_files.source_digest_value` preserves counts and logical JSON bytes, so an
identical rerun can return `Ready` for altered metadata. Recompute canonical
family, run, and generation identities from the decoded projection and compare
them to the stored header/stats. Add same-length mutation coverage across every
semantic row family; each must return `RebuildNeeded(InvalidMetadata)`.

### PERF-02: Forged stable-key compression prefixes permit repeated 64 MiB allocations

**Files:** `analysis_kernel/metadata.rs:607-795`,
`store/generation.rs:3008-3010,3070-3082,4093-4135`,
`store/migrations.rs:441-454`

`StableFactKey::from_storage` trusts the LZ4 size prefix up to 64 MiB, does not
require the actual decoded length to equal it, and retains the encoded form.
Later comparisons, hashing, formatting, and serialization decompress again;
sorting many small forged rows can amplify this into repeated large allocations.
The plain branch is unbounded too. Enforce small encoded/decoded per-key limits,
exact size-prefix equality, and a hard total key budget before collecting rows;
normalize/decode once. Add forged-prefix, oversized plain/encoded, zero-output,
and many-row amplification tests with controlled failure.

### WR-06: `StableFactKey` violates the `Eq`/`Ord` contract

**Files:** `analysis_kernel/metadata.rs:542-552,602-604,758-787`,
`store/generation.rs:4112-4135`, `store/commit_plan.rs:1728-1768`

Compressed/compressed equality compares encoded bytes while ordering and hashing
use decoded text. Two valid LZ4 encodings of one string can therefore compare
equal under `Ord` but unequal under `Eq`, and duplicate semantic facts can evade
validation. Normalize at the storage boundary or define all three traits over
one decoded representation. Add trait-law coverage using alternate encodings
and a store projection test that rejects semantic duplicates.

### WR-07: Syntax-layer dependencies are incomplete and unauthenticated

**Files:** `go/adapter.rs:201-336`, `ts/adapter.rs:233-375`,
`incremental/layer_cache.rs:695`, `incremental/run_report.rs:1300-1309`

Go and TS syntax keys include source, settings, lifecycle, parser-toolchain, and
parameter identities, but both retained manifests store `Vec::new()`
dependencies. The persisted dependency graph therefore omits the syntax layer
nodes entirely. Cache-hit validation also checks only dependency sources, not
exact destinations/kinds/shapes, so a locally modified current-schema manifest
can replace dependency truth while retaining a valid payload digest. Build one
canonical expected edge set from the current key/snapshot, use it on write, and
require exact equality on read. Pass the canonical settings digest directly
instead of rehashing its textual value. Add real Go/TS round trips plus missing,
replaced, and unrelated-edge tests.

### WR-08: Failed providers can be persisted as `NativeTrusted`

**Files:** `analysis_kernel/mod.rs:1079-1130` and provider result types/callers

Multiple explicit provider failures return `output_digest: None`, but the kernel
special-cases only data-flow/evidence. Other `None` values are replaced by a
digest over remaining rows and labeled `NativeTrusted`; several store-rejection
paths also return `Some` and are trusted. Carry an explicit typed execution
outcome (`Skipped`, `Succeeded`, `Failed`) from every provider and never infer
success from `Option<Digest>`. Persist failures as `provider_failed` and keep
intentional skips distinct. Add injected provider/store failure round trips.

### WR-09: Production input snapshots certify false model and Go-tool inputs

**Files:** `incremental/input_snapshot.rs:308,355,914`,
`incremental/run_report.rs:902`, `analysis/semantic_graph/provider.rs:188`

Every production snapshot records `model.files` as absent even though semantic
graph discovers and hashes `.polint/models/**`. It records
`go.tool_invocation` as unsupported even though Go semantic invokes and versions
the frontend/toolchain. Those false rows enter `RunIdentity`, leaving no true
typed endpoint for later change-set construction. Share model discovery/digest
construction with the snapshot and finalize a truthful Go tool identity before
the run identity is sealed. Cover model add/edit/delete and Go tool/version
changes, including unlinked-sibling reuse.

### WR-10: Missing language syntax providers are recorded as present upstream layers

**Files:** `analysis_kernel/mod.rs:251`, `module_graph/mod.rs:2238`,
`symbol_graph/mod.rs:949`, `metrics.rs:595`

The kernel represents a missing Go or TS syntax output with an absent digest,
but module graph, symbol graph, and metrics always attach
`InputComponentStatus::Present`. Preserve availability alongside optional
outputs and write `Absent` for a missing language. Add Go-only and TS-only store
round trips proving the sibling changes from absent to present when introduced.

### WR-11: Go semantic durable identity uses Rust `Debug` spelling

**File:** `go/semantic/provider.rs:318-329`

Lifecycle identity material formats `InputComponentStatus` with `{:?}` even
though the type has a stable `label()` codec. Use structured digest fields with
the stable label and cover every canonical lowercase status, proving Rust enum
variant spelling is absent from durable identity material.

### PERF-03: Reachability output invalidates semantic graph without being consumed

**Files:** `analysis/semantic_graph/provider.rs:446-482`,
`analysis_kernel/mod.rs:799`

The semantic-graph manifest and builder do not read reachability facts, yet its
output digest is folded solely to over-invalidate. A reachability-roots change
therefore rotates semantic graph and downstream solver/refinement identities
while their semantic bytes are unchanged. Remove this dependency until the
provider actually consumes it and add an identity-stability regression.

## Info Finding

### IN-01: `git diff --check` fails on an extra EOF blank line

**File:** `.planning/phases/65-generation-manifest-and-metadata-mirroring/65-PATTERNS.md:184`

Remove the extra blank line before the green-check pass.

## Clean Areas

No additional actionable defect was found in reservation/publication atomicity,
active-pointer CAS, rollback/failure-event isolation, pinned-reader behavior,
exact schema-object comparison, provider/query stable-key ordering, public API
visibility, Cargo/MSRV/dependency changes, privacy boundaries, SARIF/CLI output,
or the locked performance gate itself.

---

_Reviewed: 2026-07-15T08:43:47Z_
_Review mode: three independent read-only deep passes, merged and validated by the orchestrator_

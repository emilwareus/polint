---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-15T18:07:24Z
depth: deep
files_reviewed: 77
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - crates/polint/Cargo.toml
  - crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go
  - crates/polint/src/analysis/adaptation/discovery.rs
  - crates/polint/src/analysis/adaptation/mod.rs
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
  - crates/polint/src/go/semantic/client.rs
  - crates/polint/src/go/semantic/process.rs
  - crates/polint/src/go/semantic/provider.rs
  - crates/polint/src/go/tests.rs
  - crates/polint/src/metrics.rs
  - crates/polint/src/module_graph/mod.rs
  - crates/polint/src/repo_fs.rs
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
  info: 0
  total: 9
status: issues_found
---

# Phase 65 Deep Code Re-review Report

**Reviewed:** 2026-07-15T18:07:24Z
**Depth:** deep, three fresh independent post-fix reviewers
**Fix range:** `0dda5032..3b9e1ba8`
**Full diff:** `origin/main...3b9e1ba8`
**Status:** issues found

## Summary

The second fix pass resolved all fourteen prior findings and passed `make check`,
but a fresh adversarial review found nine novel defects in provider outcome
semantics, the active-store trust boundary, and sealed Go frontend execution.
The reviewers also rechecked the repaired stable-fact accounting, runtime
content framing, private-cache permissions, and supported public surfaces.

## Provider and Capability Semantics

### WR-20 (P2): Per-file cache-write warnings become durable syntax facts

**Files:** `go/adapter.rs:425-468`, `ts/adapter.rs:464-507`

Go and TypeScript per-file analysis-cache write failures are inserted into the
`SyntaxLayerPayload`. That payload is then hashed and persisted in the layer
cache, so a transient write warning changes semantic identity and can be
replayed by a later warm hit after the underlying failure has disappeared. The
existing failed-write parity tests cover the layer-cache write rather than this
earlier per-file write path.

Keep persistence warnings in run-local telemetry and out of semantic payloads,
digests, and cached diagnostics. Add cold/warm regressions that fail a per-file
analysis-cache write while allowing the layer cache to publish, then prove the
semantic identity is unchanged and the warning is not replayed.

### WR-21 (P2): Extensions can succeed on an unauthenticated partial universe

**Files:** `analysis_kernel/mod.rs:842`, `analysis/extensions/provider.rs:124`

The extensions provider still runs after unsupported symbol or entrypoint
dependencies and builds from `native_stable_keys(db)`, which silently returns a
partial universe. It may therefore publish `Succeeded` / `NativeTrusted`, while
its digest does not bind the failed upstream status or output identity.

Make the required dependency contract explicit: either skip extensions when a
required universe provider is unsupported, or deliberately model degradation.
In both cases, bind every consumed dependency's stable status and digest into
the extension identity. Add failed-versus-absent dependency-chain tests.

### WR-22 (P1): Late provider failures do not revoke advertised capabilities

**Files:** `analysis_kernel/mod.rs:342`, `analysis_plan.rs:770`,
`core/mod.rs:7738`, `policy_queries.rs:57`

`capability_support` is frozen before late providers execute. Calls, control
flow, and dataflow can remain statically `Supported` after their provider fails
or is dependency-blocked, so requesting rules execute against empty or fallback
views instead of receiving a `polint/capability` diagnostic.

Derive effective capability availability from completed provider outcomes and
use it before rule execution. Failed and dependency-blocked hard capabilities
must prevent the rule from running; planned absence must remain distinct. Add
end-to-end rule tests for failed calls, CFG, and dataflow providers.

## Active Store Trust Boundary

### WR-23 (P2): Identical-generation validation spans two active snapshots

**File:** `analysis_kernel/store/generation.rs:344-412,3013-3124`

`match_active_generation` reads the manifest and header without the later read
transaction. A concurrent publisher can rotate the active pointer after that
match; `validate_active_generation_statistics` then validates the old immutable
handle without rechecking that it remains active and may return `Ready` with
statistics for inactive truth.

Carry the expected identities and schema into the matched handle. Inside the
projection transaction, re-read the manifest, require the same active handle,
and authenticate the header again. Add an interlock regression that publishes a
new generation between match and validation and proves the old match cannot
return `Ready`.

### PERF-07 (P2): Non-fact metadata is materialized without allocation preflight

**Files:** `analysis_kernel/store/generation.rs:3021-3139,3293-3318`,
`analysis_kernel/store/migrations.rs:145-156`

The active reader preflights fact storage, but first materializes every input,
provider, layer, summary, query, diagnostic, dependency, and telemetry row into
unbounded vectors and strings. A hostile active store can allocate an enormous
TEXT value, or many short metadata rows, before count or identity rejection.

Apply a writer-symmetric store-wide row and byte budget to every persisted
family. Preflight SQLite storage classes, byte lengths, bounded row counts, and
aggregate overhead before any active materialization. Add huge-TEXT and
many-short-row tests in a non-fact family with a decode seam proving zero rows
were materialized before rejection.

### WR-24 (P3): Canonical metadata ordinals are not authenticated

**File:** `analysis_kernel/store/generation.rs:3297-4531`

Outside fact metadata, readers order by stored ordinals but usually do not
select or validate them. Parent `semantic_ordinal` values are discarded and
regenerated with `enumerate`. Changing a one-child ordinal from `0` to `999`
therefore preserves order, counts, logical projection, and recomputed identities
even though the store is no longer the canonical writer projection.

Select and validate every persisted parent and child ordinal as a contiguous
`0..n` sequence, globally or per parent as appropriate, before discarding it.
Add gap and offset tamper tests for each ordinal family.

## Runtime and Toolchain Boundaries

### SEC-04 (P1): The Go executable used by package loading is not sealed into identity

**Files:** `go/semantic/process.rs:204-306,1286-1299`,
`go-sidecar/polint-go-frontend/internal/semantic/emit.go:98-129,1080-1085`

Prepared frontend identity binds frontend bytes, but binary and installed modes
record no Go toolchain. Runtime execution inherits `PATH`, while
`packages.Load` resolves `go` independently through `os.Environ()`. The same
frontend identity can therefore cache facts under Go A and reuse them under Go
B; source mode also does not prove that probe, build, and runtime selected the
same executable.

Resolve one exact Go executable for every frontend mode; bind its content,
canonical selection, version, and normalized module environment into provenance
and identity; then force both build and package loading to use that sealed
selection. Test two fake launchers and a `PATH` swap: identity must rotate or
execution must stay pinned to the first verified toolchain.

### PERF-08 (P2): Custom frontend source traversal is unbounded before its limit

**File:** `go/semantic/process.rs:785-878`

`capture_source_snapshot` checks the 512-file limit only after recursive
collection. The walker has no entry, directory, depth, or frontier ceiling, so
a large tree can grow the vector without bound, irrelevant entries consume
unbounded time, and deep nesting risks stack overflow.

Use iterative descriptor-anchored traversal with entry, directory, depth, file,
and frontier budgets enforced while walking. Add flat overflow, excessive depth,
and irrelevant-entry fixtures that prove deterministic early rejection.

### REL-01 (P3): Frontend staging directories can collide and leak on errors

**File:** `go/semantic/process.rs:749-754,926-942,1073-1110,1176-1183`

Persistent staging names contain only the PID and wall-clock nanoseconds, which
does not guarantee uniqueness across threads. Multiple `?` exits after creation
bypass cleanup, leaving source, build, and seal staging trees behind after
transient failures; a timestamp collision can make callers share a directory.

Allocate staging directories atomically with unpredictable/create-new names and
an RAII cleanup guard that is disarmed only after successful publication. Add
failure injection at every post-create step, concurrent allocation coverage,
and bounded cleanup for stale staging entries.

## Clean Areas

The repaired fact writer/reader accounting is symmetric for UTF-8 byte lengths,
encoded and decoded stable-key limits, empty keys, fixed row overhead, aggregate
budgeting, SQLite blob preflight, and LZ4 declared sizes. No additional defect
was found in SHA-256 framing, special-file nonblocking rejection, sealed frontend
byte reuse, Unix ownership and modes, descriptor closure, macOS POSIX behavior,
intentional non-Unix fail-closed behavior, supported SDK/CLI visibility, or
Cargo/MSRV compatibility.

---

_Reviewed: 2026-07-15T18:07:24Z_
_Review mode: fresh three-domain post-fix deep review_

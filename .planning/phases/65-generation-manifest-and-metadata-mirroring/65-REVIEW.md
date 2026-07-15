---
phase: 65-generation-manifest-and-metadata-mirroring
reviewed: 2026-07-15T11:02:10Z
depth: deep
files_reviewed: 75
files_reviewed_list:
  - Cargo.lock
  - Cargo.toml
  - crates/polint/Cargo.toml
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
  - crates/polint/src/symbol_graph/mod.rs
  - crates/polint/src/ts/adapter.rs
  - crates/polint/src/ts/tests.rs
  - crates/polint/tests/cli.rs
  - crates/polint/tests/public_surface_leak.rs
  - tests/eval-fixtures/cache/input-snapshots/expected.polint-eval.toml
  - tests/fixtures/public-surface-leak-probe/Cargo.lock
findings:
  critical: 0
  warning: 14
  info: 0
  total: 14
status: issues_found
---

# Phase 65 Deep Code Re-review Report

**Reviewed:** 2026-07-15T11:02:10Z
**Depth:** deep, three fresh independent post-fix reviewers
**Fix range:** `2498f996..f4d86091`
**Full diff:** `origin/main...f4d86091`
**Status:** issues found

## Summary

The first fix pass resolved all ten original findings and passed `make check`,
but a fresh adversarial review found fourteen novel defects in the fixes and
their cross-effects. These findings were independently scoped to the post-fix
tree and were not present in the prior review/fix report.

## Store Trust Boundary

### WR-12: Writer can publish stable keys that its own reader rejects

**Files:** `analysis_kernel/metadata.rs:608-650,1212-1295`,
`store/commit_plan.rs:2133-2173`, `store/generation.rs:800-829,2253-2290,4102-4148`

Fact finalization and plan validation do not enforce the reader's 256 KiB
encoded/decoded and 32 MiB aggregate budgets. A first run can publish an active
generation that the next identical run rejects; very large keys can also reach
the `u32::try_from(...).expect(...)` panic. Make finalization fallible and apply
one exact symmetric accounting function before reservation and during reads.
Test incompressible boundary keys and aggregate overflow, proving no first-run
`Ready` can become an identical-run rebuild.

### PERF-04: Stable-key guards run after SQLite allocation and omit row overhead

**Files:** `store/generation.rs:3079-3091,4124-4128`,
`analysis_kernel/metadata.rs:659-693`, `store/commit_plan.rs:2139-2144`

`row.get::<_, Vec<u8>>` materializes an arbitrary BLOB before the size check.
The aggregate budget charges only key bytes, so millions of short rows and their
other strings can exhaust memory; empty keys consume zero budget. Preflight SQL
storage type/length before materialization, reject empty keys at decode time,
and enforce a writer-symmetric row/fixed-overhead budget before collection.
Add oversized `zeroblob` and many-short-row tests with a seam proving early
rejection.

### WR-13: Input child relationships and declared counts are unauthenticated

**Files:** `store/commit_plan.rs:297-329,963-1046,1543-1558`,
`store/generation.rs:899-974,2968-3055,3218-3365`,
`store/migrations.rs:131-190`

Same-width tampering of `input_components.detail_count`,
`requested_capabilities.requester_count`, or a detail's copied component digest
can still return `Ready`; active reads do not invoke the SQL child-count audit.
Validate parent existence, copied digest equality, detail counts, capability
requester counts, and summary dependency counts in the typed plan and active
SQL audit. Add child reassignment/count tamper regressions.

## Provider and Dependency Semantics

### WR-14: Cache write warnings are misclassified as provider failures

**Files:** `go/adapter.rs:190-210,502-508`, `ts/adapter.rs:222-242`,
`module_graph/mod.rs:823-838,1303-1318`, `symbol_graph/mod.rs:217-232`,
`metrics.rs:138-153`, `analysis_kernel/mod.rs:1149-1170`

Valid in-memory facts and complete layer metadata are discarded solely because
cache persistence fails, changing semantic identities for a telemetry warning.
Retain `Succeeded`, the output digest, and layer metadata after write failure;
keep warning/cache counters separate. Restore five-path metadata parity tests
and compare provider/run/generation identities.

### WR-15: Non-syntax provider failures collapse to absence before downstream execution

**Files:** `analysis_kernel/mod.rs:269-942`, `incremental/stats.rs:104-127`,
`go/semantic/provider.rs:113-136`, `analysis/cfg/provider.rs:53-65`,
`symbol_graph/mod.rs:439-444`, `module_graph/mod.rs:470-490`

Only syntax results use `ProviderOutputDependency`; later failures are converted
with `unwrap_or_else(Digest::absent)` and downstream layers often hardcode
`Present`. Thread typed digest+execution status through every provider. Hard
dependents must skip/fail; deliberate degradation must preserve `Unsupported`
instead of `Absent`/`Present`. Add failed-vs-skipped chains for Go semantic to
identity/solver, MIR to CFG, and module graph to symbol/topology.

### WR-16: Solver consumes reachability roots without authenticating them

**Files:** `analysis_kernel/mod.rs:711-862`, `analysis/solver/provider.rs:90-103`,
`analysis/solver/go_rta/inputs.rs:193-203`

The semantic graph correctly stopped folding unconsumed reachability, but the
solver directly reads `db.reachability_roots()` and receives no reachability or
exact Go-root-set digest. Construct Go RTA inputs once and fold a canonical
sorted-root digest into solver identity. Preserve TS-root stability and add a
Go-root mutation that changes RTA/solver output.

### PERF-05: Semantic graph still folds four provider outputs it does not consume

**Files:** `analysis/semantic_graph/provider.rs:100-110,161-174,305-359`,
`analysis/semantic_graph/build.rs:111-132`

Identity, abstract domains, entrypoints, and module topology are folded into the
semantic-graph digest despite no corresponding fact reads. Remove each unused
input until a read exists and add one identity-stability regression per input.

### WR-17: Stable input-status codec repair is incomplete

**Files:** `analysis/provider.rs:274-282`, `analysis/cfg/provider.rs:451-473`,
`analysis/calls/provider.rs:250-256`, `analysis/domains/provider.rs:242-248`,
`analysis/entrypoints/provider.rs:158-164`,
`analysis/semantic_graph/provider.rs:506-512`,
`analysis/types/cache_key.rs:131-141`, `analysis/extensions/provider.rs:202-207`

These durable/cache identities still use `{:?}` for `InputComponentStatus`.
Centralize component identity encoding with `status.label()` and pin every
status for every provider builder; Rust variant spellings must be absent.

## Runtime Input and Security Boundaries

### SEC-01: Predictable shared-temp frontend cache can execute a preseeded binary

**File:** `go/semantic/process.rs:227-234,483`

The embedded/source frontend cache uses a predictable shared temporary path and
trusts `.complete` plus any existing fixed-name executable. Another local user
can preseed a Linux first-run cache and gain execution. Use a private,
user-owned, no-symlink cache root with restrictive permissions and verify
content/provenance on every reuse. Add hostile preseed and symlink fixtures with
an injected cache root.

### SEC-02: Sealed tool identity does not bind the executable actually run

**Files:** `go/semantic/process.rs:73-104`, `go/semantic/client.rs:74`,
`analysis_kernel/mod.rs:478`

Preparation hashes a path, seals the snapshot, and later reopens the mutable
path for execution. Replacing bytes between those steps runs different code
under the old identity. Copy verified bytes into a private immutable
content-addressed execution path or retain a safe executable handle. Test
prepare-A/replace-with-B and prove B cannot execute under A's identity.

### WR-18: Behavior-affecting Go environment is outside identity

**Files:** `go/semantic/process.rs:493`, `go/semantic/client.rs:79`,
`go-sidecar/polint-go-frontend/internal/semantic/emit.go:98-125,1079`,
`incremental/input_snapshot.rs:1485`

Build and `packages.Load` inherit `GOOS`, `GOARCH`, `GOFLAGS`, `CGO_ENABLED`,
and related values, but tool identity records only executable/source/toolchain
and the environment policy only workspace selection. Sanitize semantic
modifiers at build/execution or encode an explicitly supported normalized set.
Add isolated-process identity/behavior tests varying target and flags.

### WR-19: Source-mode cache ignores source/toolchain/target provenance

**File:** `go/semantic/process.rs:479-493`

A fixed cached binary returns before the Go 1.25 check, so source edits,
toolchain changes, target changes, and even a preexisting Go 1.24 build can be
accepted. Key/stamp the cache by source digest, supported toolchain, host target,
and normalized build environment; validate toolchain before lookup. Test source
edits, target/toolchain changes, and a Go 1.24 preseed.

### PERF-06: Adaptation-model discovery budget does not bound traversal memory

**File:** `analysis/adaptation/discovery.rs:72-106`

Each directory is fully collected and every eligible entry enters a pending map
before the 32-file content cap is checked. Use a deterministic bounded
walker/top-k queue plus explicit visited-entry/directory ceilings. Test queue and
retained-path bounds with a large flat fixture.

### SEC-03: Model no-symlink/root check is check-then-open

**Files:** `analysis/adaptation/discovery.rs:110-143`,
`repo_fs.rs:129-181,438`

Discovery checks symlink metadata, then later canonicalizes and opens through a
fresh path operation; swapping a file or ancestor for a symlink/reparse point
can read outside the repository and expose content through parse diagnostics.
Use an anchored root descriptor with no-follow semantics and validate/read from
the same handle. Add a deterministic race-hook test swapping an in-repo model
for an outside symlink between validation and open.

## Clean Areas

The first-pass fixes remain correct for normalized key equality, same-length
family identity recomputation, exact syntax-edge equality, the explicit outcome
codec itself, syntax availability, and stable Go lifecycle labels. No supported
SDK/CLI API widening, Cargo/MSRV defect, or planning whitespace issue was found.

---

_Reviewed: 2026-07-15T11:02:10Z_
_Review mode: fresh three-domain post-fix deep review_

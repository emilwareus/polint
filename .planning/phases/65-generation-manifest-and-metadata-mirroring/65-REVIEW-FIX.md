---
phase: 65-generation-manifest-and-metadata-mirroring
iteration: 2
findings_in_scope:
  - WR-01
  - WR-02
  - WR-03
  - PERF-01
fixed: 4
skipped: 0
status: all_fixed
---

# Phase 65 Code Review Fix Report

## Fixed Findings

### WR-01 — Prove optimized kernel handoffs before publication

- Commit: `d7d89a2d` (`fix(65): validate optimized store handoffs`)
- Finalized canonical fact rows cross the allocation-saving kernel boundary
  through a private sealed handoff carrying the digest of the exact sorted,
  deduplicated pre-compression rows.
- Optimized run finalization validates the complete run identity and dependency
  invariants against that proof without re-expanding compressed fact keys.
- Publication requires a private `ValidatedStoreCommitPlan`; the boolean bypass
  and unsealed prevalidated commit entry points were removed.
- Kernel-path regressions prove malformed fact keys, unknown producers, and
  absolute paths are rejected before candidate reservation, completion, or
  activation while the prior active generation remains readable.

### WR-02 — Validate every active row before identical-generation reuse

- Commit: `b909b52b` (`fix(65): validate identical active generations`)
- The identical-generation path checks its lifecycle and identity header, then
  typed-decodes every persisted row family, reconstructs the semantic plan, and
  runs complete plan validation before returning `Ready`.
- Tamper regressions cover an altered input digest and missing representative
  fact, query, and dependency rows. Each returns controlled
  `RebuildNeeded(InvalidMetadata)` without reusable statistics.

### WR-03 — Require the exact canonical SQLite schema

- Commit: `d73c7c48` (`fix(65): enforce exact semantic store schema`)
- Current-version validation compares the complete non-internal schema inventory
  and canonical table, index, and trigger definitions with a migration-built
  reference schema.
- Quote-aware SQL normalization removes formatting-only whitespace while
  preserving quoted tokens and trigger/string semantics.
- Existing column, lifecycle, foreign-key, and forbidden-payload checks remain
  independent defenses. Negative coverage includes weakened constraints,
  same-name but incorrect triggers and indexes, extra payload-bearing tables,
  and version-zero migration rollback with an unknown schema object.

### PERF-01 — Restore the locked semantic-store RSS boundary

- Commit: `24b09b87` (`fix(65): bound semantic store validation memory`)
- Controlled, environment-gated instrumentation isolated three overlapping
  allocations in the locked fixture, which contains 228,975 fact rows and
  130,694,467 bytes of plain stable keys:
  - WR-01's persistence-only validation reconstructed a dependency index with a
    cloned edge vector and forward/reverse adjacency maps even though the proof
    compared only schema and canonical edges. That step added approximately
    42.5 MB at the measured high-water mark.
  - WR-02's newly produced `ValidatedRunMetadata` remained live while the full
    active generation was projected and validated, adding approximately
    155.8 MB to the absolute high-water mark.
  - Top-level `rayon::join` made large fact-key compaction overlap dependency
    metadata projection, producing enough run-to-run variation to breach the
    immutable RSS boundary.
- A private sealed `CanonicalDependencyIndexProof` now binds the canonical
  dependency schema version, edge count, and digest at construction time. The
  optimized path validates this proof without reconstructing edges or adjacency
  maps; the ordinary non-sealed integrity path still performs the complete
  reconstruction and equality check. The traced proof stage added only about
  0.26 MB at the high-water mark.
- Identical-generation reuse now separates its identity/header match from the
  unchanged full typed projection. On an identity match, new run metadata is
  dropped before projection begins. Projection or validation failure still
  returns a controlled rebuild status and never falls through to publication.
- Fact-key compaction and metadata/dependency projection now run as sequential
  top-level stages. Their internal data parallelism remains intact, while their
  largest temporary allocations no longer coexist.
- All diagnostic instrumentation and child-stderr plumbing were removed before
  the source commit. No instrumentation marker remains in the source tree.

## Locked Boundary Evidence

An initial clean trial after the first two allocation fixes produced two passes
and one RSS failure at ratio 1.2153. That sequence was rejected rather than used
as acceptance evidence; it exposed the remaining top-level allocation overlap
and led to the final staging fix.

After that fix, three exact locked samples ran sequentially on an otherwise idle
workspace with no overlapping work:

| Sample | RSS delta (bytes) | RSS ratio | Cold time (ms) | Cold ratio | Store bytes | Diagnostics |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1 | 1,004,617,728 | 1.0976 | 11,010 | 1.2164 | 120,352,592 | matched |
| 2 | 896,712,704 | 0.9828 | 11,029 | 1.2109 | 120,352,592 | matched |
| 3 | 903,299,072 | 1.1013 | 11,024 | 1.2180 | 120,352,592 | matched |

The maximum RSS ratio was 1.1013, leaving 0.0987 below the unchanged 1.2000
limit. The maximum cold-time ratio was 1.2180, leaving 0.0320 below the unchanged
1.2500 limit. The 16 MiB RSS floor and 50 ms cold-time floor are also unchanged.
Every sample preserved the exact store byte count and diagnostic digest parity.

Exact command for each accepted sample:

```text
cargo test -p polint --lib eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary --locked -- --exact --ignored --test-threads=1 --nocapture
```

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental::run_report::tests::canonical_dependency_proof_binds_schema_count_and_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1`
  — 77 passed, 0 failed in 4.78s, including malformed handoffs and the identical
  active-generation tamper matrix.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1`
  — 5 passed, 0 failed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity::all_store_modes_preserve_byte_identical_json_and_exit_semantics --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed in 0.63s.
- `cargo test -p polint --lib eval::bench::runner::tests::semantic_store::isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed in 0.32s.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1`
  — 7 passed, 0 failed in 71.78s.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo test --workspace --all-features --locked` — exited 0. The polint
  library ran 2,589 passing tests with 2 ignored; CLI integration ran 167
  passing tests; public-surface ran 7; polint-bench ran 2; polint-macros ran 11;
  the polint doctest ran 1; and all remaining example, binary, and doctest
  targets passed.
- The source commit passed the repository `make lint` hook. The performance
  threshold and baseline files are unchanged, and the fix did not widen the
  supported public API.

## Atomicity

The first review iteration is preserved as three focused commits for handoff
proofs, identical-generation validation, and exact schema enforcement. The
iteration-2 source fix is one focused follow-up commit addressing the memory
lifetime consequences of WR-01 and WR-02 without weakening either fail-closed
boundary. This report is intentionally left uncommitted for the review workflow.

## Residual Risks

- Cold time has less headroom than RSS: the maximum accepted ratio was 1.2180
  against 1.2500. The budget and noise floor were not relaxed; continued locked
  monitoring is appropriate.
- The low-allocation dependency proof relies on private sealed type state created
  alongside the exact canonical index and digest. Independently assembled,
  non-sealed reports continue through the full reconstruction path.
- Identical-generation reuse still performs the full typed projection and
  validation. The fix changes object lifetimes, not the fail-closed validation
  contract.
- Sequential top-level staging intentionally trades one source of coarse-grained
  overlap for bounded peak memory. The stages themselves remain data-parallel,
  and the locked cold-time gate passed without any threshold change.

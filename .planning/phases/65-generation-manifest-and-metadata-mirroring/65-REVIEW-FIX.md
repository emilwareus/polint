---
phase: 65-generation-manifest-and-metadata-mirroring
iteration: 1
findings_in_scope:
  - WR-01
  - WR-02
  - WR-03
fixed: 3
skipped: 0
status: all_fixed
---

# Phase 65 Code Review Fix Report

## Fixed Findings

### WR-01 — Prove optimized kernel handoffs before publication

- Commit: `d7d89a2d` (`fix(65): validate optimized store handoffs`)
- Finalized canonical fact rows now cross the allocation-saving kernel boundary
  through a private sealed handoff that carries the digest of the exact sorted,
  deduplicated pre-compression rows.
- The optimized run finalizer validates the complete run identity and dependency
  invariants against that proof without re-expanding compressed fact keys.
- Generation publication now requires a private `ValidatedStoreCommitPlan`; the
  boolean validation bypass and the unsealed prevalidated commit entry points
  were removed.
- A kernel-path regression injects an empty fact key, unknown producer, and
  absolute path and proves each malformed handoff is rejected before candidate
  reservation, completion, or activation while the prior active generation
  remains readable.

### WR-02 — Validate every active row before identical-generation reuse

- Commit: `b909b52b` (`fix(65): validate identical active generations`)
- The identical-generation shortcut still checks the lifecycle and identity
  header first, then typed-decodes every persisted row family, reconstructs the
  semantic plan, and runs the full plan validation before returning `Ready`.
- Tamper regressions alter an input source digest and remove representative fact,
  query, and dependency child rows. Every identical rerun now returns controlled
  `RebuildNeeded(InvalidMetadata)` with no reusable statistics, never `Ready`.

### WR-03 — Require the exact canonical SQLite schema

- Commit: `d73c7c48` (`fix(65): enforce exact semantic store schema`)
- Current-version validation compares the complete non-internal schema inventory
  and canonical table, index, and trigger definitions against a reference schema
  constructed from the versioned migrations.
- SQL normalization is quote-aware: it removes formatting-only whitespace while
  preserving quoted tokens and trigger/string semantics.
- Existing column, lifecycle, foreign-key, and forbidden-payload checks remain as
  independent defenses.
- Negative tests cover a required table recreated without constraints, a
  same-name no-op trigger, a same-name index over the wrong columns, and an extra
  payload-bearing table. Version-zero migration also fails closed and rolls back
  when an unknown schema object is present.

## Verification

- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1`
  — 77 passed, 0 failed, 4.75s.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1`
  — 7 passed, 0 failed, 89.95s.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity::all_store_modes_preserve_byte_identical_json_and_exit_semantics --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed, 0.83s.
- `cargo test -p polint --lib eval::bench::runner::tests::semantic_store::isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest --locked -- --exact --test-threads=1`
  — 1 passed, 0 failed, 0.35s.
- `cargo test -p polint --lib eval::bench::gate::tests::semantic_store_boundary::real_store_enabled_measurement_passes_locked_boundary --locked -- --exact --ignored --test-threads=1 --nocapture`
  — passed in 67.97s. Peak-RSS delta was 985,989,120 bytes with a
  1.1675 ratio against the unchanged 1.2000 limit and 16 MiB floor. Cold time
  was 11,075 ms with a 1.2251 ratio against the unchanged 1.2500 limit and 50 ms
  floor. The store occupied 120,352,592 bytes and the diagnostics digest matched
  at `28cac8a32a5bb2a9`.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo test --workspace --all-features --locked` — passed with exit code 0.
  The main library ran 2,588 passing tests (2 ignored), the CLI integration suite
  ran 167 passing tests, the public-surface suite ran 7 passing tests, and the
  remaining bench, macro, example, and doctest targets all passed.
- Each atomic fix commit also passed the repository `make lint` hook.

## Atomicity

The fixes are independent and reviewable in three commits: WR-01 seals and
validates publication handoffs, WR-02 validates already-published state before
reuse, and WR-03 validates the database schema boundary. None depends on a later
fix commit to compile or pass its focused tests.

## Residual Risks

- Identical-generation reuse now incurs a full typed projection read and
  validation. This intentionally favors fail-closed correctness over the old
  header-only shortcut; future optimization would need an equivalent
  content-authenticated proof.
- Exact schema validation intentionally rejects semantically equivalent but
  noncanonical user modifications and unknown non-internal SQLite objects. The
  store is private and migration-owned, so fail-closed rejection is the intended
  boundary behavior.
- The locked performance sample passed with less headroom on cold time than on
  peak RSS. Thresholds and noise floors were not changed; repeated performance
  monitoring remains appropriate as the store evolves.

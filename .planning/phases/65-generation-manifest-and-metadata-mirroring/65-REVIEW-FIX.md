---
phase: 65-generation-manifest-and-metadata-mirroring
fixed_at: 2026-07-18T05:40:32Z
review_path: .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md
iteration: 8
findings_in_scope: 6
fixed: 6
skipped: 0
review_status: converged
validation_status: final_baseline_and_full_gate_pending
---

# Phase 65 Deep Review Fix Report

The six findings in the fourth deep review are fixed. Review then continued in
independent security, storage, filesystem, determinism, performance, and
cross-platform passes. Those passes found additional boundary issues, each was
fixed and re-reviewed, and the final broad and focused reviewers returned
clean. No finding was skipped.

## Fourth-Review Findings

### SEC-05 — Seal the complete Go execution closure

The Go launcher is no longer treated as the whole toolchain identity. Frontend
preparation captures the selected toolchain and the files it can execute or
consume into a bounded, content-addressed dependency snapshot. The snapshot is
authenticated before use, retained for the command lifetime, and recertified
across execution boundaries. Toolchain-content changes rotate provenance, and
post-prepare replacement cannot silently change the executed closure.

The implementation also pins the dependency snapshot during retirement. On
Darwin, cleanup uses an authenticated directory descriptor and verifies device
and inode identity before removal, so renames and path replacement cannot turn
retention cleanup into an ambient-path deletion.

### WR-25 — Remove unauthenticated ambient Go resolution state

Go subprocesses now receive one explicit environment policy instead of a fresh
ambient process environment. Resolution, cache, proxy, VCS, workspace, and
toolchain inputs are normalized, certified, and incorporated into analysis
identity where retained. Local module inputs and repository cache boundaries
are snapshotted through the same lifecycle, including monorepos without a root
`go.mod`; the sidecar consumes the certified environment rather than rebuilding
one from `os.Environ()`.

Failed or incomplete dependency preparation does not publish placeholder
semantic facts. It returns a controlled setup/capability outcome.

### REL-02 — Bound Go subprocesses, streams, and protocol allocation

Probe, build, dependency, and semantic commands share bounded execution
machinery with deadlines, process-tree containment, capped stdout/stderr,
bounded descriptor and descendant inspection, and cleanup that remains active
through pipe EOF. Timeout and overflow paths terminate the complete owned
process tree and cannot wait forever on a descendant holding a pipe.

The NDJSON protocol validates line, row, collection, field, and aggregate byte
limits before accumulation. Oversized and malformed output becomes a controlled
error rather than unbounded allocation or a panic.

### WR-26 — Revoke trust after global fact validation fails

Effective capabilities and provider trust are finalized only after global fact
validation. A malformed CFG, call, refined-call, semantic, or data-flow output
revokes the affected capability and all dependent provider trust. Rules that
require a revoked hard capability receive capability diagnostics and do not
execute. Cold and warm paths preserve the same outcome, and failed validation
cannot publish a trusted cache generation.

### WR-27 — Authenticate persisted projections before activation

Pending semantic-store candidates are decoded and validated through the typed
projection boundary before completion and active-pointer rotation. Validation
authenticates scalar values, references, identities, lifecycle state, schema,
and the exact candidate handle inside the protected publication sequence. A
tampered candidate fails closed and leaves the previous complete generation
active.

### WR-28 — Bind ordinals to canonical row content

Ordinal checks now authenticate the association between each canonical row and
its ordinal, globally or within its parent partition. A value-preserving swap no
longer passes merely because the partition still contains the set `0..n`.
Writer, active-reader, identical-generation, and failed-attempt paths share the
same canonical-order contract and tamper matrix.

## Convergence Fixes Found After the Fourth Review

The iterative review loop also hardened the boundaries surrounding the six
reported findings:

- Filesystem operations used by repository discovery, semantic caches, SQLite,
  dependency snapshots, and cleanup are descriptor-anchored or equivalently
  revalidated. Unix symlinks and Windows reparse points are rejected at the
  relevant boundary, final SQLite components use no-follow semantics, and
  rename/replacement races fail closed.
- Traversal, hashing, cleanup, process inspection, protocol decoding, and cache
  retention have explicit entry, byte, depth, frontier, or visit budgets.
  Limits are enforced during traversal rather than after unbounded collection.
- Dependency-snapshot publication authenticates staging, reservation, and
  published state. Failed validation cannot leak a publication; retention will
  not remove a live snapshot; stale cleanup remains bounded and scope-checked.
- Semantic provider identities bind only the upstream universes they actually
  consume. Late failure propagates through dependency trust without rotating
  identities for unrelated providers.
- SQLite generation validation preserves one authenticated snapshot across
  matching, projection, and reuse. Storage classes and sizes are preflighted
  before materialization, and every persisted canonical family participates in
  order and reference validation.
- Performance evidence uses same-host paired measurements, absolute RSS
  high-water values, authentic generated scale, and nonzero-boundary checks.
  Zero raw deltas remain informational rather than fabricated measurements.
- The shared default Go cache is coordinated only in tests that execute the
  semantic pipeline. Coordination wait happens before fixture runtime timing,
  nested frontend preparation reuses the same permit, and the dependency lease
  drops before the shared slot is released.
- The refined-call functional fixture now uses a portable 30-second envelope,
  matching its sibling. Strict speed contracts remain in the separate,
  serialized `performance-gate` profile.

## Review Convergence

Review proceeded in loops: independent reviewers inspected a changed-file
diff, fixes were applied, focused tests ran, and fresh reviewers inspected the
integrated result. The final broad reviews covered the Go runtime/toolchain and
dependency lifecycle, persistent store and SQLite integrity, filesystem and
platform behavior, provider trust, deterministic ordering, public visibility,
CI commands, and the test-only concurrency lifetime. They returned `CLEAN` with
no remaining concrete bug, security issue, or API-boundary finding.

## Verification So Far

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`:
  passed.
- The full workspace diagnostic run completed 2,886 tests successfully. Its
  only failures were two intentionally stale generated baselines and two
  assertions sharing the test-only fixture timing issue fixed afterward; no
  production behavior test failed.
- The refined-call regression passed under CI's
  `CARGO_PROFILE_TEST_DEBUG=0` profile after the timing fix.
- The default-parallel determinism gate passed twice consecutively: 13 tests
  per run, including both Go fixtures and ten seeded permutations.
- Focused Go frontend/process, Go RTA, persistent-store, provider-chain,
  filesystem, public-surface, documentation, install-smoke, and dependency
  policy checks passed during the fix loop.
- `git diff --check`: passed.

The committed store-disabled baselines must be regenerated from a clean source
commit. After that, the complete CI-equivalent workspace and performance matrix
will be rerun; this report's validation status will be updated with the final
evidence before publication.

## Residual Boundaries

- Toolchain and cache sealing defend the same-user local integrity boundary;
  compromise of the operating-system account remains outside the threat model.
- Certified size and traversal ceilings intentionally fail closed. Supporting
  larger inputs requires an explicit contract change rather than silent budget
  expansion.
- Windows and macOS platform-specific behavior is covered by isolated local
  compilation/tests and remains part of the required remote CI matrix.

---
phase: 64-store-foundation-and-boundary-proof
reviewed: 2026-07-10T11:25:00Z
depth: standard
re_review: false
files_reviewed: 17
files_reviewed_list:
  - Cargo.toml
  - crates/polint/Cargo.toml
  - crates/polint/src/cache/mod.rs
  - crates/polint/src/repo_fs.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/analysis_kernel/store/mod.rs
  - crates/polint/src/analysis_kernel/store/connection.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/store/tests.rs
  - crates/polint/src/eval/performance.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/bench/gate.rs
  - crates/polint/tests/public_surface_leak.rs
  - tests/fixtures/public-surface-leak-probe/src/lib.rs
findings:
  blocker: 0
  critical: 0
  warning: 3
  info: 1
  total: 4
status: issues_found
---

# Phase 64 Code Review Report

**Depth:** standard  
**Scope:** Phase 64 product, test, external-probe, and benchmark-gate changes  
**Status:** issues found

## Findings

### WR-01 — Future/invalid stores are switched to WAL before compatibility refusal

**Severity:** Warning  
**File:** `crates/polint/src/analysis_kernel/store/connection.rs:45`

`open_writer` executes `PRAGMA journal_mode = WAL` before `apply_migrations`
reads and validates `PRAGMA user_version`. Journal mode is persistent database
state, so a newer or malformed store can be mutated before the code returns
`FutureSchema`/`InvalidSchema`. The fixtures currently assert version and
sentinel preservation, but not byte/journal-mode preservation.

**Recommendation:** Add a read-only compatibility preflight before any
persistent connection pragma. Refuse future, malformed-current, and corrupt
databases first; configure WAL and migrate only empty/prior/current-valid
stores. Extend fixtures to assert journal mode/file bytes remain unchanged.

### WR-02 — Malformed bootstrap shapes can escape the typed invalid-schema path

**Severity:** Warning  
**File:** `crates/polint/src/analysis_kernel/store/migrations.rs:75`

The invariant check verifies that the table exists, then queries its `version`
column. A v1 database with `_polint_schema_migrations` present but a wrong
column shape returns a raw SQLite `Unknown` error, which connection
classification maps to `OpenFailed`, not `RebuildNeeded(InvalidSchema)`.
Additionally, a valid version-1 marker plus extra version rows is accepted
because only `count(version = 1)` is checked.

**Recommendation:** Convert any bootstrap-invariant query failure to
`InvalidSchema`, require exactly one total marker row, and add wrong-shape plus
extra-row current-schema fixtures.

### WR-03 — Phase boundary priming uses enabled mode and excludes first-open cost

**Severity:** Warning  
**File:** `crates/polint/src/eval/bench/gate.rs:71`

The digest priming run uses `SemanticStoreBenchMode::Enabled`, creating and
migrating the database before the isolated measurement. This stabilizes the
tiny fixture, but the measured point no longer includes Phase 64's first-open
schema cost, unlike the plan's store-enabled child requirement. The committed
baseline generator primes analysis/toolchain caches with a store-disabled
digest run.

**Recommendation:** Prime with a disabled digest (matching the baseline
generator), run the enabled isolated point against an absent store, then compute
the enabled digest for parity. This compares equivalent analysis cache states
while retaining real first-open store cost.

### IN-01 — Managed containment does not distinguish hard-linked files

**Severity:** Info  
**File:** `crates/polint/src/repo_fs.rs:309`

Canonical containment and symlink checks accept a regular hard link whose inode
also has a name outside the cache. SQLite writes through that link would mutate
the shared inode. This does not cross OS user permissions, but it weakens the
claim that the existing database is exclusively cache-owned.

**Recommendation:** Where portable metadata permits, reject existing managed
database files with multiple hard links; otherwise document this local-user
trust limitation. Do not broaden Phase 64 into platform-specific handle APIs.

## Positive Observations

- Disabled mode returns before filesystem or SQLite work and is proven through a full kernel run.
- Writer contention is bounded and deterministic; future/corrupt/symlink fixtures are non-panicking.
- Raw rusqlite/SQL types stay under `analysis_kernel/store/`; the prelude remains exactly 115 items.
- The full all-feature workspace suite and focused Phase 64 matrix pass.

## Required Follow-up

Fix WR-01 through WR-03 and re-run migration, recovery, real-boundary, lint, and
workspace tests. IN-01 is advisory unless a small portable/narrow mitigation is
available.

---
_Reviewed: 2026-07-10_  
_Reviewer: Codex (inline gsd-code-reviewer fallback; sub-agents not authorized)_  
_Depth: standard_

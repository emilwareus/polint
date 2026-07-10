---
phase: 64-store-foundation-and-boundary-proof
reviewed: 2026-07-10T11:31:00Z
depth: standard
re_review: true
pass: 2
prior_findings: { critical: 0, warning: 3, info: 1, total: 4 }
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
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 64 Code Review Report — Second Pass

**Depth:** standard  
**Scope:** Phase 64 product, test, external-probe, and benchmark-gate changes  
**Status:** clean

## Re-review Summary

Commit `8ac63000` resolves all three warnings. The sole informational hard-link
note is accepted as a documented same-user/local-cache limitation: it does not
cross OS permissions, and a portable handle-level defense would exceed Phase
64's narrow boundary without improving the stated symlink threat contract.
No actionable finding remains.

## Confirmed Fixed / Disposition

### WR-01 — Future/invalid stores are switched to WAL before compatibility refusal

**Prior severity:** Warning — **RESOLVED**
**File:** `crates/polint/src/analysis_kernel/store/connection.rs:45`

`open_writer` executes `PRAGMA journal_mode = WAL` before `apply_migrations`
reads and validates `PRAGMA user_version`. Journal mode is persistent database
state, so a newer or malformed store can be mutated before the code returns
`FutureSchema`/`InvalidSchema`. The fixtures currently assert version and
sentinel preservation, but not byte/journal-mode preservation.

**Resolution:** `preflight_schema` now runs after the bounded timeout but before
foreign-key/WAL setup. Future, malformed-current, and corrupt stores are refused
before persistent pragmas; the recovery fixture asserts byte identity across
future maintenance and explicit rebuild refusal.

### WR-02 — Malformed bootstrap shapes can escape the typed invalid-schema path

**Prior severity:** Warning — **RESOLVED**
**File:** `crates/polint/src/analysis_kernel/store/migrations.rs:75`

The invariant check verifies that the table exists, then queries its `version`
column. A v1 database with `_polint_schema_migrations` present but a wrong
column shape returns a raw SQLite `Unknown` error, which connection
classification maps to `OpenFailed`, not `RebuildNeeded(InvalidSchema)`.
Additionally, a valid version-1 marker plus extra version rows is accepted
because only `count(version = 1)` is checked.

**Resolution:** Bootstrap-shape query failures now classify as
`InvalidSchema` while busy/corrupt/I/O codes retain their operational classes.
Validation requires exactly one total marker, and dedicated wrong-shape and
extra-row fixtures pass.

### WR-03 — Phase boundary priming uses enabled mode and excludes first-open cost

**Prior severity:** Warning — **RESOLVED**
**File:** `crates/polint/src/eval/bench/gate.rs:71`

The digest priming run uses `SemanticStoreBenchMode::Enabled`, creating and
migrating the database before the isolated measurement. This stabilizes the
tiny fixture, but the measured point no longer includes Phase 64's first-open
schema cost, unlike the plan's store-enabled child requirement. The committed
baseline generator primes analysis/toolchain caches with a store-disabled
digest run.

**Resolution:** The boundary now primes with a disabled digest, measures enabled
mode against the absent store, then computes the enabled parity digest. Three
consecutive runs passed at 37–38 ms cold, ~41.7–42.0 MB RSS delta, and 8 KiB
store size without threshold changes.

### IN-01 — Managed containment does not distinguish hard-linked files

**Prior severity:** Info — **ACCEPTED / NON-ACTIONABLE**
**File:** `crates/polint/src/repo_fs.rs:309`

Canonical containment and symlink checks accept a regular hard link whose inode
also has a name outside the cache. SQLite writes through that link would mutate
the shared inode. This does not cross OS user permissions, but it weakens the
claim that the existing database is exclusively cache-owned.

**Disposition:** A hard link can only target a file the same OS user may already
modify and does not bypass the documented symlink/containment boundary. A
portable no-follow/open-handle redesign is deferred; Phase 64 does not claim
protection from a malicious same-user process racing or relinking cache files.

## Positive Observations

- Disabled mode returns before filesystem or SQLite work and is proven through a full kernel run.
- Writer contention is bounded and deterministic; future/corrupt/symlink fixtures are non-panicking.
- Raw rusqlite/SQL types stay under `analysis_kernel/store/`; the prelude remains exactly 115 items.
- The full all-feature workspace suite and focused Phase 64 matrix pass.

## Re-review Verification

- Migration fixtures: 9 passed, including wrong-shape and extra-marker current schemas.
- Recovery fixtures: 4 passed, including future database byte preservation.
- Writer contention: passed.
- Real Phase 64 boundary: three consecutive passes with first-open store creation.
- Workspace clippy pre-commit gate: passed with `-D warnings`.

No additional issue was introduced by the fixes.

---
_Re-reviewed: 2026-07-10_
_Reviewer: Codex (inline gsd-code-reviewer fallback; sub-agents not authorized)_  
_Depth: standard_

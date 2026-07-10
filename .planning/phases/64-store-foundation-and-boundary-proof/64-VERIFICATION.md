---
phase: 64-store-foundation-and-boundary-proof
verified: 2026-07-10T11:52:20Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  # No — initial verification (no prior VERIFICATION.md existed)
---

# Phase 64: Store Foundation and Boundary Proof Verification Report

**Phase Goal:** A private, crash-safe SQLite store facade exists with migrations and connection discipline, `polint check` behavior is provably unchanged, and the store costs nothing when disabled.
**Verified:** 2026-07-10
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth (roadmap Success Criteria) | Status | Evidence |
|---|----------------------------------|--------|----------|
| 1 | Bundled rusqlite is contained behind a `pub(crate)` store facade and no raw SQLite type escapes | ✓ VERIFIED | Workspace dependency is exactly `rusqlite = { version = "0.40.1", features = ["bundled"] }`. The implementation is confined to `analysis_kernel/store/`; the supported SDK/runner/CLI scan reports no `rusqlite`, public store module, or store re-export. The seven-test public leak suite and external prelude-only probe pass with the prelude fixed at 115 names. |
| 2 | Numbered migrations cover bootstrap, current, future, invalid, and recovery behavior without mutating refused schemas | ✓ VERIFIED | `CURRENT_SCHEMA_VERSION = 1`, `PRAGMA user_version`, transactional migration, schema preflight, and a minimal single-marker bootstrap schema are implemented. Nine migration tests cover empty, explicit v0, idempotent current, future refusal, missing/wrong-shape/extra-marker invalid schemas, and refusal-before-mutation; four recovery tests include future database byte preservation and explicit safe rebuild. |
| 3 | Connection policy and bounded single-writer behavior are explicit | ✓ VERIFIED | Writer setup applies foreign keys, WAL, and a 250 ms busy timeout; `TransactionBehavior::Immediate` supplies the lease. Readers open separately with `SQLITE_OPEN_READ_ONLY`. Connection-policy, read-only, and deterministic contention fixtures pass; the loser returns `BusySkipped` without interleaved writes. |
| 4 | Store states do not change policy behavior, and disabled mode has zero store I/O | ✓ VERIFIED | Default production construction leaves the store disabled. Disabled maintenance returns before path validation or SQLite open and the no-filesystem-touch fixture passes. Six-mode disabled/enabled/busy/future/invalid/corrupt kernel parity produces identical normalized JSON and exit status. The Phase 64 gate passes diagnostics digest parity and locked RSS/cold checks. |
| 5 | Providers/rules receive no SQL surface and the public boundary remains unchanged | ✓ VERIFIED | Store maintenance runs only after kernel computation and validation and records a private `StoreStatus` in `KernelRunReport`. No provider, adapter, rule, `RuleCtx`, SDK, runner, or public renderer receives SQL/store handles. Public source, JSON, docs, examples, and generated-skill scans pass with marker-family negative controls. |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `Cargo.toml` / `crates/polint/Cargo.toml` | Locked bundled rusqlite dependency | ✓ VERIFIED | Workspace dependency plus crate-private implementation dependency; no activation feature was added. |
| `crates/polint/src/cache/mod.rs` | Cache-owned path and disabled activation | ✓ VERIFIED | `semantic-store/store.sqlite3` derives from the existing cache root; production constructors disable it; tests prove path ownership and zero filesystem touch. |
| `crates/polint/src/analysis_kernel/store/mod.rs` | Typed private store facade | ✓ VERIFIED | `StoreConfig`, `SemanticStore`, `StoreStatus`, skip/rebuild reasons, and safe maintenance/rebuild mapping remain crate-private and carry no rusqlite values. |
| `crates/polint/src/analysis_kernel/store/migrations.rs` | Strict numbered migration runner | ✓ VERIFIED | Version preflight precedes persistent pragmas; migration is transactional/idempotent; future and malformed schemas are typed refusals. |
| `crates/polint/src/analysis_kernel/store/connection.rs` | Writer/read-only policy and lease | ✓ VERIFIED | WAL, foreign keys, bounded timeout, immediate writer transaction, read-only connection, and error classification are implemented privately. |
| `crates/polint/src/analysis_kernel/store/tests.rs` | Connection, contention, and recovery fixtures | ✓ VERIFIED | Connection policy, independent reader, bounded contention, corrupt/future/invalid handling, safe rebuild, and path defenses pass. |
| `crates/polint/src/analysis_kernel/mod.rs` | Post-validation integration and parity proof | ✓ VERIFIED | Maintenance is invoked after validation/finalization; disabled/default and six store modes are covered without public output changes. |
| `crates/polint/src/analysis_kernel/incremental/run_report.rs` | Private store telemetry | ✓ VERIFIED | Run report records only the typed crate-private `StoreStatus`; rendering contracts are untouched. |
| `crates/polint/src/eval/bench/runner.rs` | Isolated enabled-store measurement | ✓ VERIFIED | Test-only mode enables the store, measures store bytes separately, and computes normal check diagnostics. |
| `crates/polint/src/eval/bench/gate.rs` | Real Phase 64 regression boundary | ✓ VERIFIED | Disabled cache priming is followed by enabled first-open measurement and digest parity against the committed Phase 63 baseline. |
| `crates/polint/tests/public_surface_leak.rs` | Store/SQL leak gate | ✓ VERIFIED | Seven tests scan supported public sources/output/docs/examples/skill text and build the external consumer; negative controls cover every marker family. |
| `tests/fixtures/public-surface-leak-probe/src/lib.rs` | Outside SDK-only consumer | ✓ VERIFIED | Compiles with `polint::sdk::prelude::*` and cannot name store internals. |

### Key Link Verification

| From | To | Via | Status |
|------|----|-----|--------|
| `cache/mod.rs` | `analysis_kernel/store/mod.rs` | Cache-owned path and private enabled flag become `StoreConfig` | ✓ WIRED |
| `store/mod.rs` | `store/connection.rs` | Facade maps private connection results to typed status | ✓ WIRED |
| `store/connection.rs` | `store/migrations.rs` | Compatibility preflight, connection policy, then strict migration | ✓ WIRED |
| `analysis_kernel/mod.rs` | `store/mod.rs` | Maintenance occurs after output validation/finalization | ✓ WIRED |
| `incremental/run_report.rs` | `StoreStatus` | Private telemetry captures maintenance outcome | ✓ WIRED |
| `eval/bench/gate.rs` | `store-disabled-check.json` | Real isolated enabled point and diagnostics digest feed locked regression checks | ✓ WIRED |
| `public_surface_leak.rs` | external probe/public artifacts | Compile plus curated marker scans with negative controls | ✓ WIRED |

### Locked Decision Compliance

| Decisions | Result |
|-----------|--------|
| D-01–D-03: private/default-disabled, hard disabled short circuit, existing cache root | ✓ Cache-owned test-only activation; no CLI/config switch; disabled path creates nothing. |
| D-04–D-06: transactional versions, future preservation, typed recovery | ✓ Nine migration and four recovery fixtures; future bytes remain unchanged; rebuild requires verified cache ownership. |
| D-07–D-09: connection/lease policy and SQLite containment | ✓ WAL/FK/250 ms/immediate lease/read-only split; rusqlite and SQL stay inside the store module. |
| D-10–D-12: post-validation typed integration, private telemetry, answer parity | ✓ Kernel-only integration and six-mode JSON/exit parity; no public diagnostic. |
| D-13–D-16: complete fixtures, zero-I/O/parity, leak proof, locked performance boundary | ✓ Focused suites, full regression, seven leak tests, and three repeated first-open gate passes. |
| D-17: deliberately minimal schema | ✓ Only schema bootstrap bookkeeping exists; Phase 65 manifest/generation/fact/query scope is absent. |

### Behavioral Verification

| Behavior | Result | Status |
|----------|--------|--------|
| Cache-focused suite | 36 passed | ✓ PASS |
| Migration suite after review hardening | 9 passed | ✓ PASS |
| Store connection/contention/recovery suite | 11 passed | ✓ PASS |
| Kernel store/parity coverage | Default-disabled, enabled creation, and six-mode parity passed | ✓ PASS |
| Public boundary | 7 passed; external consumer compiled; prelude remained 115 | ✓ PASS |
| Real Phase 64 boundary | Three consecutive passes: 37–38 ms cold, about 41.7–42.0 MB RSS delta, 8 KiB store, unchanged `28cac8a32a5bb2a9` digest | ✓ PASS |
| Lint | `make lint` passed; post-review clippy passed with `-D warnings` | ✓ PASS |
| Full workspace regression | Library 2,421 passed / 1 intentional ignore; CLI 166; leak 7; bench 2; macros 11; examples and doctests passed | ✓ PASS |
| Plan completeness | 4 plans / 4 summaries, no incomplete or orphaned plan artifacts | ✓ PASS |
| Code review | Second pass clean: 0 blocker, critical, warning, or info findings | ✓ PASS |

The two migration fixtures added by review hardening and their focused suites passed after the recorded full workspace run. A final post-fix `make test` process also completed without a lingering process; the exact full-suite counts above are from the captured all-feature run and are not inflated to imply a second captured transcript.

### Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|----------|
| STORE-01 | ✓ SATISFIED | Private bundled-rusqlite facade and no-leak proof. |
| STORE-02 | ✓ SATISFIED | Versioned migrations, typed refusal/recovery, safe rebuild. |
| STORE-03 | ✓ SATISFIED | FK/WAL/bounded timeout/writer lease/read-only connections. |
| STORE-06 | ✓ SATISFIED | Providers and rules receive no SQL connection or vocabulary. |
| STORE-07 | ✓ SATISFIED | Store failures become private typed status and preserve policy answers. |
| STORE-08 | ✓ SATISFIED | Bounded contention returns `BusySkipped`; schema remains intact. |
| PERF-03 | ✓ SATISFIED | Disabled/skipped paths have no store I/O and no behavior drift. |
| PROD-01 | ✓ SATISFIED | Check JSON, diagnostics digest, and exit semantics are invariant. |
| VAL-02 | ✓ SATISFIED | Empty/previous/current/future/invalid/corrupt/rebuild fixture matrix passes. |

All nine Phase 64 requirement IDs are marked complete in `.planning/REQUIREMENTS.md`; no Phase 64 requirement is orphaned.

### Anti-Patterns Found

No `TODO`, `FIXME`, `XXX`, `todo!`, or `unimplemented!` placeholder exists in the Phase 64 implementation/gate files. No public store module/re-export or `rusqlite` reference exists in SDK, runner, CLI, or crate-root public sources. The review's same-user hard-link observation is explicitly accepted as outside the phase's symlink/containment threat claim and is not an unresolved implementation gap.

### Human Verification Required

None. The phase goal is covered by deterministic fixtures, public-boundary compilation/scans, source inspection, regression measurements, and full workspace tests.

### Gaps Summary

No gaps. Phase 64 establishes the private, disabled-by-default SQLite foundation without adding useful persisted product data yet. Its measurement is intentionally the committed deterministic tiny fixture, not a large-repository scale claim; later phases retain the locked real-repo scale gates. Phase 65 can now build complete-generation manifests and metadata mirroring behind this boundary.

---

_Verified: 2026-07-10_
_Verifier: Codex (inline GSD verifier; sub-agents not authorized)_

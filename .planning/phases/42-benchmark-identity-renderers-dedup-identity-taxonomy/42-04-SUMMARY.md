---
phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy
plan: 04
subsystem: testing
tags: [public-api, leak-gate, no_implicit_prelude, ci, semver, sdk-surface]

# Dependency graph
requires:
  - phase: 42-01
    provides: analysis::identity::* (IdentityRecord, IdentityKind, LanguageTag, SignatureDigest, IdentityRecordId, IdentityProviderOutput, IdentityStore) — all pub(crate); the private surface this gate proves never leaks
  - phase: 42-02
    provides: analysis::identity::render::{go_relstring,jelly_span}::render + eval::report::{JellyOracleCoverageSection,JellyUnmatchedSpan} — all pub(crate); also covered by the gate
provides:
  - tests/fixtures/public-surface-leak-probe/ — excluded probe crate compiling #![no_implicit_prelude] + use ::polint::sdk::prelude::*; with 97 witness fns
  - crates/polint/tests/public_surface_leak.rs — workspace leak-gate integration test with the locked ALLOWED_PRELUDE (97 entries), parse_prelude_reexports helper, parser self-test, and probe-tamper redundancy gate
  - .github/workflows/ci.yml leak-gate job on ubuntu-latest + macos-latest (fail-fast:false) — the v1.3 public-surface CI gate every Phase 43-54 inherits
affects: [43-reachability-roots, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, v1.3-semantic-graph]

# Tech tracking
tech-stack:
  added: []  # No new third-party deps — Approach B (direct cargo invocation), trybuild deliberately NOT added (T-42-SC discipline)
  patterns:
    - "Public-surface leak gate: an excluded probe crate with #![no_implicit_prelude] + a single use ::polint::sdk::prelude::*; glob, witnessing every allow-listed identifier; a workspace integration test shells out cargo build on the probe + snapshot-diffs the prelude block vs a locked ALLOWED_PRELUDE"
    - "Two-layer enforcement: (1) Rust's E0365 physically forbids pub use of a pub(crate) type into the public prelude; (2) the snapshot diff catches a genuinely-pub type added to the prelude as an UNSANCTIONED addition"
    - "Parser-of-the-parser self-test (BLOCKER #6): parse_prelude_reexports is proven by a synthetic-leak negative control + a clean-set positive control so a broken parser cannot silently hollow out the gate"
    - "Probe-tamper redundancy gate: ensure_no_private_namespace_in_probe asserts the probe still has #![no_implicit_prelude] + exactly one prelude-glob import + zero private-namespace path substrings"

key-files:
  created:
    - tests/fixtures/public-surface-leak-probe/Cargo.toml
    - tests/fixtures/public-surface-leak-probe/Cargo.lock
    - tests/fixtures/public-surface-leak-probe/src/lib.rs
    - crates/polint/tests/public_surface_leak.rs
  modified:
    - Cargo.toml
    - .github/workflows/ci.yml

key-decisions:
  - "Approach B (direct cargo invocation), not Approach A (trybuild): no new third-party dependency, consistent with Plans 01/02 no-new-deps discipline (T-42-SC); cargo --message-format=short build of the excluded probe gives rustc-level granularity without a UI-test crate"
  - "Test relocated from workspace-root tests/public_surface_leak.rs to crates/polint/tests/public_surface_leak.rs because the workspace root tests/ dir is not a crate; cargo test --package polint --test public_surface_leak only resolves a polint integration-test target under crates/polint/tests/"
  - "Probe import is use ::polint::sdk::prelude::*; (leading ::) because #![no_implicit_prelude] disables the implicit extern-prelude that a bare polint:: path would rely on; semantically identical single glob import. The redundancy test accepts both :: and bare forms"
  - "Probe carries its own committed Cargo.lock so --locked builds are deterministic in CI; the probe is workspace-excluded so it never participates in the workspace Cargo.lock or normal builds/lints/tests"
  - "ALLOWED_PRELUDE count locked at 97 with an explicit count assertion so any drift fails loudly and forces a deliberate milestone-close review (D-19)"

patterns-established:
  - "Leak-gate probe: #![no_implicit_prelude] + single ::polint::sdk::prelude::* glob + PhantomData::<Type> / value-binding witnesses; lifetime-bearing fact views and RuleCtx/RenderOpts/JsonReportMeta witnessed with <'static>"
  - "Source-of-truth allow-list lives in test source (not a snapshot file), enforced by a brace-depth-bounded parser that honors X as Y aliases and path::Leaf leaves"

requirements-completed: [IDENT-01, IDENT-02, IDENT-03]

# Metrics
duration: 25m
completed: 2026-05-29
---

# Phase 42 Plan 04: Public-Surface-Leak CI Gate Summary

**v1.3 public-surface-leak gate installed: an excluded `#![no_implicit_prelude]` probe crate compiles against `use ::polint::sdk::prelude::*;` with 97 witnesses, a workspace integration test snapshot-locks `ALLOWED_PRELUDE` (97 entries) against `sdk/mod.rs`, and a Linux+macOS `leak-gate` CI job (fail-fast:false) blocks any PR that leaks a v1.3 solver type through the SDK.**

## Performance

- **Duration:** ~25m
- **Started:** 2026-05-29T06:55:00Z (approx)
- **Completed:** 2026-05-29T07:21:00Z
- **Tasks:** 3
- **Files modified:** 6 (4 created, 2 modified)

## Accomplishments

- **Probe crate** (`tests/fixtures/public-surface-leak-probe/`): the proxy for an external rule crate. `#![no_implicit_prelude]` + a single `use ::polint::sdk::prelude::*;` glob + a `mod allowlist_witness` with one witness per allow-listed v1.0–v1.2 identifier (97 total: 91 `PhantomData::<T>` type witnesses, 5 free-fn value bindings, 1 const binding). Excluded from the workspace via `[workspace] exclude` so it never enters normal builds/lints/tests. Carries its own committed `Cargo.lock` for deterministic `--locked` CI builds.
- **Leak-gate integration test** (`crates/polint/tests/public_surface_leak.rs`): 5 `#[test]`s — `probe_crate_compiles_against_prelude_only` (cargo subprocess build of the probe), `allowlist_matches_prelude_source` (snapshot diff of the parsed prelude vs the locked `ALLOWED_PRELUDE`), `ensure_no_private_namespace_in_probe` (probe-tamper redundancy gate), `parser_self_test_detects_synthetic_leak` (BLOCKER #6 test-of-the-test), and `allowlist_has_no_duplicates_and_expected_count` (count lock at 97).
- **CI wiring**: new `leak-gate` job matrixed on `[ubuntu-latest, macos-latest]` with `fail-fast: false`, running `cargo test --package polint --test public_surface_leak --locked` on every PR. Independent per-platform pass required (D-18). Windows intentionally out of scope.

## Chosen Strategy

**Approach B (direct cargo invocation)** — no new third-party dependency. The gate shells out to `cargo build --message-format=short --locked` on the excluded probe crate and asserts a clean compile, then snapshot-compares the parsed prelude re-export block against `ALLOWED_PRELUDE`. trybuild (Approach A) was deliberately NOT added: the no-new-deps discipline that governed Plans 01/02 (T-42-SC) carries here, and the direct cargo invocation already gives rustc-level granularity. The threat-register row T-42-04-SC (trybuild legitimacy) is therefore vacuous.

## Exact File Paths Landed

- `tests/public_surface_leak.rs` → **relocated to** `crates/polint/tests/public_surface_leak.rs` (the workspace-root `tests/` is not a crate; `--package polint --test public_surface_leak` only resolves a polint integration-test target). `files_modified` in the plan listed the root path; this SUMMARY records the relocation.
- `tests/fixtures/public-surface-leak-probe/Cargo.toml`
- `tests/fixtures/public-surface-leak-probe/Cargo.lock`
- `tests/fixtures/public-surface-leak-probe/src/lib.rs`
- `Cargo.toml` (root — `[workspace] exclude` entry)
- `.github/workflows/ci.yml` (new `leak-gate` job)

## ALLOWED_PRELUDE Entry Count

**97 entries** — locked by `allowlist_has_no_duplicates_and_expected_count` with an explicit `assert_eq!(ALLOWED_PRELUDE.len(), 97, ...)`. Breakdown sourced verbatim from `crates/polint/src/sdk/mod.rs:28–53`: 55 `crate::core` types, 15 `crate::diagnostics` items (13 types incl. the `TextRange as DiagnosticRange` alias + 1 const + 1 free fn), 2 `crate::rule_error`, 1 `crate::sdk` free fn, 22 `crate::sdk::facts` views, 3 `crate::sdk::scope` free fns. Phase 43+ planners can detect drift at a glance: any change to this count requires a sanctioned milestone-close API change.

## CI Wiring

**Wiring B (dedicated `leak-gate` job).** Job name: `public surface leak gate (${{ matrix.os }})` → expands to `public surface leak gate (ubuntu-latest)` and `public surface leak gate (macos-latest)`. Matrix `[ubuntu-latest, macos-latest]`, `fail-fast: false`, step `run public-surface-leak gate` → `cargo test --package polint --test public_surface_leak --locked`. Runs on every PR in the fast lane; hard-blocks on failure (non-zero exit, no `continue-on-error`). No `--release`. Not added to the `msrv` job.

## User-Action Item (REQUIRED — T-42-04-10, repo-admin only)

**Add `public surface leak gate (ubuntu-latest)` AND `public surface leak gate (macos-latest)` to GitHub branch protection required checks on `main` and any `release/*` branches.** Only a repo admin can configure branch protection; this is the one piece of the gate that cannot be automated. Until both checks are required, a PR could merge with the gate failing.

## Negative-Control Proof

The executor proved both enforcement layers before reverting (no change committed):

1. **Compiler layer (E0365):** Injecting `pub use crate::analysis::identity::facts::IdentityRecord;` into the prelude failed to compile with `error[E0365]: IdentityRecord is only public within the crate, and cannot be re-exported outside`. A `pub(crate)` v1.3 type physically cannot be re-exported into the public prelude — the lib won't build.
2. **Snapshot layer:** Injecting `pub use crate::core::Capabilities;` (a genuinely `pub` type NOT in the allow-list) compiled, and `allowlist_matches_prelude_source` FAILED with the clear diff `UNSANCTIONED additions (in prelude, NOT in ALLOWED_PRELUDE): ["Capabilities"]` / `MISSING: []`.

`crates/polint/src/sdk/mod.rs` was restored to its pristine state (`git diff` empty) and the full gate re-ran green (5 passed) before any commit.

## Forward Note for Phases 43–54

Adding any new public type requires, in the SAME PR:
1. **Extend `ALLOWED_PRELUDE`** in `crates/polint/tests/public_surface_leak.rs` and bump the count assertion (currently 97). Without this, `allowlist_matches_prelude_source` trips on the unsanctioned addition.
2. **Reference a milestone-close review record** in `docs/API-VISIBILITY-PLAN.md`.
3. **Add a witness** in the probe's `allowlist_witness` module (`PhantomData::<NewType>` for types — `<'static>` if lifetime-bearing — or a value binding for free fns/consts). Without this, the probe fails to compile against the new prelude entry, also tripping the gate.

The gate's compile failure on an unsanctioned addition is the enforcement mechanism; reviewer discipline + the `docs/API-VISIBILITY-PLAN.md` promotion record is the policy layer.

## Task Commits

Each task was committed atomically:

1. **Task 1: Probe crate fixture + workspace exclude** - `f5b5fcb` (test)
2. **Task 2: Leak-gate integration test (Approach B)** - `64f1bf2` (test)
3. **Task 3: CI leak-gate job on Linux + macOS** - `4d3f13f` (ci)

**Plan metadata:** committed separately with this SUMMARY + STATE/ROADMAP/REQUIREMENTS updates (docs).

## Files Created/Modified

- `tests/fixtures/public-surface-leak-probe/Cargo.toml` - Probe manifest, one dep (`polint` path, default-features=false), publish=false, no features.
- `tests/fixtures/public-surface-leak-probe/Cargo.lock` - Committed lock for deterministic `--locked` builds.
- `tests/fixtures/public-surface-leak-probe/src/lib.rs` - `#![no_implicit_prelude]` + single prelude glob + 97 witnesses.
- `crates/polint/tests/public_surface_leak.rs` - Leak-gate test, `ALLOWED_PRELUDE`, parser + self-test + redundancy gate.
- `Cargo.toml` - `[workspace] exclude = ["tests/fixtures/public-surface-leak-probe"]`.
- `.github/workflows/ci.yml` - `leak-gate` job (ubuntu + macos, fail-fast:false).

## Decisions Made

See `key-decisions` in frontmatter. In brief: Approach B (no trybuild); test relocated to `crates/polint/tests/` for `--test` resolution; `::polint::` import under `no_implicit_prelude`; committed probe `Cargo.lock`; count locked at 97.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Probe import requires a leading `::` under `#![no_implicit_prelude]`**
- **Found during:** Task 1 (probe compile)
- **Issue:** With `#![no_implicit_prelude]`, the implicit extern-prelude is disabled, so a bare `use polint::sdk::prelude::*;` failed with `E0433: cannot find module or crate polint` (cascading to ~97 "cannot find type" errors).
- **Fix:** Changed the single glob import to `use ::polint::sdk::prelude::*;` (leading `::` extern-crate path). Semantically the identical single glob; the redundancy test `ensure_no_private_namespace_in_probe` accepts both `::polint` and bare `polint` forms and still enforces exactly one `use polint::` line.
- **Files modified:** tests/fixtures/public-surface-leak-probe/src/lib.rs (import + doc comment)
- **Verification:** Probe builds clean with `--locked`; gate green.
- **Committed in:** f5b5fcb (Task 1)

**2. [Rule 3 - Blocking] Probe needs its own committed `Cargo.lock` for `--locked` builds**
- **Found during:** Task 1 (probe `--locked` build)
- **Issue:** The excluded probe is an independent crate with no lock file, so `cargo build --locked` failed (`cannot create the lock file ... because --locked was passed`).
- **Fix:** Generated and committed `tests/fixtures/public-surface-leak-probe/Cargo.lock`; the probe's `target/` is already gitignored.
- **Files modified:** tests/fixtures/public-surface-leak-probe/Cargo.lock (new)
- **Verification:** `cargo build --manifest-path .../Cargo.toml --locked` exits 0; CI step uses `--locked`.
- **Committed in:** f5b5fcb (Task 1)

**3. [Rule 3 - Blocking] Test relocated from workspace-root `tests/` to `crates/polint/tests/`**
- **Found during:** Task 2 (test target resolution)
- **Issue:** The plan listed `tests/public_surface_leak.rs` (workspace root), but the root `tests/` dir is not a crate; `cargo test --package polint --test public_surface_leak` only discovers integration-test targets under `crates/polint/tests/`. The plan explicitly authorized this relocation if `--test` resolution required it.
- **Fix:** Placed the test at `crates/polint/tests/public_surface_leak.rs`; it uses `CARGO_MANIFEST_DIR` to derive the repo root and reach the probe + `sdk/mod.rs` robustly.
- **Files modified:** crates/polint/tests/public_surface_leak.rs (placement)
- **Verification:** `cargo test --package polint --test public_surface_leak --locked` discovers and runs all 5 tests green.
- **Committed in:** 64f1bf2 (Task 2)

---

**Total deviations:** 3 auto-fixed (all blocking — toolchain/path resolution). No bugs, no missing-critical, no scope creep.
**Impact on plan:** All deviations are mechanical (extern-crate path under `no_implicit_prelude`, lock-file determinism, integration-test target resolution) and preserve plan intent exactly: a single prelude-glob probe, a locked allow-list, and a Linux+macOS gate. The chosen Approach B avoids any new dependency.

## Threat Model Deltas

The two high-severity items required to block (per the plan's threat-model severity policy) are addressed:

- **T-42-04-04 (probe `#![no_implicit_prelude]` removal):** Mitigated by `ensure_no_private_namespace_in_probe`, which asserts the literal `#![no_implicit_prelude]` line is present, exactly one `use polint::` import exists and is the prelude glob, and zero private-namespace path substrings appear in probe code. Removing the attribute or loosening the import trips the test.
- **T-42-04-10 (branch-protection gap):** Mitigated by the **User-Action Item** above (repo-admin must add both `leak-gate` platform checks to branch protection on `main`/`release/*`). This is the only piece that cannot be automated; it is surfaced here and in STATE.md.

Other rows: T-42-04-01/02 mitigated by `allowlist_matches_prelude_source` + reviewer discipline; T-42-04-03 mitigated by the probe-via-prelude-glob design + Rust E0365 (proven in the negative control); T-42-04-05/06 mitigated by `ensure_no_private_namespace_in_probe` + the single-dep probe manifest; T-42-04-SC vacuous (Approach B, no trybuild); T-42-04-07/08 low/accepted; T-42-04-09 mitigated by deterministic, non-flaky test output.

## Threat Flags

None — this plan adds no network endpoints, auth paths, or schema changes at trust boundaries. It is a test/CI gate that reads source files and shells out to `cargo build` on a local path-dependency probe crate; no new runtime surface is introduced.

## Known Stubs

None — the gate is fully wired and green against the current tree (5/5 tests pass; probe builds in isolation; probe excluded from workspace builds).

## Issues Encountered

- **97 vs the planning anchor's "~85" estimate:** The plan's `<interfaces>` block estimated ~85 prelude identifiers; programmatic extraction from `sdk/mod.rs:28–53` yielded exactly 97. The acceptance threshold was `>= 80`, so 97 satisfies it; the count is now locked at 97 with an explicit assertion.

## User Setup Required

**Branch protection (repo-admin action).** See the **User-Action Item** section above — add `public surface leak gate (ubuntu-latest)` and `public surface leak gate (macos-latest)` to required checks on `main` and `release/*`. No external service configuration otherwise.

## Next Phase Readiness

- The v1.3 public-surface-leak gate is installed and green against the current tree, fulfilling Phase 42 ROADMAP Success Criterion #5 and proving (indirectly) that IDENT-01/02/03's `pub(crate)` claims hold at the workspace boundary.
- Phases 43–54 inherit the gate as a precondition for landing any new public type; the Forward Note above is the discipline they must follow.
- One open repo-admin action: wire the two `leak-gate` checks into branch protection (T-42-04-10).

## Self-Check: PASSED

All created files exist on disk (probe Cargo.toml/Cargo.lock/src/lib.rs, the integration test, this SUMMARY) and all three task commits (`f5b5fcb`, `64f1bf2`, `4d3f13f`) are present in git history.

---
*Phase: 42-benchmark-identity-renderers-dedup-identity-taxonomy*
*Completed: 2026-05-29*

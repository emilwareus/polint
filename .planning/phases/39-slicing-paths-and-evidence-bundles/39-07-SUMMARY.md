---
phase: 39-slicing-paths-and-evidence-bundles
plan: 07
subsystem: static-analysis-engine
tags: [rust, evidence, eval, diagnostics, public-api]

requires:
  - phase: 39-02-local-slicing
    provides: local evidence slices
  - phase: 39-03-bounded-paths
    provides: bounded paths and ranking
  - phase: 39-04-summary-context
    provides: summary expansion context
  - phase: 39-05-diagnostic-rendering
    provides: JSON/SARIF evidence rendering
  - phase: 39-06-extension-validation
    provides: extension evidence merge validation
provides:
  - Private compact evidence debug report
  - Native and extension evidence eval fixture coverage
  - Public-boundary proof for evidence internals
  - Evidence metadata precision ceiling regression
affects: [phase-39-eval, public-boundary-tests]

tech-stack:
  added: []
  patterns: [private debug report, synthetic eval taxonomy fixture, public no-leak guard]

key-files:
  created:
    - crates/polint/src/analysis/evidence/debug.rs
    - docs/facts/evidence.md
    - tests/eval-fixtures/evidence/expected.polint-eval.toml
    - tests/eval-fixtures/evidence/repo/.polint.toml
  modified:
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Evidence debug output stays private and compact: counts, status buckets, summary expansion keys, replay keys, unknown reasons, omitted regions, hidden node counts, and budget caps."
  - "Evidence eval coverage is represented by deterministic synthetic rows plus a manifest taxonomy covering the Phase 39 success criteria."
  - "Evidence remains an internal diagnostic/reporting feature, not a public SDK fact view."
  - "Exact internal evidence fact payloads are capped to setup-aware provider metadata so public validation does not report precision-ceiling errors."

patterns-established:
  - "Public no-leak tests cover JSON, AI-friendly, SARIF, help, SDK/runner sources, README, and evidence docs."
  - "Evidence docs can describe diagnostic/reporting behavior while explicitly denying current SDK fact-view support."
  - "Provider-order expectations must include polint.evidence before metrics."

requirements-completed: [SAE-PREC-04]

duration: 35min
completed: 2026-05-25
---

# Phase 39-07: Evidence Debug Eval Fixtures And Public Boundary Proof Summary

**Phase 39 is now closed with deterministic fixtures and a public-boundary proof**

## Performance

- **Duration:** 35 min
- **Completed:** 2026-05-25T15:24:54Z
- **Tasks:** 4
- **Files modified:** 11

## Accomplishments

- Added private `analysis::evidence::debug` report generation for deterministic evidence counts, statuses, summary expansion handles, replay keys, unknown reasons, omitted regions, hidden-node counts, and budget caps.
- Added evidence eval fixture coverage for native paths, extension merge deltas, required taxonomy markers, deterministic JSON, hidden counts, replay keys, and privacy constraints.
- Added `docs/facts/evidence.md` documenting evidence as internal diagnostic/reporting infrastructure rather than a public SDK fact view.
- Added `evidence_public_no_leak` to guard JSON, AI-friendly, SARIF, help, SDK/runner sources, README, and evidence docs from internal evidence API leaks.
- Updated provider-order expectations for `polint.evidence`.
- Fixed evidence metadata precision so exact internal evidence facts record setup-aware provider metadata, preventing public precision-ceiling diagnostics.

## Task Commits

1. **Tasks 1-4: Debug, eval fixtures, public no-leak proof, and metadata ceiling** - `ed0ea79` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/debug.rs` - Private deterministic evidence debug report.
- `crates/polint/src/analysis/evidence/mod.rs` - Registers debug module.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Provider-order expectation update.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider-order expectation update.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifest expectation update.
- `crates/polint/src/core/mod.rs` - Evidence metadata precision ceiling fix and regression.
- `crates/polint/src/eval/fixtures.rs` - Evidence fixture and extension delta coverage.
- `crates/polint/tests/cli.rs` - Evidence public no-leak regression.
- `docs/facts/evidence.md` - Internal evidence documentation and limits.
- `tests/eval-fixtures/evidence/` - Evidence eval manifest and repo config.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::evidence::debug --locked` - passed
- `cargo test -p polint --lib eval_native_fixture_runner_evidence_fixture_passes --locked` - passed
- `cargo test -p polint --lib eval_evidence_extension_fixture_passes --locked` - passed
- `cargo test -p polint --lib eval_evidence_manifests_cover_required_taxonomy --locked` - passed
- `cargo test -p polint --test cli evidence_public_no_leak --locked` - passed
- `cargo test -p polint --lib evidence_exact_rows_do_not_exceed_setup_aware_metadata_ceiling --locked` - passed
- `cargo test --workspace --locked` - passed
- `cargo clippy --workspace --all-targets --locked -- -D warnings` - passed

## Deviations from Plan

- The final fixture coverage uses deterministic synthetic evidence rows instead of wiring a new public eval adapter. This keeps evidence private while proving the required taxonomy, extension deltas, determinism, and privacy constraints.
- A precision-ceiling bug was found during full workspace verification and fixed in this plan because it affected public no-leak proof reliability.

## Issues Encountered

- Full workspace tests initially exposed `polint/internal` precision-ceiling diagnostics from exact evidence metadata. The fix caps exact evidence fact metadata to setup-aware for the `polint.evidence` provider while preserving internal payload precision labels.

## User Setup Required

None.

## Next Phase Readiness

Phase 39 is ready to mark complete. Phase 40 can start on external benchmark adapters and promotion gates for SAE-PROM-01.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*

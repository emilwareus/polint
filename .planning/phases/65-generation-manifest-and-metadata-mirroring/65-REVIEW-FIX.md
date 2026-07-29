---
phase: 65-generation-manifest-and-metadata-mirroring
review_path: .planning/phases/65-generation-manifest-and-metadata-mirroring/65-REVIEW.md
iteration: 3
status: all_fixed
findings_in_scope: 4
fixed: 4
skipped: 0
fix_commit_hashes:
  - fde65ff0748c708949a202d979c86aa8f6551b80
  - a1da836e24ba873d6e59f7c4289c03beae59c1b4
  - f7f593ace2feddfa83e00fb37a039273199e3ff0
  - 28689ca75049bc3d52d30bdb334aa70bab697c26
  - f86dab6729e29e9576fb04613c403861d572a0da
final_budget:
  product_test_files: 14
  additions: 2500
  deletions: 733
  addition_cap: 2500
tests:
  status: passed
  focused_targets: 13
  static_checks: 4
fixed_at: 2026-07-29
---

# Phase 65 R3 Code Review Fix Report

All four R3 findings are fixed. The implementation remains within the original
fourteen product/test files and finishes exactly at the hard addition cap.

## Fixed Issues

### CR-01: Supported `events` rules bypass sealed syntax-provider failures

**Status:** fixed
**Commits:** `fde65ff0`, `f7f593ac`

The runtime closure for `events` now derives its hard syntax providers from the
languages actually loaded for the run. Go-only repositories require only
`polint.go.syntax`, TS/JS-only repositories require only `polint.ts.syntax`,
mixed repositories require both, and an empty applicable corpus remains valid.
The refined-call upgrade remains outside this hard syntax closure.

The focused kernel regression forces both syntax outcomes non-success in a
Go-only run and proves the single capability diagnostic names only the
applicable Go provider. The production dispatch regression derives that events
blocker through the kernel closure, keeps the affected rule counter at zero,
and lets an unrelated rule run once before any blocked `RuleCtx` can be
constructed.

### WR-01: Validation downgrade ownership is reconstructed from diagnostics

**Status:** fixed
**Commits:** `a1da836e`, `f7f593ac`

Validation issues now retain structured reason and evidence separately from
their rendered diagnostic projection. Provider-specific validation sites
assign authoritative provider ownership directly; generic, cross-provider,
unknown-provider, and otherwise ambiguous validators remain global and
fail-closed. Downgrades read only the structured provider IDs and never scan
diagnostic evidence, fact references, stable keys, family labels, or rendered
messages.

Diagnostics are rendered from the structured issue while preserving the
original presentation fields, evidence ordering, stable fingerprint, and
deterministic report order. The focused regression mutates reason and evidence
rendering and proves the owned downgrade set is unchanged, while an unowned
issue still produces a global downgrade.

### CR-02: Failed scheduled call/refinement facts can reach `events`

**Status:** fixed
**Commit:** `28689ca7`

The runtime closure for `events` now adds `polint.calls` when call-site rows are
present and `polint.refined_calls` when refined-call rows are present, provided
the corresponding provider was scheduled rather than planned absent. Those
eligible enrichment providers must therefore have sealed success before an
events rule can dispatch. Events-only plans remain on the certified,
language-filtered syntax closure.

The production regression builds one events rule and one separate calls rule,
runs the real mixed plan, retains refined-call rows while inducing an
authoritatively owned refined-call validation rejection, and projects the
matching final failed outcome. Both requesting rule counters stay at zero while
an unrelated rule executes once, proving the events rule is blocked before its
`RuleCtx` can run. The mixed plan also exposed and fixed a sequencing defect:
the reachability identity is now read after its provider projection is
recorded, restoring deep calls-plan execution without changing the provider
contract.

### CR-03: Dependency-blocked `polint.calls` executes a fallback and can panic

**Status:** fixed
**Commit:** `f86dab67`

Calls execution now distinguishes provider readiness from a deliberately
omitted full calls plan. A selected calls provider that fails
`begin_provider` returns the neutral provider output immediately; it cannot
enter the lightweight call-site derivation or evaluate any readiness digest.
The lightweight path remains available only when the full CFG/calls plan is
not selected and both required upstream identities are present. With the
current identical semantic and CFG/calls gates, that path is deliberately
unreachable.

A private, test-only, thread-local one-shot seam records a semantic-MIR
execution failure before its outcome projection. The regression proves the
semantic-MIR outcome is an execution failure, CFG and calls close through
dependency blocking, calls retain no output identity or facts and record only
zeroed telemetry, the static manifest still seals completely, and the
independent metrics provider succeeds. The adjacent provider audit found no
second occurrence: the abstract-domain alternatives both require the same
successful readiness flag, while every other provider defaults directly when
blocked.

## Verification

- CR-03 blocked-calls execution-failure regression: 1 passed.
- `analysis_kernel::outcome::tests`: 6 passed.
- `analysis_kernel::validation::tests`: 9 passed.
- `analysis_kernel::tests::provider_outcomes`: 3 passed.
- `core::tests::run_rules_skips_rules_with_runtime_provider_blockers`: 1 passed.
- Mixed-plan production dispatch regression: 1 passed.
- Events-only planned-absent pipeline regression: 1 passed.
- Deep calls-plan CFG/refinement regression: 1 passed.
- Provider-backed events matching regression: 1 passed.
- Refined-call owned-validation regression: 1 passed.
- Public semantic-store vocabulary leak gate: 1 passed.
- Semantic-store JSON and exit parity: 1 passed.
- `eval::performance::tests`: 6 passed.
- `cargo fmt --all -- --check`: passed.
- Workspace/all-target/all-feature strict Clippy: passed.
- `cargo check --workspace --all-features --locked`: passed.
- Normal pre-commit `make lint` hooks passed for all five fix commits.
- `git diff --check c453748c..HEAD`: passed.
- Final scope audit: exactly 14 product/test files, 2,500 additions, 733
  deletions, no fifteenth file, and no public/store/CI contract expansion.

## Skipped Issues

None.

---

_Fixer: gsd-code-fixer_
_Iteration: 3_

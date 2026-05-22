---
quick_id: 260522-n3q
slug: fix-phase-33-review-findings-with-tdd-te
status: complete
completed: 2026-05-22
---

# Summary: Fix Phase 33 Review Findings

## Completed

- Replaced the synthetic SCC closure eval fixture runner with real kernel fixture observation across cold, warm, and no-cache runs.
- Added a recursive SCC regression test proving fixpoint convergence without unbounded digest growth.
- Fixed recursive SCC digest joins to treat semicolon-delimited digest contributions as an idempotent set.
- Strengthened summary validation diagnostics with family, stable key, field, and reason evidence.
- Tightened validation tests to assert the specific intended diagnostic reasons instead of accepting any `polint/internal` diagnostic.
- Removed the production `KernelRunReport` SCC closure result field and moved schedule/result debug data into a test-only snapshot.
- Updated metadata debug JSON to use the executed SCC schedule snapshot when available, avoiding recomputation from the final database.
- Strengthened Phase 33 public-boundary coverage to exercise real `check`, `check --help`, `inspect rule`, and `test` CLI surfaces, including stderr.

## Verification

- `cargo test --lib -p polint -- closure_recursive_scc_reaches_fixpoint_without_digest_growth`
- `cargo test --lib -p polint -- validate_summaries_rejects`
- `cargo test --lib -p polint -- eval_scc_closure_observes_schedule_demand_and_determinism`
- `cargo test -p polint --test cli -- phase33_internals_do_not_leak_from_real_public_cli_surfaces`
- `cargo test -p polint`
- `cargo fmt --check`
- `git diff --check`

## Review

Second-pass inline review found no remaining blocking issues in the changed files.

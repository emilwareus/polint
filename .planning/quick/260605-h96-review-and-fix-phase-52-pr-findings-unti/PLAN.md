---
quick_id: 260605-h96
slug: review-and-fix-phase-52-pr-findings-unti
status: implemented
created: 2026-06-05
---

# Review and Fix Phase 52 PR Findings

Task: Review and fix Phase 52 PR findings until two consecutive review rounds report no findings.

Initial known findings:

- `inspect unknowns` over-collects Go semantic diagnostics into unknown rows, causing duplicate package-error rows and misclassified provider quality warnings.
- `inspect unknowns` exposes internal provider and fact-family names despite the public JSON contract requiring no solver/provider internals.

Process:

- Fix known findings locally.
- Use subagent review passes for unknowns/CLI and refined-call/eval surfaces.
- Repeat review/fix until two consecutive clean review rounds.

Verification:

- Focused CLI and taxonomy tests.
- Relevant eval/refined-call tests.
- `cargo fmt --all -- --check`
- `cargo clippy -p polint --all-targets -- -D warnings`

---
status: complete
created: 2026-05-25
workflow: gsd-quick
---

# Fix Refined-Call Phase 37 Review Findings

## Goal

Close the three deep-review gaps from Phase 37:

1. Replace the synthetic refined-call extension/model fixture with a real runtime fixture.
2. Strengthen direct-vs-refined eval assertions so observed refined edges include precision and status.
3. Validate refined-call evidence/input provenance so invalid edges cannot pass internal validation.

## TDD Plan

1. Add or tighten tests/fixtures so they fail for the current gaps.
2. Implement the smallest code and fixture changes needed to pass.
3. Run focused refined-call/eval checks, clippy, formatting, and a second deep review.

## Outcome

- Removed synthetic observed rows from the refined-call extension/model fixture.
- Corrected the fixture extension payload to target the real TypeScript call site and emit a stable synthetic target.
- Tightened direct-vs-refined expected fact assertions with precision/status.
- Added validator coverage and enforcement for missing refined-call evidence and input stable keys.

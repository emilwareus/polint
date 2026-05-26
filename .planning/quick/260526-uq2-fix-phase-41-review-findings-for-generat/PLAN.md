---
status: complete
quick_id: 260526-uq2
slug: fix-phase-41-review-findings-for-generat
date: 2026-05-26
---

# Fix Phase 41 Review Findings

## Task

Fix review findings for generated fixtures and agent JSON contracts:

- Make generated Go rule fixtures self-validating.
- Ensure `polint unknowns` rejects capabilities whose unknown inspection is not implemented.
- Use explicit stable JSON labels instead of Rust `Debug` enum names.
- Sort fact samples before applying the public limit.

## Plan

1. Update Go `new-rule` scaffold body and negative fixture source.
2. Add public metadata for `unknowns` support and gate `polint unknowns` with it.
3. Add snake_case label helpers for resolved-import precision and unresolved reasons.
4. Sort full `facts sample` candidate rows before truncating to `--limit`.
5. Add regression coverage and run focused plus full CLI verification.


---
status: in_progress
created: 2026-05-24
workflow: gsd-quick
---

# Fix Phase 36 Review Findings

## Objective

Fix the Phase 36 type/value alias substrate review findings, add focused regression coverage, run verification, commit the result, and perform a deeper follow-up review.

## Tasks

1. [x] Restore Go MIR traversal for composite and function literal child expressions.
2. [x] Preserve allocation-token references when value output normalization reassigns allocation IDs.
3. [x] Stop representing ordinary place copies as call returns.
4. [x] Keep call-return access-path projections in the `CallSiteId` domain.
5. [x] Preserve unsupported Go provenance in emitted type facts.
6. [x] Add focused regression tests for the corrected behavior.
7. [x] Run formatting, targeted tests, broader tests/checks, commit, and run a deeper review.

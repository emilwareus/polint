---
quick: fix-phase38-39-review-findings
status: in_progress
date: 2026-05-25
---

# Fix Phase 38/39 Review Findings

Goal: fix the deep PR review blockers for private data-flow, slicing, evidence,
diagnostic rendering, eval coverage, and public-contract truthfulness using
test-driven changes.

Acceptance checklist:
- Data-flow summary projection does not fabricate flow and direct-call edges connect real call-site/place facts.
- Slicing/path queries preserve unknown/budget status, enforce structural validity, and have global search budgets.
- Evidence store/provider digest, validation, controlled-block anchoring, duplicate checks, and debug summaries are deterministic and complete.
- Structured evidence rendering is bounded, schema-defined, truthfully located, and does not accept unbounded arbitrary JSON.
- Real eval/no-leak coverage exercises evidence behavior instead of synthetic self-matching claims.
- Full formatting, focused tests, workspace tests, clippy, and deep review pass.

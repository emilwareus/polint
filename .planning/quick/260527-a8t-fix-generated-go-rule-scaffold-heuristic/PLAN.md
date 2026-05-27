---
status: complete
quick_id: 260527-a8t
slug: fix-generated-go-rule-scaffold-heuristic
date: 2026-05-27
---

# Fix Generated Go Rule Scaffold Heuristic Disclosure

## Task

Fix final PR review finding: the generated Go `new-rule` scaffold composes heuristic `BranchObligations<'_>` and `GoTests<'_>` facts but its diagnostic text did not disclose that the policy is heuristic.

## Plan

1. Update the generated Go scaffold diagnostic text to include heuristic disclosure.
2. Add CLI regression coverage that asserts the generated Go module contains the heuristic wording.
3. Run focused tests plus formatting checks, then re-review the PR diff.

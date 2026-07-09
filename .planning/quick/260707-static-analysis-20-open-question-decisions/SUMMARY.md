---
status: complete
---

# Summary: Static Analysis 2.0 Open Question Decisions

Validated the accepted recommendations against the current checkout and current
external source state, then locked them into
`research/static-analysis-2.0/OPEN-QUESTIONS.md`.

Key validation correction: the local data-flow provider already consumes
`refined_call_edges()`, so Q33 was decided as "keep refined-call data-flow under
the existing `dataflow` capability" rather than adding a new capability.

Verification:
- `rg -n "\\[open\\]|Recommendation" research/static-analysis-2.0/OPEN-QUESTIONS.md`
- `git diff --check`

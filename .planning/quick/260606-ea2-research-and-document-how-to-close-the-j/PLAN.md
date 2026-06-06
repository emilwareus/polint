---
quick_id: 260606-ea2
slug: research-and-document-how-to-close-the-j
status: planned
created: 2026-06-06
description: Research and document how to close the Jelly JS/TS callgraph performance gap
---

# Quick Task 260606-ea2: Research Jelly Gap Closure

## Objective

Research how polint can close the measured JS/TS call graph gap against Jelly
and write a dated implementation research report.

## Tasks

1. Compare Jelly's local source, public README, and relevant primary papers
   against polint's current TS/JS MIR, points-to, refined-call, and benchmark
   evidence.
2. Identify the concrete missing abstractions and order them by likely recall
   impact.
3. Write `performance/2026-06-06-jelly-gap-closure-research.md` with a staged
   implementation roadmap, risks, and regression strategy.

## Verification

- `rg -n "module execution|function-object|CommonJS|Promise|call/apply/bind|Jelly" performance/2026-06-06-jelly-gap-closure-research.md`
- `git diff --check`

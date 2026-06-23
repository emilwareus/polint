# Phase 61-01 Summary: Consolidated Policy Query Docs

## Completed

- Added `docs/facts/policy-queries.md` as the shared public reference for the v1.4 preview policy-query surface.
- Documented the one query-object style, preview view capabilities, pattern vocabulary, query structs, evidence header, precision/status semantics, unknowns, budgets, templates, and limits.
- Linked the shared reference from the fact index, events, calls, control-flow, data-flow, evidence, and capability docs.
- Added a docs coverage test to keep the shared preview contract present.

## Notes

- The docs explicitly keep raw CFG, call graph, data-flow graph, solver, provider, MIR, and evidence-store APIs out of the public rule-authoring surface.


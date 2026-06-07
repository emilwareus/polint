---
quick_id: 260607-bzh
slug: native-computed-js-recall
status: complete
created: 2026-06-07
description: Improve Jelly JS recall through native object/array models and computed property key flow
---

# Quick Task 260607-bzh: Native/Computed JS Recall

## Objective

Improve benchmark-visible Jelly JS/TS callgraph recall by implementing two
bounded semantics slices:

1. Constant string computed property keys such as `obj[p]` and
   `obj["arr" + "1"]`.
2. Standard native object/array flows used by Jelly's `tests/approx/natives.js`.

## Verification

- Focused `ts_value_flows` unit tests.
- Release external graph benchmark.
- Update `performance/2026-06-06-jelly-gap-closure-research.md`.
- Commit and push the completed slice.

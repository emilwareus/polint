# Summary: Graph Engine Benchmark Research

## Completed

- Created `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md`.
- Anchored the document in current real graph benchmark baselines:
  - Go x/tools RTA: 1 TP, 9 FP, 36 FN, 10.00% precision, 2.70% recall.
  - Jelly micro: 2 TP, 6 FP, 313 FN, 25.00% precision, 0.63% recall.
- Documented scanner engine implementation tracks with expected metric impact
  and time complexity:
  - benchmark identity and normalization;
  - reachability and root semantics;
  - Go semantic package/type/SSA frontend;
  - Go RTA/VTA provider;
  - JS/TS function and callsite inventory;
  - JS/TS scope, binding, import, and module graph;
  - JS/TS function-token propagation;
  - JS/TS object/property/prototype/class modeling;
  - validated native/framework/adaptation models;
  - unsupported/unknown taxonomy;
  - incremental cache and solver budgets.
- Linked the research document from `research/evaluation-harness/REPO-INDEX.md`.

## Verification

- Checked edited markdown for non-ASCII characters.
- No code tests were run because this task only changed documentation and GSD
  planning artifacts.

---
status: complete
date: 2026-05-26
---

# Summary

Implemented graph benchmarks as the primary external benchmark path.

Changes:
- Added `go_x_tools_callgraph` adapter for Go x/tools RTA `WANT` call-edge fixtures.
- Added `jelly_callgraph` adapter for Jelly JS/TS call graph JSON `fun2fun` and `call2fun` edges.
- Added supported suite manifests for both graph suites.
- Added an internal external-suite runner that materializes scratch workspaces,
  runs `AnalysisKernel`, normalizes observed call graph edges, and writes
  ignored reports under `.context/graph-benchmarks/`.
- Added graph normalizers for Go x/tools and Jelly plus explicit unresolved-call
  unknown facts.
- Added a conservative direct-call lexical fallback for unique function/method
  names when semantic references are missing.
- Added the graph adaptation-agent prompt at
  `research/evaluation-harness/prompts/graph-adaptation-agent.md`.
- Updated evaluation harness docs so graph benchmarks are primary and security suites are secondary.
- Added a manifest-validation test covering committed suite TOML files.

Local clone counts:
- Go x/tools RTA callgraph: 5 cases, 37 expected call edges.
- Jelly JS/TS callgraph micro: 76 cases, 1,479 expected call edges.

Fast-tier baseline reports:
- Go x/tools RTA callgraph: TP 1, FP 9, FN 36, precision 0.1000,
  recall 0.0270, unknowns 26, output hash `80d0165b07a079fc`.
- Jelly callgraph micro: TP 2, FP 6, FN 313, precision 0.2500,
  recall 0.0063, unknowns 28, output hash `27214e12046c2f18`.

Verification:
- `cargo test -p polint eval::external --locked`
- `cargo test -p polint committed_evaluation_suite_manifests_parse_and_validate --locked`
- `POLINT_WRITE_GRAPH_BENCH=1 cargo test -p polint eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`
- `cargo test -p polint direct_calls --locked -- --nocapture`
- `cargo test -p polint refined_calls --locked -- --nocapture`
- `cargo test -p polint cfg_core --locked -- --nocapture`
- `cargo clippy -p polint --all-targets --locked -- -D warnings`

No commit was created.

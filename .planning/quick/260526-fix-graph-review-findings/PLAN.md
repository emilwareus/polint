# Quick Task: Fix Graph Review Findings

## Goal

Fix the two review findings from the graph benchmark implementation:

- name-only direct-call fallback must not resolve dynamic member calls;
- Go x/tools txtar materialization must not reuse scratch directories for case
  ids that sanitize to the same string.

## TDD Plan

1. Add or update tests that fail on the current behavior.
2. Apply the smallest implementation fixes.
3. Rerun focused direct-call and external graph adapter tests.

## Verification

- `cargo test -p polint analysis::calls::direct --locked -- --nocapture`
- `cargo test -p polint eval::external::go_x_tools_callgraph --locked -- --nocapture`
- `cargo test -p polint --lib eval --locked`

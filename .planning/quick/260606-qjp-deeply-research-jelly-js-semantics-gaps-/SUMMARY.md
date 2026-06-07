---
quick_id: 260606-qjp
slug: deeply-research-jelly-js-semantics-gaps-
status: complete
completed: 2026-06-06
commit: pending
---

# Quick Task 260606-qjp Summary

Deeply reviewed Jelly's implementation and updated the gap-closure report with
the source-level architecture, test evidence, and an implementation plan focused
on the remaining recall blockers.

## Files Changed

- `performance/2026-06-06-jelly-gap-closure-research.md`
- `crates/polint/src/analysis/calls/ts_value_flows.rs`
- `.planning/quick/260606-qjp-deeply-research-jelly-js-semantics-gaps-/PLAN.md`
- `.planning/quick/260606-qjp-deeply-research-jelly-js-semantics-gaps-/SUMMARY.md`

## Results

- Identified Jelly's core design as a module/function call graph over tokens,
  constraint variables, object/prototype storage, native models, and post-solver
  recovery patches.
- Ported three representative Jelly obligations into ignored Rust probes:
  `Promise.allSettled` result object properties, async generator iterator result
  objects, and receiver-side effects across calls.
- Confirmed the probes currently fail with empty actual edge lists, showing the
  missing semantics clearly without breaking normal CI.
- Updated the implementation plan toward a private JS token heap and fixpoint
  propagation layer rather than more local syntactic recognizers.

## Verification

Passed:

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`

Failed as expected:

- `cargo test -p polint analysis::calls::ts_value_flows::tests::jelly_gap --lib --locked -- --ignored --nocapture`

The ignored failure output shows all three actual edge lists are empty while the
expected Jelly-style edges are present.

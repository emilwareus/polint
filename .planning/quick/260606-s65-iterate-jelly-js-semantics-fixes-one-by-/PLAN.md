---
quick_id: 260606-s65
slug: iterate-jelly-js-semantics-fixes-one-by-
status: in_progress
created: 2026-06-06
description: Iterate Jelly JS semantics fixes one by one with performance report updates
---

# Quick Task 260606-s65: Iterate Jelly JS Semantics Fixes

## Objective

Implement Jelly-backed JS/TS recall improvements one semantic slice at a time,
updating the performance report and committing after each verified slice.

## Iteration Order

1. `Promise.allSettled` result objects: make `value` and `reason` properties
   flow through `.then` handler parameters.
2. Async generator iterator/result objects: make `.next().then(res =>
   res.value())` and `for await` bind yielded values.
3. Receiver-side effects: bind member-call receiver `this` and preserve
   same-file writes such as `this.a2 = () => {}`.
4. Re-run focused probes and, when a slice is meaningful, the external graph
   benchmark.

## Verification Per Slice

- `cargo test -p polint analysis::calls::ts_value_flows --lib --locked`
- targeted ignored probe command for the slice under work
- append progress to
  `performance/2026-06-06-jelly-gap-closure-research.md`
- commit and push each complete slice

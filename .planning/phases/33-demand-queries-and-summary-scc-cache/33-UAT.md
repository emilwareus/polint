---
status: passed
phase: 33-demand-queries-and-summary-scc-cache
source:
  - 33-01-SUMMARY.md
  - 33-02-SUMMARY.md
  - 33-03-SUMMARY.md
  - 33-04-SUMMARY.md
  - 33-05-SUMMARY.md
  - 33-06-SUMMARY.md
  - 33-07-SUMMARY.md
started: 2026-05-22T20:01:40Z
updated: 2026-05-24T05:37:33Z
---

## Tests

### 1. Direct summaries layer cache
expected: Running the Phase 33 direct-summary cache checks shows cold runs writing summary payloads and warm runs restoring them with the same output digest, without changing public check output.
result: [passed]

### 2. Demand query infrastructure and trace
expected: Demand query tests show stable QueryKey/SummaryKey behavior, in-run memoization, deterministic trace rows, cache hit/miss status, input digests, and result digests for executed queries.
result: [passed]

### 3. Deterministic SCC discovery
expected: SCC discovery over direct call targets groups recursive and non-recursive functions correctly, recognizes self-calls and mutual recursion, and orders independent SCCs by stable keys.
result: [passed]

### 4. SCC closure, fixpoint, and backdating
expected: Summary SCC closure applies callee summaries to callers, iterates recursive SCCs with bounded convergence, reports budget exhaustion honestly, and stops invalidation when recomputed SCC digests are unchanged.
result: [passed]

### 5. Extension-aware quarantine
expected: Synthetic extension digest tests quarantine only extension-keyed cache entries on digest or manifest changes, never native facts, and allow matching quarantined entries to be reinstated.
result: [passed]

### 6. Validation and internal debug output
expected: Validation rejects stale or malformed demand/SCC results before reuse, and internal debug JSON exposes demand_queries and scc_schedule sections without promoting them to public CLI, SDK, or docs surfaces.
result: [passed]

### 7. Eval fixture and public boundary proof
expected: The direct-summaries SCC eval fixture passes, public no-leak tests pass, and rendered `polint check --format json`, help text, SDK, runner, docs, and README stay free of Phase 33 internal markers.
result: [passed]

## Summary

total: 7
passed: 7
issues: 0
pending: 0
skipped: 0
blocked: 0

## Validation Commands

- `cargo test --lib -p polint -- scc direct_summaries large_stdout wire_fact_span extension eval_extension provider_order no_leak --nocapture`
- `cargo test -p polint --test cli -- extension_no_leak --nocapture`
- `cargo clippy -p polint -- -D warnings`

## Gaps

[none]

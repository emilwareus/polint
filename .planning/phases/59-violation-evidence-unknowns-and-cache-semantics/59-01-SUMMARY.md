# Phase 59-01 Summary: Policy Evidence Schema and Query Digests

## Completed

- Added a shared preview policy query version and internal operation labels for events, calls, control-flow, and data-flow query families.
- Added deterministic query digest helpers for `EventPattern`, `ReachQuery`, `GuardQuery`, `LifecycleQuery`, and `FlowQuery`.
- Extended `PolicyViolation` so every diagnostic emitted through `diagnostic(rule_id, message)` includes the normalized evidence header:
  - `policy_query`
  - `policy_query_version`
  - `query_digest`
  - `policy_status`
  - `policy_precision`
- Centralized sorting and deduplication for all policy query result vectors before they are returned to rules.

## Notes

- Query-specific evidence such as roots, paths, sources, sinks, barriers, and budget state remains in the existing per-query evidence keys.
- The public rule-authoring syntax did not change; rules still request typed views and report `PolicyViolation` diagnostics.


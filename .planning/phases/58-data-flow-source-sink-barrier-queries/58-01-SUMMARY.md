# Phase 58-01 Summary: Data-Flow Query Internals

## Result

Completed.

`DataFlow<'_>::forbidden(FlowQuery)` is now backed by a private policy-query
implementation for the documented Phase 58 preview scope. Rule authors still
use the single query-object API from `polint::sdk::prelude::*`; raw data-flow
graph IDs and stores remain internal.

## Delivered

- Added crate-private pattern accessors for source, sink, and barrier patterns.
- Changed the SDK `DataFlow<'_>` view into a thin bridge to `policy_queries`.
- Added private source, sink, and barrier matching for:
  - HTTP request trust-boundary sources.
  - Explicit secret-like source names.
  - Exact call sinks.
  - Logger sinks.
  - Explicit call barriers.
- Wired bounded private path search through `find_paths` using query depth and
  path caps.
- Added deterministic violation evidence for found, uncovered, unknown, and
  budgeted paths.
- Added focused unit coverage for found paths, barrier-covered paths, depth
  budget behavior, unknown paths, and no-store behavior.

## Verification

- `cargo test -p polint --lib data_flow_forbidden --locked` passed.
- Covered by the full library regression: `cargo test -p polint --lib --locked`
  passed with 2308 tests.


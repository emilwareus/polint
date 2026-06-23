# Phase 59-03 Summary: Cache, Determinism, Docs, and Closeout

## Completed

- Added policy-query preview version digest parts to analysis-plan identity for policy capabilities so schema changes invalidate relevant planning/cache decisions.
- Added tests for query digest stability and option sensitivity.
- Documented normalized policy evidence and cache boundaries in `docs/facts/evidence.md` and the policy capability docs.
- Updated generated skill text so agents know policy diagnostics expose the normalized evidence header and preview policy unknowns.
- Ran focused and broad verification, including the public-surface leak gate, clippy, docs, CLI regressions, and the full `polint` library suite.

## Notes

- Runtime query parameters are represented by `query_digest` evidence. Provider cache identity still comes from lifecycle inputs, solver budgets, rule/config/model inputs, and provider output digests.
- A public-surface guard initially caught an internal name containing `QueryKind` in `sdk/policy.rs`; the type was renamed without changing public behavior.


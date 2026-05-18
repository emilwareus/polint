# Summary

Fixed the follow-up cache review findings:

- Manifest schema is now `polint-layer-cache-manifest-2` after adding the required `dependency_index` field.
- Exact cache misses now scan same-layer manifests, run their stored dependency index through the invalidation planner, and evict stale candidates without changing miss accounting.
- `LayerCacheStore::write_json` now rejects unsupported manifest metadata instead of writing manifests the read path would later reject.

Verification passed with the commands listed in `PLAN.md`.

# Quick Summary: Fix Phase 33 SCC Closure Review Findings

## Result

Fixed the Phase 33 SCC closure review findings with regression coverage.

## Changes

- Non-recursive SCC processing now merges each SCC before callers run, so chains like `A -> B -> C` observe already-closed callee summaries.
- Leaf SCCs with no callees no longer rewrite local direct summaries as interprocedural `SetupAware` output.
- SCC closure now persists final per-SCC output digests through the internal cache and records warm unchanged SCCs as demand-query cache hits.
- Backdating digests are computed from the final summaries present in the SCC, including preserved leaf direct summaries.
- Eval fixtures were updated for the corrected local precision and new SCC demand-query cache-hit accounting.

## Verification

- `cargo test --lib -p polint -- closure_leaf_scc_digest_tracks_preserved_direct_summary scc_closure_with_cache_backdates_warm_run_and_records_hit_trace`
- `cargo test -p polint`
- `cargo clippy -p polint --all-targets -- -D warnings`
- `cargo fmt --check`
- `git diff --check`

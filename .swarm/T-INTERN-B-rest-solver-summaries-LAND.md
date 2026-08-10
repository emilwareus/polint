# T-INTERN-B REST SOLVER/SUMMARIES LAND

## Scope

Migrated identity-owning solver/summaries fields from `String` to `StableKeyId`,
and renamed projected/debug/sort text away from the structural `stable_key`
literal:

- `analysis/solver`: `DerivedEdgeFact`, `ContributingFact`, Go RTA
  callsite/dispatch identities, TS points-to callsite/constraint identities,
  plus producers/stores/validation/digest/provenance consumers.
- `analysis/summaries`: `SummaryFact`, `SummaryEventFact`,
  `FunctionSummaryState`, SCC `member_stable_keys`, builders/stores/provider
  digests.
- `analysis/slicing`: path/summary step projected fields → `*_text`.
- `analysis_kernel/incremental/keys.rs`: `SummaryKey.callable_stable_key_text`
  (serde rename keeps serialized cache key name `callable_stable_key`).
- `policy_queries.rs` `ControlOrder.stable_key_text`: resolved-text ordering
  only; never `StableKeyId` allocation order.

Construction uses `stable_key_from_parts` / `interner.intern`. Sorting, digests,
diagnostics, FactMeta registration text, and debug output resolve via
`interner.resolve` / `AnalysisDb::resolve_stable_key`. Debug summary/SCC rows
use `*_text` + `serde(rename)` for byte-identical JSON.

Out of scope (unchanged): `FactMeta` / `stable_key_owners` (T-INTERN-C), solver
densification, public API paths / leak allowlist.

## Structural delta

| Metric | Count |
|---|---|
| `rg -c 'stable_key: String' crates/polint/src` before | **63** |
| after | **41** |
| delta | **−22** |

Scoped family owning declarations under solver/summaries/slicing/keys/ControlOrder
are gone.

## Named regression

`solver_derived_call_edges_project_to_refined_calls` failed on tip
(`output.edges.len() == 0`).

Diagnosis:

- Fixture `db_with_solver_edge` used `CallSiteFact.stable_key = StableKeyId(0)`
  while contributing facts referenced text `"call-site:callee"`.
- Projection indexes callsites by `resolve(site.stable_key)` text, so the
  lookup missed and produced zero refined edges.
- Introduced in calls slice `a465e5c2` (StableKeyId(0) hardcoded). Same fixture
  shape is present at `d7dafb90` (pre rest-small-facts); rest-small-facts did
  not introduce the mismatch.
- Fix: intern `"call-site:callee"` for the callsite; migrate contributing /
  derived-edge keys to `StableKeyId` and resolve both sides for projection /
  `input_stable_keys`.

## Verification

- `cargo check -p polint --all-targets --all-features --locked` — PASS
- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` — PASS
- Focused: `analysis::solver::` (116), `analysis::summaries::` (106),
  `analysis::slicing::` (32), `solver_projection_tests::` (5), named regression — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS (8; nested
  probe cargo reused warm default target)
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS (12)
- Golden (`POLINT_GO_SYMBOLS=target/debug/polint-go-symbols`) — PASS;
  diagnostics byte-identical on first successful sidecar run. Cost baselines
  not regenerated.

## Remaining `stable_key: String` (41) — next buckets

| Bucket | Paths (approx hits) |
|---|---|
| residual/debug | `analysis_kernel/debug.rs` (7), `entrypoints/debug.rs` (5), `data_flow/debug.rs` (4), `semantic_graph/debug.rs` (3), `evidence/debug.rs` (1) |
| residual/wire | `go/semantic/protocol.rs` (2), `extensions/protocol.rs` (1) |
| residual/other | `semantic_graph/build.rs` (5), `symbol_graph/{model,ts}.rs` (5), `data_flow/validate.rs` (1), `cli/mod.rs` (1) |
| T-INTERN-C | `core/metadata.rs` (2), `analysis_kernel/metadata.rs` (4) |

## Next

T-INTERN-B still open for residual/debug/wire buckets above. T-INTERN-C remains
blocked on full B (`stable_key: String` count → 0 including FactMeta).

## Landing

- Feat commit: `1a2d62387820060178dba5f5626c8fc645fef88a`
- Swarm land: `f3284061`

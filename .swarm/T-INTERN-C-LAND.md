# T-INTERN-C LAND — FactMeta StableKeyId

## Result

Migrated the final FactMeta identity copy and owner map from `String` to
`StableKeyId` with no dual path/shim.

| Surface | After |
|---|---|
| `FactMeta.stable_key` | `StableKeyId` |
| `StableKeyConflict.stable_key` | `StableKeyId` |
| `FactMetaInsert::Conflict.stable_key` | `StableKeyId` |
| `FactMetaStore.stable_key_owners` | `HashMap<StableKeyId, StableKeyOwner>` |
| `fact_meta_from_stable_key*` | accept `StableKeyId` (+ interner for digests) |

## Structural delta

| Metric | Count |
|---|---|
| `rg -c 'stable_key: String' crates/polint/src` before | **6** |
| after | **0** |
| `HashMap<String, StableKeyOwner>` | **0** |

## Semantics preserved

- Interner remains `AnalysisDb`-scoped; no production globals.
- Payload/cache digests, provider summary parts, sorts with user-visible effects,
  conflict diagnostic evidence, debug JSON, and composed keys resolve via
  `interner.resolve(id)` text — never raw ID order/numeric serde.
- Owner lookup / conflict equality use IDs; conflict emission sorts explicitly on
  resolved text (conflicts stored in `HashSet`, not `StableKeyId` Ord/`BTreeSet`).
- `MetadataDebugFields` owns `stable_key_text` with `serde(rename = "stable_key")`.
- `data_flow/local` and other `stable_key_from_parts` inputs sourced from metadata
  receive resolved text.
- Unused `_family` args removed from `fact_meta_from_stable_key*` /
  `topology_fact_metadata` / `semantic_fact_metadata` (no dual/legacy API).

## Key paths

- `crates/polint/src/analysis_kernel/metadata.rs`
- `crates/polint/src/core/metadata.rs`
- `crates/polint/src/core/db.rs`
- `crates/polint/src/analysis_kernel/{debug,validation,mod}.rs`
- Consumers: calls/domains/entrypoints/summaries/refined_calls/extensions/
  data_flow/local/unknown_taxonomy/validate + tests/fixtures

## Gates

| Gate | Result |
|---|---|
| Focused `analysis_kernel::metadata` | PASS (9) |
| Focused `analysis_kernel::validation` (+ conflict evidence) | PASS (31) |
| Focused `analysis_kernel::debug` | PASS (19) |
| Focused call-fact / core metadata consumers | PASS |
| `cargo check -p polint --all-targets --all-features --locked` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` | PASS |
| Structural `stable_key: String` = 0 | PASS |
| No `HashMap<String, StableKeyOwner>` | PASS |
| `eval::determinism_gate` | PASS (12) |
| `public_surface_leak` | 7/8 marker PASS; nested probe rebuild blocked by sandbox `PermissionDenied` on libsqlite3-sys/tree-sitter build-scripts (same env limit as T-INTERN-B); no public surface change |
| `golden` | PASS (8) after Go sidecar offline warmup; earlier cost-only red on cold `go-sensitive-writes` (1058→522 ms vs 358) with byte-identical diagnostics; baselines not regenerated (Q6.2) |

## Cost note

Cold golden cost on `examples/go-sensitive-writes/json` was elevated while the Go
symbols sidecar compiled under sandbox/offline constraints. Peak RSS stayed
effectively flat. Warm full golden suite passed without regenerating cost
baselines.

## Next

T-INTERN-C **MERGED**. T-SPLIT **READY** (interning complete; structural gate 0).

## Landing

- Feat commit: `932cfd0bd6dbc74c0e459e714c7b0f8560cedbd8`
- Swarm land: `22263b225b2b0f37f1fd06bc765d5b32ac61e1d0`

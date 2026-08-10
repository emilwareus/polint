# T-INTERN-B REST BOUNDARIES LAND

## Scope

Final T-INTERN-B rename-only pass on resolved-text / wire / debug / construction
boundaries. No fact identity storage changes, no dual fields, no public path
changes. Wire and debug JSON keys stay byte-identical via `serde(rename)`.

Touched surfaces:

- `analysis_kernel/debug.rs` — semantic/MIR/CFG/call/abstract-domain/extension debug rows
- `analysis/{entrypoints,data_flow,semantic_graph,evidence}/debug` (+ data_flow validate issues)
- `go/semantic/protocol.rs` + `lower.rs` wire frames
- `analysis/extensions/protocol.rs` + provider wire ingest
- `analysis/semantic_graph/build.rs` — `intern_node`, TS token-source flow helpers
- `symbol_graph/{model,ts}.rs` — insert helpers and alias/resolution construction params
- `analysis/solver/{facts,provenance}.rs` — stable payload structs (resolved text only)
- `cli/mod.rs` — `DerivedEdgeProvenanceView` resolved text field

Untouched (T-INTERN-C): `FactMeta`, `StableKeyConflict`, conflict enum, sentinel,
`core/metadata.rs` `fact_meta_from_stable_key` params — six `stable_key: String`
hits remain, all C-owned.

## Structural delta

| Metric | Count |
|---|---|
| `rg -c 'stable_key: String' crates/polint/src` before | **41** |
| after | **6** |
| delta | **−35** |

All six remaining hits:

| File | Line | Context |
|---|---|---|
| `analysis_kernel/metadata.rs` | 246 | `FactMeta.stable_key` |
| `analysis_kernel/metadata.rs` | 297 | `StableKeyConflict.stable_key` |
| `analysis_kernel/metadata.rs` | 308 | conflict enum variant |
| `analysis_kernel/metadata.rs` | 423 | conflict sentinel |
| `core/metadata.rs` | 262 | `fact_meta_from_stable_key` params |
| `core/metadata.rs` | 284 | `fact_meta_from_stable_key` params |

## Verification

- `cargo check -p polint --all-targets --all-features --locked` — PASS
- `cargo fmt --all -- --check` — PASS (after one-line build.rs wrap)
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` — PASS
- Focused modules: entrypoints (112), semantic_graph (77), evidence (93),
  go::semantic (70), extensions (49), symbol_graph (99), solver/provenance (6) — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS (12)
- `cargo test -p polint --test golden --locked` — PASS (8); diagnostics
  byte-identical; cost baselines not regenerated
- `public_surface_leak` — 7/8 marker tests PASS; nested probe build blocked in
  sandbox (tree-sitter/sqlite build-script `PermissionDenied`); no public surface
  change in this slice
- Pre-existing failures unchanged on tip: `calls_debug_json` metadata fixture,
  `dense_id_is_omitted_from_stable_payload` (numeric-id substring in resolved text)

## Next

T-INTERN-B **complete**. T-INTERN-C **unblocked** (`FactMeta` / `stable_key_owners`).

## Landing

- Feat commit: `38c55c3dadae8e2cb17f7666e4964f9baccc3120`
- Swarm land: `e76cd1e4957677fa7a6dabe0d1176e2be5f144ba`

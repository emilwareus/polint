# T-INTERN-B REST SMALL FACTS LAND

## Scope

Migrated identity-owning `stable_key` fields from `String` to `StableKeyId` for:

- `analysis/identity`
- `analysis/extensions` (candidate/accepted/rejected stores; wire protocol stays `String`)
- `analysis/adaptation`
- `analysis/types`
- `analysis/values`
- `analysis/points_to`
- `analysis/aliases`
- `analysis/access_paths`
- `analysis/reachability`
- `analysis/refined_calls` (including `TargetRefinement` and consumers)

Construction uses `stable_key_from_parts` / `interner.intern`. Sorting, digests,
diagnostics, FactMeta registration text, and debug output resolve via
`interner.resolve` / `AnalysisDb::resolve_stable_key`. Debug report rows for
reachability and refined calls were renamed to `stable_key_text`.

Out of scope (unchanged): `FactMeta` / `stable_key_owners` (T-INTERN-C), solver
densification, public API paths.

## Structural delta

| Metric | Count |
|---|---|
| `rg -c 'stable_key: String' crates/polint/src` before | **81** |
| after | **63** |
| delta | **−18** |

Scoped family owning declarations are gone except
`extensions/protocol.rs` `ExtensionFactCandidateWire.stable_key: String` (wire).

## Verification

- `cargo check -p polint --all-targets --all-features --locked` — PASS
- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS (8 tests; nested
  probe cargo reused a warm target under sandbox build-script constraints)
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS (12 tests)
- Golden diagnostic sets — byte-identical on both harness runs (with
  `POLINT_GO_SYMBOLS` pointing at a prebuilt sidecar when the sandbox blocked
  Go module downloads).
- Golden cost — first characterization run cost-red on
  `examples/go-sensitive-writes/json`: 520 ms vs 358 ms baseline (+45.3%);
  peak RSS effectively flat. Required retry: PASS with diagnostics still
  identical. Cost baselines not regenerated (DECISION Q6.2).

## Next

T-INTERN-B still open for remaining rest families (summaries/solver/slicing and
other residual `stable_key: String` owners). T-INTERN-C remains blocked on full B.

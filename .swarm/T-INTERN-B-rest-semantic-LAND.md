# T-INTERN-B REST SEMANTIC LAND

## Result

- Semantic symbol-graph facts and semantic graph node, edge, and constraint
  identities now retain `StableKeyId` instead of owned stable-key strings.
- Semantic graph builder indexes retain interned IDs; cache payloads use
  explicit text-only serialization rows and re-intern identities on restore.
- Normalization, validation, digests, diagnostics, debug output, and downstream
  call sites resolve IDs to text wherever textual ordering or display is
  required.
- `FactMeta`, frontend inventory/binding/scope/object-model facts, and unrelated
  remaining fact families were not migrated.
- No dual string/ID fields or compatibility shims were added.

## Structural delta

- `rg -c 'stable_key: String' crates/polint/src` totals 149 remaining
  declarations, down from 178 at the calls-integrated base.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --all-features --locked` —
  PASS
- `cargo test -p polint --lib eval::determinism_gate --all-features --locked` —
  PASS
- Semantic graph focused tests — PASS
- Golden diagnostic sets — PASS (byte-identical on both stock-harness runs).
- Golden cost sidecar — the known `examples/go-sensitive-writes/json`
  sensitivity remained cost-red: 533 ms versus the 358 ms baseline (+48.9%)
  on the first run and 488 ms (+36.3%) on the required retry. Peak RSS remained
  effectively flat. Per DECISION Q6.2, the retry result is recorded and cost
  baselines were not regenerated.

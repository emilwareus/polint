# T-INTERN-B REST MODULE GRAPH LAND

## Result

- Module-graph topology facts now retain `StableKeyId` for primary and related
  stable identities; the owned string identity fields were deleted.
- Topology normalization, dependency digests, validation diagnostics, eval
  observation, and tests resolve interned identities when text or lexical
  ordering is required.
- Module-graph and module-topology cache payloads use canonical, text-sorted
  identity tables and re-intern IDs on restore. Their schemas advanced to v3
  and v2 respectively.
- No dual string/ID fields or compatibility shims were added.

## Structural delta

- `rg -c 'stable_key: String' crates/polint/src/module_graph` — 0 matches.
- `rg -c 'stable_key: String' crates/polint/src` — 112 remaining declarations
  outside this locked fact family.

## Verification

- `cargo fmt --all -- --check` — PASS.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS.
- `cargo test -p polint module_graph --all-features --locked` — PASS (147
  focused tests).
- `cargo test -p polint --test public_surface_leak --all-features --locked` —
  PASS.
- `cargo test -p polint --lib eval::determinism_gate --all-features --locked`
  — PASS.
- Golden diagnostic sets — byte-identical on the initial run and required
  retry.
- Golden cost sidecar — the known `examples/go-sensitive-writes/json` timing
  sensitivity remained cost-red: 674 ms versus the 358 ms baseline (+88.3%)
  initially and 574 ms (+60.3%) on the Q6.2 retry. Peak RSS stayed effectively
  flat. The cost baseline was not regenerated.

# T-INTERN-B REST ANALYSIS MID LAND

## Result

- Evidence, abstract-domain, data-flow, and entrypoint fact identities now retain
  `StableKeyId` instead of owned stable-key strings.
- Producers intern identities into the analysis database interner. Stores and
  downstream call sites keep IDs internally and resolve text only for stable
  ordering, digests, diagnostics, metadata, and debug or rendered output.
- No dual string/ID identity fields or compatibility shims were added.
- `FactMeta`, stable-key ownership metadata, module-graph identities, and other
  remaining analysis families were not migrated.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features -- -D warnings` — PASS
- `cargo test -p polint --test public_surface_leak --all-features --locked` —
  PASS (8 tests)
- `cargo test -p polint --lib eval::determinism_gate --all-features --locked` —
  PASS (14 tests)
- Golden diagnostic sets — byte-identical on both runs.
- Golden cost sidecar — the known `examples/go-sensitive-writes/json` timing
  sensitivity remained cost-red after rebase: 1263 ms versus the 358 ms
  baseline (+252.8%) on the first run and 1741 ms (+386.3%) on the required
  retry. Peak RSS stayed effectively flat. Per DECISION Q6.2,
  diagnostics-identical after the retry is PASS; cost baselines were not
  regenerated.

## Landing

- Rebased onto integration commit `e34e111c`, which includes module-graph
  interning commit `753b0ca3`.
- `rg -c 'stable_key: String' crates/polint/src` totals 81 declarations on the
  rebased branch, down from 113 on the integration tip.

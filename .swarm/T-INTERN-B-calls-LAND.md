# T-INTERN-B CALLS LAND

## Result

- Call-site, call-target, and unresolved-call facts now retain `StableKeyId`
  instead of owned stable-key strings.
- Call extraction and target derivation intern identities into the
  `AnalysisDb`-scoped interner.
- Normalization, validation, metadata, digests, debug output, solver inputs,
  and downstream consumers resolve IDs to text wherever stable textual
  ordering or output is required.
- CFG facts remain `StableKeyId` as landed by the preceding CFG slice. No
  `FactMeta`, MIR, or broad remaining-family migration was included.
- No dual string/ID fields or compatibility shims were added.

## Structural delta

- On the rebased integration tip, `stable_key: String` declarations under
  `crates/polint/src` decreased from 181 to 178.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
- Golden diagnostic sets — PASS (byte-identical on both stock-harness runs).
- Golden cost sidecar — the known `examples/go-sensitive-writes/json`
  sensitivity from the MIR landing remained cost-red: 515 ms versus the
  358 ms baseline (+43.9%) on the first post-rebase run and 632 ms (+76.5%) on
  the required retry. Peak RSS was slightly below baseline on both runs.
  Per DECISION Q6.2, diagnostics-identical after the retry is PASS; cost
  baselines were not regenerated.

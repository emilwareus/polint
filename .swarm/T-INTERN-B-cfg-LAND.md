# T-INTERN-B CFG LAND

## Result

- CFG function, node, block, edge, reachability, dominator,
  post-dominator, control-dependence, and unsupported-control-flow facts now
  retain `StableKeyId` instead of owned stable-key strings.
- CFG-owned builder identities use `StableKeyId`; lowering interns identities
  into the `AnalysisDb`-scoped interner.
- Normalization, graph indexing, validation, metadata, digests, debug output,
  and downstream consumers resolve IDs to text wherever stable textual ordering
  or output is required.
- Call facts, `FactMeta`, MIR, and other fact families were not migrated.
- No dual string/ID fields or compatibility shims were added.

## Structural delta

- `stable_key: String` declarations under `crates/polint/src` decreased from
  192 to 181.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
- Golden diagnostic sets — PASS (byte-identical on the successful full retry).
- Golden cost sidecar — the known `examples/go-sensitive-writes/json`
  sensitivity reproduced on the first stock-harness run: 701 ms versus the
  358 ms baseline (+95.8%), while RSS remained effectively flat. Per DECISION
  Q6.2, the harness was retried once. The retry passed at 400 ms (+11.7%,
  within the harness's absolute-noise allowance), with byte-identical
  diagnostics. Cost baselines were not regenerated.


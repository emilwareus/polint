# T-INTERN-B REST FRONTENDS LAND

## Result

- Go semantic facts now retain `StableKeyId` for stable keys and dynamic-dispatch
  callsite identities. Raw sidecar protocol frames remain textual only at the
  deserialization boundary and are interned during lowering.
- TypeScript/JavaScript inventory, scope, binding, and object-model facts now
  retain `StableKeyId` for stable keys and related cross-fact identities.
- Extraction interns identities into the `AnalysisDb`-scoped interner.
  Normalization, validation, indexes, digests, debug output, and downstream
  consumers resolve IDs only where stable text is required.
- No dual string/ID identity fields or compatibility shims were added.
- `FactMeta`, stable-key ownership, semantic graph facts, module graph facts,
  MIR, calls, CFG, and solver densification remain outside this slice.

## Structural delta

- On the calls-landed integration base, exact `stable_key: String` occurrences
  under `crates/polint/src` decreased from 178 to 151.
- After rebasing over the semantic-identity slice, the combined remaining count
  is 121.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
- Golden diagnostics — PASS (all expected diagnostic sets remained identical
  on both stock-harness runs).
- Golden cost sidecar — the known `examples/go-sensitive-writes/json`
  sensitivity remained cost-red after rebase: 493 ms versus the 358 ms baseline
  (+37.7%) on the first run and 441 ms (+23.2%) on the required retry. Peak RSS
  stayed close to baseline on both runs. Per DECISION Q6.2,
  diagnostics-identical after the retry is PASS; cost baselines were not
  regenerated.

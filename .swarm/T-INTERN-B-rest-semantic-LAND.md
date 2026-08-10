# T-INTERN-B REST SEMANTIC LAND

## Result

- Semantic symbol-graph and semantic-graph fact identities now retain
  `StableKeyId` instead of owned stable-key strings.
- Builders, stores, validation, debug output, digests, solver inputs, and
  required downstream consumers resolve IDs only where stable text is needed.
- No dual string/ID identity fields or compatibility shims were added.
- `FactMeta`, stable-key ownership, frontend facts, module graph facts, and
  solver densification remain outside this slice.

## Structural delta

- On the calls-landed integration base, exact `stable_key: String` occurrences
  under `crates/polint/src` decreased from 178 to 148.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
- Golden diagnostics and cost are verified on the combined semantic/frontend
  integration, avoiding duplicate cost-sensitive runs.

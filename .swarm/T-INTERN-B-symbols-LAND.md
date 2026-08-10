# T-INTERN-B symbols LAND

## Result

- `SymbolFact`, `DefinitionFact`, and `ReferenceFact` now retain
  `StableKeyId` values instead of owned stable-key strings.
- Symbol graph construction interns each identity once into the
  `AnalysisDb`-scoped interner.
- Metadata, cache payloads, diagnostics, SDK accessors, comparisons, and
  digests resolve IDs to stable-key text at their boundaries.
- The cache payload preserves its existing text schema and byte-stable
  ordering; raw interner allocation order is never serialized or compared.
- No MIR, call, CFG, solver, or `FactMeta` identity fields were migrated.

## Structural delta

- `stable_key: String` declarations under `crates/polint/src` decreased from
  208 to 202.
- The three migrated fact fields were deleted as strings; there are no dual
  string/ID fields or compatibility shims.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS (8 tests)
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
  (12 tests)
- `cargo test -p polint --test golden --locked` — PASS (8 tests, first run;
  78.24 s wall clock)
- External symbol/reference SDK rule tests — PASS
- Kernel metadata public-behavior test — PASS

Golden diagnostics remained byte-identical. The golden timing was effectively
flat against T-INTERN-A's recorded 78.27 s, so the retry allowance was not
used.

# T-INTERN-A LAND — stable-key interner foundation

## Result

- Added the `AnalysisDb`-scoped `StableKeyInterner` and compact
  `StableKeyId(u32)` identity. No process-global interner exists.
- Stable-key construction now interns once and returns `StableKeyId`.
- Current fact families retain their existing `stable_key: String` fields.
  Construction sites resolve IDs when writing those fields, without adding
  parallel ID fields.
- Database clones preserve existing ID assignments while detaching future
  interner mutations.
- Text remains the comparison, serialization, diagnostic, and digest boundary;
  numeric interner IDs are not used as ordering keys.

## Regression

- Interner tests cover repeated-key deduplication, exact text resolution, and
  detached clone behavior.
- Determinism and golden gates prove stable-key output and diagnostic sets
  remain byte-identical.
- No fact-family migration, solver densification, compatibility shim, or
  golden baseline regeneration was introduced.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS (8 tests)
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS (12 tests)
- `cargo test -p polint --test golden --locked` — PASS (8 tests, first run;
  78.27 s)

The golden diagnostic sets were unchanged. The timing budget passed on the
first run, so the timing-only retry allowance was not used.

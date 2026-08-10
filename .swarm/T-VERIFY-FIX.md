# T-VERIFY fix

**Result:** PASS  
**Branch:** `static-analysis-architecture-review`  
**Binding:** `.swarm/DECISION-2026-08-10-PRE-SHIP.md`

## Changes

- Made a fresh solver store report `BudgetStatus::NotRun`; replacing it with a
  completed solver output still reports that output's real budget status.
- Deleted 21 Jelly unit tests and their helper functions that asserted precision
  from the removed TS value-flow, object-model, recognizer, and token-bank paths.
  The retained Jelly tests cover adapter behavior, oracle parsing, deterministic
  normalization, TS inventory spans, and controlled handling of deeply nested input.
- Updated the abstract-domain fixture for the sole `IdeDomainSolver` path by
  removing per-function loop-widening `Top` expectations. Present, unknown,
  unsupported, budget-exceeded, index, and deterministic output remain covered.
- Excluded internal `docs/architecture-review/` design records from the test that
  scans supported public documentation for semantic-MIR leaks.
- No deleted analysis pipeline, compatibility shim, token bank, or golden
  diagnostic was restored or regenerated.

## Verification

| Gate | Result |
|---|---|
| `cargo test --workspace --all-features --locked` | PASS |
| `cargo test -p polint --test public_surface_leak --locked` | PASS |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked` | PASS |
| `cargo deny check` | PASS |
| `cargo test -p polint --test golden --locked` | PASS on the required single timing retry |

The first golden run had byte-identical diagnostics and only exceeded the
`examples/go-sensitive-writes/json` timing budget: 437 ms measured versus the
358 ms baseline (+22.1%). The required single retry passed in 48.90 seconds for
the complete golden test binary. Cost baselines were not regenerated.

The installed `cargo-deny` accepts `cargo deny check`, which is also the existing
Makefile target. The pre-ship decision's `--all-features` spelling is not a valid
local `cargo-deny` CLI option and was not used for G9.

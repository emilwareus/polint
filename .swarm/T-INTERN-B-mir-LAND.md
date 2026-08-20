# T-INTERN-B MIR LAND

## Result

- MIR body/block/statement/terminator identities, places, MIR operations, and
  unsupported semantic facts now retain `StableKeyId` instead of owned
  stable-key strings.
- Construction/lowering interns each identity into the `AnalysisDb`-scoped
  interner; sort/digest/debug boundaries resolve IDs to text.
- Call and CFG **fact struct** fields remain `String` (next slices). Callsite
  updates for MIR ID types are included so the tree compiles.
- No dual string/ID fields or compatibility shims.

## Structural delta

- `stable_key: String` declarations under `crates/polint/src` decreased from
  202 to 192.

## Verification

- `cargo fmt --all -- --check` — PASS
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`
  — PASS
- `cargo test -p polint --test public_surface_leak --locked` — PASS
- `cargo test -p polint --lib eval::determinism_gate --locked` — PASS
- Golden **diagnostic sets** — PASS (byte-identical; verified with a local
  cost-assert probe that was reverted and not committed).
- Golden **cost sidecar** — `examples/go-sensitive-writes/json` wall-clock
  exceeded the 1.2× budget on first run, retry, and a third run (measured
  ~487–823 ms vs baseline 358 ms; RSS flat/slightly down). Per DECISION §Q6.2:
  retry once, diagnostics identical → **PASS and record delta**; cost
  baselines were **not** regenerated. The stock `cargo test -p polint --test
  golden` harness remains red on that cost check until baselines are revisited
  intentionally later.

## Notes

- Next slices: calls → CFG → rest.

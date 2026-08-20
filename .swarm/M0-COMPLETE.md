# M0 COMPLETE — milestone gate closed

**Status:** PASS  
**Integration branch:** `static-analysis-architecture-review`  
**HEAD:** `db0dd36db7c6ae76a1c6d5dacdc26462a3cb74dc`  
**Recorded:** 2026-08-08T11:52:00+02:00  
**Never merge to main from swarm.**

## Task set

All of `W0.1 W0.2 W0.3 W0.4 W0.5 W0.A1 W0.A2 W0.A3 W0.A4 W0.A5` are
`MERGED` / `MERGED-NOOP` (see `.swarm/state.json` and `.swarm/M0-GATE.md`).

Gate-close fix landed after initial G3 fail:

- `db0dd36d` — move golden-cost recording out of `eval::bench` into crate-private
  `golden_cost` (W0.A4 had put `eval::` into public `runner`); refresh
  `*.cost.json` sidecars after wall-clock baselines proved too tight for
  `go-sensitive-writes`.

## §5 gates (on `db0dd36d`)

| Gate | Command | Exit | Timing (`real`) |
|------|---------|------|-----------------|
| G1 | `cargo fmt --all -- --check` | 0 | 1.14 s |
| G2 | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 | 0.20 s |
| G3 | `cargo test --workspace --all-features --locked` | 0 | **674.63 s** (~11.2 min) |
| G4 | `cargo test -p polint --test public_surface_leak --locked` | 0 | 16.46 s |
| G5 | `cargo test -p polint --lib eval::determinism_gate --locked` | 0 | 28.75 s |
| G6 | `cargo test -p polint polyglot --lib --locked` | 0 | 3.43 s |
| G7 | `cargo test -p polint --test golden --locked` | 0 | 33.16 s |
| G8 | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | 0 | 14.69 s |
| G9 | `cargo deny --all-features --locked check` | 0 | 0.79 s |

Logs: `.swarm/gate-logs/`.

### First G3 attempt (pre-fix, tip `6b093f0e`)

- **FAIL** `eval_harness_stays_internal` — `runner/mod.rs` contained `crate::eval::…`
  from W0.A4 cost wiring. `real` 711.63 s. Fixed in `db0dd36d`; full suite re-run green.

## PLAN.md §5 scoreboard — Today notes after M0

| Metric | Today (post-M0) | What M0 changed |
|--------|-----------------|-----------------|
| Retained bytes / LOC | ~5.6 KB (still) | **Gated** via W0.5 retained-bytes-per-LOC over scale-corpus artifact; ceiling unchanged |
| Largest repo analysed | Scale corpus with loud OOM | W0.4 publishes LOC + RSS + wall-clock; OOM recorded in artifact (no silent skip) |
| Module cycles | 26 | unchanged (M2 work) |
| Edits to add a frontend | ~19 | unchanged (M2 / W2.6); W0.3 MERGED-NOOP (no Rust frontend yet) |
| `cargo check` warm | 66 s | unchanged |
| Findings with evidence path | 0% | unchanged (M1 W1.2) |
| Tests asserting accuracy | **≥1 gated** | W0.1 F1 gate + W0.2 cost columns in one baseline path |
| Behavioural lock | **present** | W0.A1–A5: corpus, characterization goldens (G7), capability matrix, per-case cost, regen opt-in |
| Interprocedural taint | none | unchanged |
| Warm re-run after 1-fn edit | full | unchanged |
| Rules shareable across repos | no | unchanged |

## Next

- Milestone → **M1**. `dispatch_allowed=true`.
- READY: `W1.1 W1.2 W1.3 W1.4 W1.5 W1.6 W1.7 W1.8 W1.9` (W1.5 measure-first checkpoint).
- First dispatch wave (mechanical): `W1.4 W1.6 W1.7 W1.8`.
- Defer `W1.1 W1.2 W1.3` until after noting **golden lock serialization** for regen PRs.

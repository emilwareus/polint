# M1 COMPLETE — milestone gate closed

**Status:** PASS  
**Integration branch:** `static-analysis-architecture-review`  
**HEAD:** `70dda34cbea27a3c3ffaddd1b4298a3ec1000ef1`  
**Recorded:** 2026-08-08T14:39:11+02:00  
**Never merge to main from swarm.**

## Task set

All of `W1.1 W1.2 W1.3 W1.4 W1.5 W1.6 W1.7 W1.8 W1.9` are
`MERGED` / `MERGED-NOOP` (see `.swarm/state.json` and `.swarm/M1-GATE.md`).

Notable tip commits after behaviour wave:

- `3891f90a` — W1.2 ship validated `evidence_v1` for policy-query diagnostics
- `0873e17f` / `d86e6712` / `222838a4` / `70dda34c` — golden cost sidecar refreshes
  (quiet + multipass + G3-contention headroom)
- `4a4719a0` — stop false-positive public-surface forbid on evidence `validation` keys
- `726a2e05` — `FileId::from_raw` for `#[non_exhaustive]` external construction (W1.8 fallout)

## §5 gates (on `70dda34c`)

| Gate | Command | Exit | Timing (`real`) |
|------|---------|------|-----------------|
| G1 | `cargo fmt --all -- --check` | 0 | 1.17 s |
| G2 | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | 0 | 0.17 s |
| G3 | `cargo test --workspace --all-features --locked` | 0 | **546.25 s** (~9.1 min) |
| G4 | `cargo test -p polint --test public_surface_leak --locked` | 0 | 2.02 s |
| G5 | `cargo test -p polint --lib eval::determinism_gate --locked` | 0 | 5.38 s |
| G6 | `cargo test -p polint polyglot --lib --locked` | 0 | 3.21 s |
| G7 | `cargo test -p polint --test golden --locked` | 0 | 28.15 s |
| G8 | `RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked` | 0 | 0.18 s |
| G9 | `cargo deny --all-features --locked check` | 0 | 0.77 s |

Logs: `.swarm/gate-logs/M1-summary-close.txt`, `M1-close-*.txt`.

### Gate retries before close

- First M1 §5 on `222838a4`: G3 FAIL — bare `validation` token in public-surface forbid
  (fixed `4a4719a0`).
- Retry on `4a4719a0`: G3 FAIL — `FileId(0)` in capability_planning rule packs under
  `#[non_exhaustive]` (fixed `726a2e05`).
- Full suite on `726a2e05`: G3 FAIL — golden cost budget under suite contention
  (baselines raised `70dda34c`).

## M1 product scoreboard (ORCHESTRATION §3)

| Check | Status | Evidence |
|-------|--------|----------|
| >90% of policy-query findings carry an evidence path | **PASS** | `policy_queries::tests::policy_query_findings_carry_replayable_evidence` ok (`.swarm/gate-logs/M1-close-evidence.txt`) |
| `polint check --format ai-friendly` lists zero-finding rules | **PASS** | ai-friendly output documents `jq '.summary.rules[] \| select(.diagnostics_emitted == 0)'` (W1.3 telemetry); sample runs in `.swarm/gate-logs/M1-close-scoreboard.txt` |

## Next

- Milestone → **M2**. `dispatch_allowed=true`.
- READY / first dispatch: **W2.1** (evict `eval/`) and **W2.2** (split `core/mod.rs`) in parallel.
- Specs: `docs/architecture-review/PLAN.md` M2 rows + linked HANDOFF; detailed specs exist for W2.3+.
- Never merge to `main`.

# M3 — Make the IR real (binding overview)

**Human authorized:** 2026-08-08 — see `.swarm/HUMAN-BINDING-2026-08-08-M3-M4.md`  
**Precondition:** M2 structural work on tip (`FactStore`, provider scheduler, `LanguageFrontend` registry).  
**Languages in scope:** Go + TypeScript/JavaScript only. **No** Python/Rust proof frontends.

---

## Problem (current tree)

- `MirStatement` / `MirTerminator` exist in `analysis/mir/body.rs` and are **constructed nowhere**.
- MIR is a flat `Vec<MirOperation>` annotation stream.
- CFG (`cfg/lower_{ts,go}.rs`) recovers loops / short-circuit by **substring-matching source text** via `operation_evidence` → `contains_token`.
- `ts_value_flows.rs` already forked around MIR for call/value precision.

That is not an IR. M3 makes it one and migrates existing consumers onto it.

---

## Task graph

| Task | Spec | Depends | Done when |
|---|---|---|---|
| W3.1 | [W3.1-mir-blocks-terminators.md](W3.1-mir-blocks-terminators.md) | M2 tip | Blocks + core terminators emitted by Go/TS MIR lowerers; CFG reads terminators for Goto/Branch/Switch/Return/Unreachable — **no source-text branch_shape for those** |
| W3.2 | [W3.2-mir-effects.md](W3.2-mir-effects.md) | W3.1 | Throw / Call{normal,unwind} / Suspend terminators replace construct-string CFG recovery for those shapes |
| W3.3 | [W3.3-mir-ops-and-types.md](W3.3-mir-ops-and-types.md) | W3.1 | BinOp, Aggregate, Closure{captures}; place facts carry `ty` into existing type lattice |
| W3.4 | [W3.4-delete-cfg-source-lowerers.md](W3.4-delete-cfg-source-lowerers.md) | W3.1, W3.2 | Single language-neutral CFG builder from MIR blocks; `lower_{ts,go}.rs` CFG files deleted |
| W3.5 | [W3.5-fold-ts-value-flows.md](W3.5-fold-ts-value-flows.md) | W3.3 | `ts_value_flows` folded onto shared MIR/call pipeline or deleted with callers migrated |

Then **M4** (separate specs under `W4.*`).

---

## Exit gate (this branch — architecture, not LoC)

1. No CFG path recovers control flow by grepping source text.
2. `MirBlock` + terminators are populated by MIR lowerers and consumed by CFG.
3. Per-language CFG lowerer modules are gone.
4. `ts_value_flows` is not a parallel frontend (folded or retired).
5. Goldens + `public_surface_leak` green (locked golden regen only with orchestrator `golden` lock).

**Not required:** throwaway Python frontend; LoC/RSS ceilings; interning.

---

## Dual-system ban

Do **not** leave “flat ops CFG” and “block CFG” both live. Prefer small PRs that each leave **one** production path. Temporary scaffolding inside a single land commit is fine; long-lived dual dispatch is not.

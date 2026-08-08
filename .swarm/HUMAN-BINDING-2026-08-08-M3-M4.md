# Human binding — finish M3 + M4 on this branch

**Authorized:** 2026-08-08  
**Branch:** `static-analysis-architecture-review`  
**Never merge to main from swarm** until human ships.

## Scope

- **Do M3 (real IR) and M4 (IFDS / principled engine) on this PR/branch.**
- **Migrate existing Go + TypeScript/JavaScript analysis only.** No new languages. No Rust frontend. No layering dogfood (W0.3 stays deferred follow-up).
- **No new product features, no placeholders, no “reserved for later” hooks** that are not required to migrate what already exists.
- **No behavior regressions** on existing goldens / public SDK contracts unless a golden lock is explicitly granted for a justified MIR/CFG migration step.
- **Interning (W2.3 StableKeyId):** follow-up only — do not block M3/M4.
- **LoC / retained-bytes-per-LOC / cycle-count gates:** ignored as ship criteria. Architecture correctness and non-regression matter.

## Exit meaning (this branch)

M3 + M4 done means:

1. MIR is a real IR used by CFG (blocks + terminators constructed and consumed).
2. CFG does not recover control flow by grepping source text.
3. Per-language CFG substring lowerers are deleted; shared CFG reads MIR.
4. `ts_value_flows` is folded onto the shared pipeline or retired with callers migrated.
5. IFDS/IDE (or equivalent principled interprocedural engine on the repaired ICFG) replaces the recognizer-bank / unrealizable-path path for the existing security/taint surfaces that already claim that capability.
6. Existing example goldens and public surface leak gate stay green (or locked regenerations only when MIR/CFG output shape must change).

## Explicit non-goals on this branch

- Python / Rust / SCIP / LSIF language adapters
- W0.3 self-layering dogfood
- StableKeyId interning completion
- M5 compound (crate split as product milestone, shareable packs, framework-models productization) unless a change is strictly required to land M3/M4 without dual systems

## Orchestration override

`ORCHESTRATION.md` previously forbade M3/M4 task creation until specs existed. Human overrides: **write binding specs, then dispatch.** Specs live under `docs/architecture-review/specs/W3.*` and `W4.*`.

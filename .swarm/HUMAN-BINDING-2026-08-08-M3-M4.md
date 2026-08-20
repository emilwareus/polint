# Human binding — finish M3 + M4 on this branch

**Authorized:** 2026-08-08 (amended 2026-08-09)  
**Branch:** `static-analysis-architecture-review`  
**Never merge to main from swarm.** **Ship only after the migration is complete.**

## Scope

- **Do M3 (real IR) and M4 (IFDS / principled engine) on this PR/branch.**
- **Migrate existing Go + TypeScript/JavaScript analysis only.** No new languages.
- **No new product features, no placeholders, no “reserved for later” hooks** that are not required to migrate what already exists.
- **No dual paths. No internal backwards compatibility.** Complete migration only: delete old logic in the same change that lands the new path.
- **No behavior regressions** on existing goldens / public SDK contracts unless a golden lock is explicitly granted.
- **LoC / retained-bytes-per-LOC / cycle-count gates:** ignored as ship criteria.

## Exit meaning (this branch)

M3 + M4 done means:

1. MIR is a real IR used by CFG (blocks + terminators constructed and consumed).
2. CFG does not recover control flow by grepping source text.
3. Per-language CFG substring lowerers are deleted; shared CFG reads MIR.
4. `ts_value_flows` is folded onto the shared pipeline or retired with callers migrated.
5. IFDS/IDE replaces the recognizer-bank / unrealizable-path path for existing security/taint surfaces.
6. Existing example goldens and public surface leak gate stay green (or locked regenerations only when MIR/CFG output shape must change).

## Explicit non-goals (do not plan, do not mention in this PR)

- **W0.3 Rust self-layering dogfood** — net-new; not this PR
- **Cross-language taint (e.g. TS calling Go over HTTP)** — net-new; not this PR
- Python / SCIP / LSIF adapters, shareable rule packs, framework-models productization

## Before ship (migration continue — 2026-08-09)

- **StableKeyId interning — CONTINUE** (replan; prior RTA prototype was wrong proof). See `docs/architecture-review/INTERNING-CONTINUE.md`.
- **M5 before merge:** see `docs/architecture-review/M5-BEFORE-MERGE.md`.
  - **Start with two:** (1) interning implement, (2) W5.1 crate split — then discuss.
  - **Then before merge:** W5.2 persistent store, W5.3 demand queries.
  - **Not this PR:** W5.4 shareable packs, W5.5 Python, W5.6 external indexes, W5.7 framework models.

## Orchestration override

`ORCHESTRATION.md` previously forbade M3/M4 task creation until specs existed. Human overrides: **write binding specs, then dispatch.** Specs live under `docs/architecture-review/specs/W3.*` and `W4.*`.

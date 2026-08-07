# Architecture Review — 2026-07-29

A deep review of polint's architecture against the goal of becoming the world's most capable static
analysis engine. Base commit `1263208a`, branch `static-analysis-architecture-review`.

**Start here: [00-TARGET-ARCHITECTURE.md](00-TARGET-ARCHITECTURE.md)** — the verdict, the seven
inversions, and the target crate graph.
**Then: [PLAN.md](PLAN.md)** — the executable sequence, milestone gates, and the scoreboard.

> ### ⚠️ Implementing agents: read [HANDOFF.md](HANDOFF.md) FIRST.
> It carries the **errata table** (these documents disagree with each other on several numbers;
> HANDOFF is authoritative), the hard rules, and the task tiering.
> Then read the binding spec for your item in [**specs/**](specs/README.md).
> **Orchestrators: [ORCHESTRATION.md](ORCHESTRATION.md).**
>
> **Precedence: spec > HANDOFF > PLAN > review documents.**

## The one-paragraph finding

polint is a remarkably well-engineered **pipeline** that has been designed, throughout, as if it were
a **platform** — and the platform wiring was never connected. The provider manifests, the query-key
algebra, the persistent store, the evidence model, the Andersen solver, the lattice kernel, the
promotion gates: all built, none load-bearing. Fixing this is rewiring, not rewriting.

## Documents

| # | Document | Core finding |
|---|---|---|
| **00** | [Target Architecture](00-TARGET-ARCHITECTURE.md) | Seven inversions; critical path is interning → provider trait → real IR → IFDS |
| **↳** | [**PLAN.md**](PLAN.md) | Six milestones with falsifiable exit gates; the v2.0 reordering call; Phase-65-derived working rules |
| **⚠** | [**HANDOFF.md**](HANDOFF.md) | **Errata (authoritative), hard rules, task tiering, the golden-harness spec.** Implementing agents start here |
| **🤖** | [**ORCHESTRATION.md**](ORCHESTRATION.md) | **Swarm runbook** — task DAG with parallel widths, locks, exact gate commands, orchestrator decision table, hold protocol |
| **↳** | [**specs/**](specs/README.md) | Eight binding implementation specs — design decisions, PR-sized steps, acceptance commands, anti-goals |
| 01 | [Layering and boundaries](01-layering-and-boundaries.md) | 26 of 27 modules in import cycles; 17 traits in 267k LOC; one 877-line pipeline function; 132-field god struct |
| 02 | [Rust code quality](02-rust-code-quality.md) | Discipline A-tier (clean clippy, forbidden unsafe, 12 CI jobs); data modelling D-tier (zero interner, 229 `stable_key: String`) |
| 03 | [Frontends, IR, language scaling](03-frontend-ir-and-language-scaling.md) | No adapter contract exists; the MIR is not an IR; the TS pipeline already forked around it |
| 04 | [Analysis core capabilities](04-analysis-core-capabilities.md) | Semgrep-tier, not CodeQL-tier: 18 unioned edge producers, no taint, no realizable-path discipline |
| 05 | [Incrementality and store](05-incrementality-and-store.md) | A memoization cache, not an incremental engine; the SQLite store holds exactly one table |
| 06 | [Performance and scale](06-performance-and-scale.md) | ~5.6 KB retained per LOC → OOM at 1M LOC on a 7 GB runner; TS corpus re-parsed up to 12× serially; never measured on a repository |
| 07 | [Extension surface](07-extension-surface.md) | Best rule front door in the category; no rule distribution at all; evidence built and then stripped |
| 08 | [Evaluation and correctness](08-evaluation-and-correctness.md) | No test anywhere asserts a precision, recall, or F1 value — the goal is unfalsifiable by its own CI |
| 09 | [Declared direction and gaps](09-declared-direction-and-gaps.md) | Research adoption audit; v2.0 gates; the Phase 65 post-mortem; no `ARCHITECTURE.md` exists |
| 10 | [SOTA landscape and the bar](10-sota-landscape-and-bar.md) | What "most capable" requires; three defensible wedges; what polint must not try to be |

## Method and confidence

Docs 01–06, 08, 09 are grounded in direct code reading with `path:line` citations; treat their claims
as verifiable against the tree. Doc 06 was produced without building (greps and struct-derived
estimates only) and labels every number `[recorded]` or `[estimated]`. Doc 10 is from model knowledge
of the named systems' public documentation, not verified against live sources in this session —
architectural characterizations are high confidence, specific numbers are approximate and should be
re-verified before entering a plan.

## The three highest-leverage actions

1. **Wire the accuracy gate** (`eval/external/mod.rs:27-29` silently `return`s; the committed
   baseline is `null`). Three lines turn 29,344 LOC of harness into a circuit breaker.
2. **Intern the identity model.** ~66% of retained memory is redundant copies of the same string.
3. **Execute the provider DAG that is already declared** in `PROVIDER_MANIFESTS`, replacing the
   877-line hand-written pipeline.

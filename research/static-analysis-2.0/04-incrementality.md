# 04 — Incrementality

## Problem

Beyond the parser-layer cache, every run recomputes everything. The target:
warm runs proportional to the change — sub-second `polint review` on a PR,
editor-speed re-analysis eventually. Incrementality only pays if queries are
*firewalled* so an edit doesn't invalidate whole-program layers; that
firewall is the summary store (doc 03). Engine choice matters less than
architecture — the rust-analyzer lesson.

## Current state in polint

- Layer cache for Go/TS syntax facts (content + config + schema keyed);
  everything downstream recomputes each run.
- `analysis_kernel/incremental/` exists as a placeholder.
- The internal incremental-query-engine research track (2026-05-26) already
  chose "hybrid deterministic provider DAG + typed fact layers", explicitly
  deferring engine adoption — consistent with this doc.
- `polint review` diff-gating reduces the requirement: we need O(diff)
  fact derivation, not editor-keystroke latency, first.

## What the research says

- **Salsa / red-green** (rust-analyzer, rustc:
  https://salsa-rs.github.io/salsa/reference/algorithm.html,
  https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html):
  memoized query graph; on edit, re-validate by checking whether inputs'
  results changed; **durability** tags let whole subgraphs (deps) validate in
  O(1). Native Rust crate, production-proven. Direct mapping: deps=high
  durability, workspace=low.
- **Glean stacked DBs** (https://glean.software/blog/incremental/):
  incremental fact DB hides changed units of a base DB — O(changes)
  indexing, <10% query overhead. The persistence-side complement to Salsa.
- **LADDDER** (PLDI 2021,
  https://www.pl.informatik.uni-mainz.de/files/2021/04/inca-whole-program.pdf):
  incremental whole-program points-to/constant-prop in **milliseconds per
  edit** via differential dataflow + lattice aggregation. Proof of
  possibility — but resident incremental state carries a memory tax
  ("orders of magnitude" in places), the wrong first move for a tool that
  OOMs. Watchlist, not roadmap.
- **Incrementalizing production CodeQL** (ESEC/FSE 2023,
  https://arxiv.org/abs/2308.09660): update times proportional to change
  size are achievable on a relational evaluator, but context-sensitive query
  design can destroy reuse — a warning for how we write derived-fact
  queries.
- **Demanded abstract interpretation** (PLDI 2021,
  https://dl.acm.org/doi/10.1145/3453483.3454044; TOPLAS 2024 extension):
  incremental + demand-driven unified for arbitrary abstract interpreters
  with from-scratch consistency; 95% of queries <1.2s at interactive edit
  rates. The theory backstop for correctness claims.

## Direction for polint

1. Land the summary store first (doc 03) — it is the invalidation unit.
2. Adopt Salsa (or a Salsa-shaped internal layer if the dependency is
   unwanted) over: file → lowered MIR digest → function summary → derived
   facts → rule outputs. Durability: deps > workspace config > source.
3. `polint review` becomes: diff → changed functions → invalidate summaries
   via Merkle keys → recompute frontier → re-derive only affected facts.
4. Determinism gates must survive: memoization cannot change byte-stable
   outputs (existing determinism fixtures become the regression net).
5. Re-evaluate differential dataflow only if editor-latency whole-program
   facts become a product requirement.

## References

Salsa book · rustc query system
(https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation-in-detail.html)
· Glean incrementality · LADDDER PLDI'21 · CodeQL incremental arXiv:2308.09660
· demanded AI PLDI'21/TOPLAS'24 · internal:
research/incremental-query-engine/FINAL-REPORT.md

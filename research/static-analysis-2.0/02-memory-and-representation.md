# 02 — Memory & Representation

## Problem

polint OOMs on large repos. Root causes: (a) all sources are read into
memory up front; (b) all facts/MIR/CFGs are materialized in `AnalysisDb` and
retained; (c) the points-to solver uses per-cell owned `BTreeSet`s with
hot-path clones; (d) the pipeline is eager within every enabled capability
slice. None of this is inherent — it is representation and lifetime
discipline.

## Current state in polint

- Eager source load: `crates/polint/src/fs/mod.rs:85-135` — parallel
  `read_to_string` of every discovered file into `AnalysisDb` before any
  analysis runs.
- Parser trees (oxc / tree-sitter) are already dropped after fact
  extraction; MIR + CFG + all facts are retained for the whole run.
- Points-to sets: `sets: Vec<BTreeSet<TokenId>>`
  (`analysis/calls/js_points_to/solver.rs:248`); whole-set clone at
  `solver.rs:422`, successor-list clone at `solver.rs:458`.
- Budgets as blunt degradation: token cap (64/cell) and step cap (2M)
  silently drop edges under pressure.
- Fixed costs: Jelly micro ≈1.3–1.7 s per hello-world-sized fixture vs
  ≈0.2 s for Go fixtures — unprofiled.

## What the research says

- **Drop-AST / stub discipline**: rust-analyzer keeps only item-level stubs
  and re-parses on demand; oxc's arena (bumpalo) parse-then-drop is designed
  for this (https://oxc.rs/docs/learn/performance). Peak RSS becomes
  O(concurrency × largest file) instead of O(all files).
- **Hash-consed points-to sets**: Barbar & Sui, SAS 2021
  (https://yuleisui.github.io/publications/sas21.pdf) — deduplicate identical
  points-to sets, memoize unions: **1.85× average speedup (up to 3.21×) for
  Andersen's** in SVF plus large memory cuts. Follow-ups: "The ART of
  Sharing" (arXiv:2409.09062), multi-level dedup (arXiv:2604.10445).
- **Sparse bitsets / roaring**: LLVM `SparseBitVector`, SVF cores; ID
  renumbering for clustering matters as much as the container
  (arXiv:1108.2683). Rust: `roaring-rs`, `fixedbitset`, `hibitset`.
- **Indirection-bounded propagation**: ECOOP 2024 (Jelly group,
  https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ECOOP.2024.10)
  — bound pointer/higher-order indirection depth during wave propagation
  (Pereira & Berlin, CGO 2009): **~2× speedup, precision unchanged, ~5%
  recall loss, tunable**. A principled scale knob to replace the token cap.
- **Out-of-core when needed**: Graspan (ASPLOS 2017,
  https://dl.acm.org/doi/10.1145/3093336.3037744) ran fully
  context-sensitive pointer + dataflow analysis on Linux-kernel-scale code
  on one desktop by spilling the constraint graph to disk. Proof that OOM is
  a representation problem.
- **BDDs (bddbddb, PLDI 2004)**: historical answer to context explosion;
  superseded for our shape by explicit shared sets + Datalog-style
  structures. Not recommended.
- **Interning**: a globally-locked interner cost oxc ~50% CPU in parallel
  parsing — use sharded/lock-free or inline (CompactStr) interners; u32
  spans/IDs everywhere.

## Direction for polint

1. **Per-file pipeline** replacing the up-front load: read → parse → lower →
   extract → drop source+AST, bounded concurrency ≈ cores; lazy re-read of
   source only for diagnostic snippet rendering.
2. **Kill solver clones first** (`solver.rs:422`, `:458`) — cheapest win —
   then move per-cell sets to sorted-vec or roaring over dense `TokenId`s
   with hash-consed sharing.
3. **`max_indirection_depth` budget** alongside (eventually replacing) the
   token cap; surfaced in benchmark telemetry (doc 01).
4. **Profile fixed per-run costs** (flamegraph one Jelly fixture) before any
   further algorithmic work.
5. Acceptance bar for all of the above: Jelly micro F1 unchanged; runtime
   and RSS down; real-app curves (doc 01) improve.

Explicitly out: GPU solvers (POPL 2021, arXiv:2006.01491 — Andersen's is
inherently poorly parallelizable in the worst case), distributed clusters,
full Datalog-engine rewrites (constant-factor memory tax vs bespoke interned
worklists).

## References

SAS'21 hash-consing · ECOOP'24 indirection bounding · Graspan ASPLOS'17 ·
oxc performance docs · rust-analyzer architecture
(https://rust-analyzer.github.io/book/contributing/architecture.html) ·
bddbddb PLDI'04 · POPL'21 Andersen complexity arXiv:2006.01491

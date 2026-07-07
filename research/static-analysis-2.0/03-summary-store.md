# 03 — Compositional Summary Store (the keystone)

## Problem

The pipeline is whole-program: every run parses and solves the application
*and* its full dependency tree, resident in memory. Multi-M-LOC analysis is a
solved problem in industry, and every solution has the same shape:
**per-unit analysis + persistent summaries**. A summary is a compact transfer
function that lets the analyzer reason about a call without ever loading the
callee's body — that property changes the complexity class of a run.

## The two tiers (same artifact, different reasons to exist)

**Tier 1 — dependencies, keyed by (package, version).** Immutable, universal
(`lodash@4.17.21` is bit-identical everywhere), acyclic (app→dep never cycles
back, so summaries finalize bottom-up with no cross-boundary fixpoint), and
most of the code by volume (node_modules is typically 10–100× app code;
Stubbifier measured ~56% of dependency code unreachable —
https://arxiv.org/abs/2110.14162). Computed once — locally on first
encounter or fetched from a **remote registry** (build-cache analogy:
sccache/Bazel remote cache; Glean's stacked DBs). The remote registry is a
later product option, not the first implementation. The first implementation is
the embedded local semantic store in `research/local-semantic-store/`: local
and CI cache restore must prove the summary format, validation, invalidation,
and recompute-and-diff story before any networked registry exists. Where static
analysis can't summarize (heavily dynamic packages), LLM-synthesized summaries
validated against `.d.ts` fill the gap (doc 07).

**Tier 2 — application code, keyed by content hash at function/SCC
granularity.** The payoff is *incrementality*, not sharing: an edit
invalidates one function's summary plus its transitive summary-dependents;
`polint review` becomes O(diff). The wrinkle: application summaries
participate in fixpoints (mutual recursion, whole-program refinement), so
keys are Merkle-shaped — hash(body) + digests of callee summaries + schema/
config digests. This tier is also the **firewall Salsa needs** (doc 04).

## What a v1 summary must contain (for CG + DF facts)

- **Callable/export surface** — exported callables incl. re-exports (CG
  stitching alone).
- **Callback-invocation behavior** — which parameters get invoked, with what
  `this`/args (`app.get(path, handler)` invokes `handler(req,res)` — the
  express problem becomes a lookup).
- **Value flow** — param→return, param→field escapes, allocation/type shape
  of returns (receivers downstream).
- **Taint transfer** — param→return / param→sink flows so interprocedural
  taint composes (doc 08).
- **Honesty metadata** — precision tier, unknowns, provenance; summaries are
  approximations and must say so (extends the existing fact model).

At a call site crossing a summary boundary, the solver applies the summary as
pre-baked constraints instead of descending into callee MIR.

## Current state in polint (seeds)

- `analysis/summaries/` (~7.3k LOC): intraprocedural direct summaries + SCC
  fixpoint closure — computed fresh every run, never persisted.
- Cross-file return summaries (built for the express chain) — a primitive
  per-module summary already proven to move recall.
- `.polint/cache/derived/` — reserved directory, currently unused, exactly
  for this.
- Cache-key discipline exists (content + config + schema-version digests in
  the layer cache) — reuse it.
- effects-summaries research track (research/effects-summaries/) reached the
  same conclusion: "summaries are the scaling boundary."

## What the research says

- **Infer / biabduction** (Calcagno et al., POPL 2009; "Scaling Static
  Analyses at Facebook", CACM 2019,
  https://cacm.acm.org/magazines/2019/8/238344-scaling-static-analyses-at-facebook/fulltext):
  per-procedure bottom-up summaries → near-linear total cost, embarrassing
  parallelism, free incrementality; deployed diff-time at Meta scale. Pulse
  rebuilds it on under-approximate (Incorrectness) logic — every report is a
  true path, the right bias for review rules.
- **JAM — modular CG for Node.js** (ISSTA 2021,
  https://cs.au.dk/~amoeller/papers/jam/paper.pdf): per-npm-module CGs
  composed via main-module abstractions — the direct model for Tier 1.
- **Glean** (Meta, https://glean.software/docs/implementation/incrementality/):
  immutable deduplicated facts in RocksDB, incremental via stacked DBs
  labeled by units — O(changes) re-indexing, tens of billions of facts. The
  storage-shape reference.
- **Demanded summarization** (TOPLAS 2024,
  https://plv.colorado.edu/bec/papers/demanded-summarization-toplas24.pdf):
  compositional summaries + demand + from-scratch consistency — the
  theoretical blueprint for summaries under interactive edits.
- **Salsa durability** (doc 04) maps directly: deps = high durability,
  workspace = low.

## Direction for polint

1. Define the v1 summary schema (CG + callback + value-flow first; taint
   transfer next) with honesty metadata; version it in cache keys.
2. Persist existing direct/SCC summaries through the SQLite/rusqlite local
   semantic store, with content-addressed payloads where useful; measure
   warm-run wins before any remote story.
3. Summarize dependencies at the export-API boundary (JAM-style), keyed
   (package, version, schema, config-digest); stop parsing dep bodies on
   warm runs.
4. Parallelize: rayon over the SCC-condensed call DAG in reverse topological
   order.
5. Remote registry later only: signed entries or fetch-then-spot-verify (a
   poisoned summary is a supply-chain vector into analysis results). Preserve
   the seam now, but do not build the registry in the first implementation.

## References

Infer CACM'19 · JAM ISSTA'21 · Glean docs · Stubbifier arXiv:2110.14162 ·
demanded summarization TOPLAS'24 · Saturn (Xie & Aiken) as historical
precedent · RacerD OOPSLA'18 (deliberately-unsound compositional precision at
monorepo scale)

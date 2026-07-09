# 06 — Selective Precision & Demand-Driven Queries

## Problem

Uniform precision doesn't scale: context sensitivity everywhere explodes;
context sensitivity nowhere loses precision exactly at higher-order hot
spots (promise combinators, `forEach`/`map` dispatchers, wrapper utilities —
our documented FP/FN buckets). Likewise, eagerly solving *all* facts at
maximum depth wastes work on facts no rule reads. Two complementary answers:
spend precision only where it pays (selective context sensitivity), and
compute expensive facts only when asked (demand-driven queries).

## Current state in polint

- JS points-to is context-insensitive; precision issues are patched
  per-construct in recognizers/value-flow (array-positional bucket, wrapper
  smear).
- Capability gating is coarse-grained demand (a rule requesting `calls`
  triggers the whole CFG/call pipeline eagerly); there is no per-query
  demand within a capability. `analysis/demand/` (~2.2k LOC) exists as a
  seed.
- Rules are literal demand clients — the SDK shape (typed fact views pulled
  by rule signatures) is already demand-shaped at the surface.

## What the research says

**Selective context sensitivity**
- **Zipper** (Li, Tan, Møller, Smaragdakis, OOPSLA 2018,
  https://cs.au.dk/~amoeller/papers/zipper/paper.pdf): three flow patterns
  identify precision-critical methods (~38% of methods); keeps 98.8% of
  2obj precision at 3.4× average speedup.
- **Scaler** (FSE 2018): per-method context flavor under a global cost
  budget — "scalability-first" self-tuning.
- **Context tunneling** (OOPSLA 2018,
  https://dl.acm.org/doi/10.1145/3276510): spend the k-limit slots on the
  *useful* context elements instead of the most recent — mechanism is
  symbolic; the selection policy can be hand-written or learned (doc 07,
  Graphick).
- JS transfer: rank functions by fan-in of function-valued parameters;
  give only those 1-call-site sensitivity. Cheap prototype, targets our
  exact FP buckets.

**Demand-driven queries**
- **CFL-reachability lineage** (Reps POPL'95 →): points-to/data-flow as
  language reachability → answer per-query without whole-program solving.
- **Sridharan & Bodík** (PLDI 2006): client-driven refinement — start
  imprecise, refine only along paths the client can't discharge;
  orders-of-magnitude memory savings, ~1s/query, IDE-suitable.
- **Boomerang** (ECOOP 2016) / **SPDS** (POPL 2019,
  synchronized pushdown systems): demand-driven, flow/field/context-
  sensitive points-to + alias sets restricted to the query's slice;
  SparseBoomerang adds sparsification. Well-specified, Rust-implementable
  (worklist over weighted pushdown automata).
- Pairing with diff-gating: `polint review` only needs facts reachable from
  the diff — demand queries make that literal.

## Direction for polint

1. After the type tier (doc 05) shrinks the residue, add Zipper-style
   selection over the heap: precision-critical functions get 1-CFA/
   tunneled contexts; everything else stays insensitive.
2. Grow `analysis/demand/` into a real query layer: rules issue queries
   (points-to of X at S, taint from source-set to sink-set) answered
   SPDS-style over summaries (doc 03) instead of iterating eagerly
   materialized fact tables. SDK evolution: query-shaped fact views beside
   iterator-shaped ones.
3. Selection heuristics start hand-written (fan-in of callable params,
   higher-order fan-out); learned policies only later, trained on our own
   benchmark traces (doc 07 §5).

## References

Zipper OOPSLA'18 (+TOPLAS'20) · Scaler FSE'18 · tunneling OOPSLA'18 · Reps
POPL'95 · Sridharan/Bodík PLDI'06 · Boomerang ECOOP'16 · SPDS POPL'19 ·
SparseBoomerang: https://github.com/secure-software-engineering/SparseBoomerang

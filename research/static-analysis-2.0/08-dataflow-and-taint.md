# 08 — Data-Flow & Taint

## Problem

Data-flow is polint's weakest analysis relative to its value: interprocedural
taint propagates over **direct call edges only** — the refined callgraph
(points-to + RTA, months of work) never reaches it. And DF is solved eagerly
for everything when the capability is requested, which will not survive large
repos. Taint/source-sink rules are among the highest-value policies for the
agent-feedback product (the security templates polint ships as scaffolds all
want it).

## Current state in polint

- `analysis/data_flow/` (~5.3k LOC): intraprocedural reaching-definitions +
  interprocedural propagation via per-function summary edges
  (`summary_edges.rs`), but wired to direct edges (`direct_calls.rs`) only.
- `analysis/evidence/` (~4.3k LOC): path slicing for diagnostics — the
  user-facing form (matches the program-slicing-evidence research track:
  evidence bundles before raw graphs).
- CFG per function with dominators/control dependence exists (adequate;
  interprocedural CFG not needed — CG + intraproc CFG is the standard
  decomposition).
- Sources/sinks/sanitizers: rule-defined (correct per the data-flow research
  track — no universal defaults).

## What the research says

- **IFDS/IDE** (Reps, Horwitz, Sagiv POPL'95): interprocedural dataflow as
  graph reachability with summary edges — the standard framework (Heros,
  WALA, PhASAR). Our summary-edge design is already IFDS-shaped; the gap is
  the callgraph feeding it and demand-driven evaluation.
- **Compositional taint transfer in summaries**: Infer/Pulse-style
  under-approximate summaries (every reported path is real) fit review
  rules, where precision > soundness (CACM 2019; Incorrectness Logic POPL
  2020).
- **Demand-driven DF**: Boomerang/SPDS (doc 06) compute the slice a query
  needs; pairs with diff-gating — a PR review only needs flows intersecting
  the diff.
- **Spec inference for the long tail**: IRIS (ICLR 2025) shows LLM-inferred
  source/sink/sanitizer specs + symbolic taint doubles real-vuln recall vs
  CodeQL alone (27→55 on CWE-Bench-Java) — for polint this maps to
  LLM-assisted *summary* taint-transfer facts for dependencies (docs 03/07),
  while rule authors keep owning app-level specs.
- **Bimodal filtering**: Fluffy (ISSTA 2023, arXiv:2301.10545) — symbolic
  flows + NL-channel model ranking "unexpected" flows, F1 ≥ 0.85 at 250k-repo
  scale — a later-stage triage idea for noisy taint rules (doc 07 §4).

## Direction for polint

1. **Wire refined call edges into interprocedural taint** (small change,
   immediately rule-visible — the single best value-for-effort item in this
   track).
2. Add taint-transfer components to the v1 summary schema (doc 03) so flows
   compose across summarized boundaries, including dependencies.
3. Move DF to demand: rules pose (source-set, sink-set) queries; evaluation
   is SPDS-style over summaries + refined CG, sliced by the diff under
   `polint review`.
4. Keep evidence bundles as the output contract (thin slice / ranked path +
   provenance) — already the researched decision.
5. Precision bias: prefer under-approximate (Pulse-style) interprocedural
   steps for review rules; label tiers on flow facts like call edges.

## References

IFDS POPL'95 · PhASAR (https://phasar.org) · Infer CACM'19 · Incorrectness
Logic POPL'20 · IRIS ICLR'25 arXiv:2405.17238 · Fluffy arXiv:2301.10545 ·
internal: research/data-flow/, research/program-slicing-evidence/

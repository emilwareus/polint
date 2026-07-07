# Critical Review & Improvement Plan: CG/CFG/DF Accuracy and Scale

Date: 2026-07-07
Scope: full-repo review of the analysis engine + 2020–2026 literature survey
(algorithmic and ML), aimed at building CG/CFG/DF for very large applications
with high F1 and low latency/memory.

Baselines at time of writing:

| Suite | TP/FP/FN | Precision | Recall | F1 | Runtime | Peak RSS |
|---|---|---|---|---|---|---|
| Jelly JS/TS micro (76 fixtures) | 1217/37/262 | 97.05% | 82.3% | 89.06% | 80–130s | ~63 MB |
| Go x/tools RTA (5 fixtures) | 37/6/0 | 86.05% | 100% | 92.50% | ~1s | — |

Known failure mode on large real repos: OOM (eager whole-program pipeline,
all sources + facts resident; no cross-run incrementality beyond the parser
layer cache).

---

## TL;DR

The accuracy problem and the speed problem have different shapes than the
daily benchmark grind suggests.

- **Accuracy**: the JS callgraph is already at the static state of the art on
  the Jelly micro suite (F1 ~89%; Jelly itself reaches ~88% recall on real
  apps only *with* a dynamic pre-analysis; static-only ≈ 75.9%). Remaining F1
  headroom will not come from more recognizer iterations. It comes from
  (1) **type information we do not use today** (TS types — and the real
  enterprise targets are typed TS/Go, not untyped JS), (2) **verified ML
  type/shape inference** on the untyped residue, (3) **framework/dependency
  summaries**.
- **Scale**: the architecture is the problem — eager, whole-program,
  everything-resident, with budget caps (`max_tokens_per_cell: 64`,
  `max_steps: 2M`) that silently degrade recall on exactly the large repos
  that matter. The fix is the industry-proven path: **compositional bottom-up
  summaries with a disk-backed content-addressed store (Infer/Glean/JAM
  model), drop-AST representation discipline, then a Salsa query layer**.
- **ML**: its real role is not replacing the solver. Every production system
  (GitHub, Meta, Semgrep, Snyk) keeps the detection core symbolic and uses ML
  for (a) offline spec/summary mining, (b) candidate ranking with symbolic
  verification, (c) post-detection triage. Amazon CodeGuru — the one
  ML-detector product — was retired in 2025.

---

## 1. Evaluation of the parts

**Analysis kernel (capability-gated provider chain) — good bones, wrong
execution model.** Gating (rules declare fact views → only needed providers
run) matches Tricorder's tiering lesson and is right. But within an enabled
slice everything is eager and whole-program: all sources loaded up front
(`crates/polint/src/fs/mod.rs:85-135` parallel `read_to_string` of every
file), all facts materialized in `AnalysisDb`, providers sequential, only the
parser layer cached. Every run recomputes the world. This is the root of both
OOM and the latency ceiling.

**JS/TS callgraph stack (`analysis/calls` ~19k LOC + `js_points_to` +
value-flow) — at SOTA, but showing overfit and sprawl risk.** Three
overlapping resolvers (direct recognizers, TS value-flow, Andersen points-to
with lazy property cells) took F1 from 1% to 89% in ~50 iterations. Two
criticisms: (a) the recognizer layer increasingly reverse-engineers the
76-fixture Jelly oracle (generator sequencing, array pop-bucket semantics,
reachability-filtered scoring) — benchmark modeling, not language modeling;
(b) the honesty budgets are the mechanism by which accuracy will *silently
collapse at scale*: budget exhausts → edges go missing → F1-at-scale unknown
because nothing measures it.

**Go backend (tree-sitter + RTA) — fine, not the problem.** Go is typed;
RTA-grade resolution is near-free precision. Main gap: the suite is 5
fixtures.

**CFG (`analysis/cfg`) — real and adequate.** Per-function basic blocks from
MIR; dominators/post-dominators/control dependence as derived facts.
Intraprocedural CFGs are not a bottleneck anywhere in the literature; do not
over-invest here.

**Data-flow (`analysis/data_flow`) — the weakest analysis.** Interprocedural
taint flows through *direct call edges only* (`direct_calls.rs`); the refined
callgraph (points-to + RTA via `refined_calls/`) never reaches it. The best
asset does not feed the most valuable rule capability.

**Evaluation harness (`eval/`, adapter-based external suites) — a strategic
asset with a dangerously small corpus.** External-benchmark-first with pinned
oracles and revert-on-regression discipline is better methodology than most
academic tools. But the whole accuracy picture rests on 76 JS micro fixtures
+ 5 Go fixtures: no real-application benchmark, no F1-vs-size curve, no
memory/latency regression gates. The stated problem ("not accurate and fast
enough on large apps") is invisible in the harness.

**The 14-track research corpus — excellent, and independently confirmed.**
The internal FINAL-REPORTs (summaries as the scaling boundary, unknowns as
first-class facts, layered fact tiers, Salsa-style hybrid incrementality)
match the 2020–2026 literature almost point for point. Criticisms: the 22-PR
roadmap is waterfall-shaped and reality diverged (points-to built ahead of
the persistence/incrementality foundations); the public `ANALYSIS-ROADMAP.md`
still lists CG/CFG/DF as "Planned" while ~60k LOC exist internally — the
promotion pipeline is its own bottleneck.

---

## 2. Five hard critiques

1. **Optimizing the wrong variable for the market.** The Jelly benchmark is
   untyped-JS-heavy; enormous effort went into resolving dynamic patterns
   statically. Real users (enterprise repos, AI-agent workflows) are
   overwhelmingly **TypeScript and Go — typed**. A type-directed CG tier
   (XTA-grade, consuming tsc-provided types the way the Go sidecar consumes
   `go/packages`) resolves the majority of real-world call sites
   near-precisely at ~linear cost, reserving the Andersen heap for the
   untyped residue. TS type-directed CG construction is also under-published
   — the best novelty angle available. This is the single highest-leverage
   strategic correction.
2. **Whole-program-in-RAM is disqualifying for the goal, and no budget knob
   fixes it.** Multi-M-LOC analysis is a solved problem, and every solution
   has the same shape: per-unit analysis + persistent summaries — Infer
   (biabduction summaries, diff-time analysis at Meta scale), Glean (stacked
   RocksDB fact DBs, O(changes) incremental indexing), JAM (per-npm-package
   CG composition), CodeQL (disk-backed relational store). Graspan (ASPLOS'17)
   ran fully context-sensitive pointer analysis on Linux-kernel-scale code
   *out-of-core on one desktop*. The OOM is a representation-and-architecture
   decision, not a law of nature.
3. **Accuracy-at-scale is unmeasured and probably bad.** Budget exhaustion
   degrades recall invisibly; eager solving means big repos hit budgets
   first; the FN cost lands exactly where rules matter (framework dispatch
   chains). An F1-vs-size curve is required before believing any number.
4. **The recognizer/heap/value-flow triplication will not survive language
   #3.** Each new construct needs coordinated changes in three layers
   (frontend + MIR + value-flow). The unified solver/semantic-graph direction
   is right; the migration must actually retire recognizer sprawl rather than
   accrete beside it.
5. **Per-run fixed costs are high.** ~1.3–1.7s per hello-world-sized Jelly
   fixture vs ~0.2s for Go. Profile before algorithmic work; "fast on large
   apps" starts with "fast on small apps."

---

## 3. Scale plan (memory/speed)

Ordered; each step compounds the next. Matches both the internal research
conclusions and the strongest external evidence.

**Phase 0 — measure the actual problem (1–2 weeks).** Real-world benchmark
tier: 5–10 real Node/TS apps with NodeProf dynamic-trace oracles (Jelly
PLDI'24 methodology; artifact: zenodo.org/records/10930752) and 2–3 large Go
repos against `golang.org/x/tools` callgraph output. Record F1, wall-clock,
peak RSS, and budget-exhaustion events per repo size; add as regression
gates.

**Phase 1 — representation discipline (weeks, no semantic change).**
- **Drop-AST**: lower each file to MIR/facts, discard the oxc/tree-sitter
  tree immediately, re-read source lazily for diagnostics. Peak RSS:
  O(all files) → O(concurrency × largest file). rust-analyzer proves the
  discipline.
- Bounded parse concurrency + interned IDs/arenas (u32 spans, compact
  strings).
- **Hash-consed + roaring points-to sets** (Barbar & Sui, SAS 2021:
  1.85× speedup and large memory cuts in SVF for exactly this solver shape).
  Today: per-cell `BTreeSet<TokenId>` (`js_points_to/solver.rs:248`) with
  hot-path full-set clones (`solver.rs:422`, `:458`).
- **Indirection-bounded wave propagation** (ECOOP 2024, Jelly group): bound
  pointer-indirection depth in the solver — ~2× faster, precision unchanged,
  ~5% recall cost, tunable. Ship as the scale knob instead of the blunt
  token cap.

**Phase 2 — the keystone: compositional summaries + disk-backed store
(1–2 months).** Bottom-up per-function summaries over the SCC-condensed call
DAG (rayon-parallel, reverse topological order), persisted content-addressed.
Dependencies (`node_modules`, Go module cache) become **pre-summarizable
units keyed by (package, version)**: analyzed once ever, shared across runs —
eventually across users as a public summary registry (a product moat).
Existing cross-file return summaries + the effects-summaries research track
are the seed. Converts whole-program O(repo) into O(working set).
Stubbifier's finding (56% of dependency code unreachable) says most of the
store is shallow. `.polint/cache/derived/` is already reserved for this.

**Phase 3 — incrementality via Salsa (after Phase 2).** Salsa red-green over
the summary layer, durability tiers (dependencies = high, workspace = low).
The rust-analyzer-proven route to sub-second re-analysis; pairs with
diff-gated `polint review` (re-derive facts only for changed functions +
summary-dependents). Deliberately *not* differential dataflow/DDlog first:
LADDDER (PLDI 2021) proves ms-level whole-program incremental points-to, but
resident incremental state carries a memory tax that is wrong for a tool that
currently OOMs.

**Explicitly skip:** GPUs, distributed clusters (out-of-core single-machine
wins for this workload — Graspan), full Datalog-engine rewrites (constant-
factor memory tax vs a bespoke interned worklist).

---

## 4. Accuracy plan (F1), ranked by expected F1-per-effort

1. **Type-directed CG tier for TS** (and lean harder on Go types). Run a
   TypeScript type sidecar — mirror `go/semantic/client.rs` +
   `Command::new` invocation (`go/lifecycle.rs:615`); a Node script on the
   TS compiler API first, tsgo when its API allows. Resolve typed call sites
   XTA-style before the heap ever runs. Largest real-world recall jump
   available at near-linear cost.
2. **Feed the refined callgraph into data-flow.** Internal wiring fix with
   immediate rule-visible payoff.
3. **Verified neural type/callable-shape inference for the untyped residue**
   (JoernTI/CodeTIDAL5, ESORICS 2023: ~220M model, CPU/ONNX-servable,
   predictions accepted only after checking against class-hierarchy/export
   facts). The EMSE 2025 head-to-head is decisive: LLMs *lose* to Jelly/PyCG
   at direct CG extraction but *beat* static type inference — infer types,
   not edges; let the symbolic engine mint edges. Attacks the #1 documented
   FN root cause in JS CGs (dynamic property access / ungrounded receivers)
   with near-zero precision risk.
4. **Zipper-style selective context sensitivity** (OOPSLA 2018: ~1/3 of
   methods carry ~99% of the precision benefit) applied only to higher-order
   hot spots — promise combinators, `forEach`/`map` dispatchers, wrappers.
   Targets the array-positional and wrapper FP buckets without global cost.
5. **Learned callee ranker for still-unresolved sites, verify-then-accept**
   (GRAPHIA 2025: top-5 contains the true target 72% of the time). Start
   with a GBDT over cheap features (name similarity, module distance, arity,
   export structure); accept only candidates passing symbolic consistency
   checks. Train on the harness's own dynamic traces.
6. **Demand-driven data-flow queries (SPDS/Boomerang-style)** — rules are
   literal demand clients; compute taint slices per query instead of eagerly.
   Both an accuracy play (deeper per-query precision affordable) and a memory
   play.
7. **Opt-in approximate interpretation** (Jelly PLDI 2024: +12pp recall by
   approximately executing module-init code) — the biggest known recall
   lever, but it executes repo code; supply-chain risk ⇒ opt-in only.

---

## 5. ML and "reducing the big O" — honest framing

No ML system in the 2020–2026 literature reduces the asymptotic complexity of
the symbolic solve (Andersen's is provably near-sequential worst-case, POPL
2021), and every attempt to *replace* symbolic dataflow with GNNs/transformers
(ProGraML → DFA-Net) remains benchmark imitation with no soundness semantics
and poor out-of-distribution behavior.

What *does* reduce effective complexity, and where ML legitimately helps:

- **Modular summaries** turn O(whole program) into O(changed units) — and
  **LLM-synthesized summaries for the dependency long tail** (IRIS/AFD
  pattern: "does `router.use(f)` invoke `f`?", validated against `.d.ts`,
  cached per package@version, cents per package, amortized forever) mean
  `node_modules` bodies are never analyzed at all. The one place ML directly
  attacks both the recall gap (framework-mediated calls) and the memory
  ceiling.
- **Learned effort policies** (Graphick / context-tunneling, OOPSLA 2018/2020):
  cheap pre-analysis → learned per-function policy for "deserves context
  sensitivity / heap depth?" — attacks the exponential term by spending
  precision only where it pays. After the summary architecture lands; train
  on our own benchmark traces.
- **LLM triage on `polint review` findings** (Semgrep Autotriage ≈95% user
  agreement; LLift-style post-filtering): cheap, reversible, product-level
  precision win for the agent-feedback loop that is polint's thesis.

**Do not build:** ML callgraph *pruning* (CGPruner/AutoPruner — buys
precision we already have at ~97% by spending recall we cannot spare; MSR
2024 re-evaluation: ≈+25%P/−9%R; the field retreated to symbolic
OriginPruner), whole-repo LLM callgraph extraction (1000× cost, loses to
static tools), learned indexes for demand queries (no literature).

Caveat that applies to our harness too: nearly all CG-ML ground truth comes
from dynamic traces under test execution, which under-covers — models (and
benchmarks) partially learn test-coverage bias. Treat "FP" labels from
dynamic oracles with suspicion (already observed with unexercised true
edges).

---

## 6. Concrete workstreams (code-grounded)

**A — Phase 0: measurement (do first, ~1–2 wks).**
- A1: new eval adapter `real_app_callgraph` beside
  `crates/polint/src/eval/external/jelly_callgraph.rs` (BenchmarkAdapter
  pattern; manifests in `research/evaluation-harness/suites/`). Corpus: 5–10
  pinned Jelly-PLDI'24-artifact Node/TS apps (NodeProf oracles); 2–3 real Go
  repos vs x/tools callgraph RTA.
- A2: plumb `budget_reasons` (`js_points_to/solver.rs:240`) + peak RSS +
  wall-clock per case into `eval/metrics.rs` / `eval/performance.rs`.
- A3: baseline snapshots per repo-size bucket → F1-vs-size and RSS-vs-size
  curves in `.context/graph-benchmarks/`; gate future work on them.
- Acceptance: one command yields per-repo P/R/F1 + RSS + time + budget-event
  counts; committed baseline shows where F1 collapses with size.

**B — Phase 1: representation (no semantic change).**
- B1: per-file pipeline replacing the all-files-up-front load in
  `fs/mod.rs:85-135`; bounded concurrency ≈ cores; lazy source re-read for
  diagnostics.
- B2: kill solver hot-path clones (`solver.rs:422`, `:458`) first, then
  sorted-vec/roaring + hash-consed sharing for `sets: Vec<BTreeSet<TokenId>>`
  (`solver.rs:248`).
- B3: `max_indirection_depth` in `Budget` (`solver.rs:219`), off/high by
  default; tunable scale knob.
- B4: flamegraph one Jelly fixture for per-run fixed costs.
- Acceptance: Jelly F1 unchanged (89.06%); runtime and RSS down; A3 curves
  improve.

**C — Wire refined callgraph into data_flow (small, high value).**
Refined edges (from `refined_calls/`) consumed by interprocedural taint
instead of direct-only (`analysis/data_flow/direct_calls.rs`), under the same
capability gates.

**D — TS type-sidecar prototype.** Node script on the TS compiler API
emitting per-call-site receiver types/signatures → new `polint.ts.types`
provider → calls provider consumes typed resolution as XTA-grade tier before
the heap. Prototype on typed fixtures; measure recall delta; then decide
productionization (tsgo evaluation included).

**E — Summary store (keystone; gets its own plan when A–D land).** As
Phase 2/3 above; `.polint/cache/derived/` is the reserved location.

**Ordering:** A → B2/B4 (immediately, safe) → B1, B3 → C → D prototype → E.
Standing recommendation: stop Jelly-micro recognizer iteration at ~89% —
transferable returns have plateaued; Workstream A supplies the benchmark that
reflects the actual goal.

---

## 7. Key references

Scalable/demand-driven analysis: Sridharan & Bodík, refinement-based
points-to (PLDI 2006); Boomerang (ECOOP 2016); Synchronized Pushdown Systems
(POPL 2019); Zipper (OOPSLA 2018) yanniss.github.io/zipper-oopsla18.pdf;
Scaler (FSE 2018); context tunneling (OOPSLA 2018); Tip & Palsberg XTA
(OOPSLA 2000); parallel complexity of Andersen (POPL 2021) arxiv.org/abs/2006.01491.

JS/TS callgraphs: ACG (Feldthaus et al., ICSE 2013); JAM modular CG (ISSTA
2021) cs.au.dk/~amoeller/papers/jam/paper.pdf; approximate interpretation
(PLDI 2024) dl.acm.org/doi/10.1145/3656424 (artifact
zenodo.org/records/10930752); indirection-bounded CG (ECOOP 2024)
drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ECOOP.2024.10; missing-edge
root causes (ECOOP 2022) arxiv.org/abs/2205.06780; comparative study
arxiv.org/abs/2405.07206.

Incrementality/compositionality: Salsa
salsa-rs.github.io/salsa/reference/algorithm.html; Glean incrementality
glean.software/docs/implementation/incrementality/; LADDDER (PLDI 2021);
incremental CodeQL arxiv.org/abs/2308.09660; demanded abstract interpretation
(PLDI 2021; TOPLAS 2024); Infer at scale (CACM 2019); Tricorder (ICSE 2015);
Stubbifier arxiv.org/abs/2110.14162; Graspan (ASPLOS 2017); hash-consed
points-to sets (SAS 2021) yuleisui.github.io/publications/sas21.pdf.

ML: CGPruner (TSE 2022); AutoPruner arxiv.org/abs/2209.03230; MSR 2024
re-evaluation arxiv.org/abs/2402.07294; OriginPruner arxiv.org/abs/2412.09110;
SEA (ASE 2024) arxiv.org/abs/2408.04344; GRAPHIA arxiv.org/abs/2506.18191;
Graphick (OOPSLA 2020); AFD arxiv.org/abs/2509.22530; TypeT5
arxiv.org/abs/2303.09564; HiTyper arxiv.org/abs/2105.03595; TypeGen
arxiv.org/abs/2307.09163; TypyBench arxiv.org/abs/2507.22086;
CodeTIDAL5/JoernTI arxiv.org/abs/2310.00673
(github.com/joernio/joernti-codetidal5); LLift arxiv.org/abs/2308.00245; IRIS
arxiv.org/abs/2405.17238; LLMDFA arxiv.org/abs/2402.10754; RepoAudit
arxiv.org/abs/2501.18160; KNighter arxiv.org/abs/2503.09002; LLMs vs static
CG tools (EMSE 2025) arxiv.org/abs/2410.00603; ProGraML
arxiv.org/abs/2003.10536; Fluffy bimodal taint arxiv.org/abs/2301.10545;
Snyk CodeReduce arxiv.org/abs/2402.13291.

# Final Data-Flow Research Report

Date: 2026-05-15

## Executive Conclusion

The data-flow report should not be read as "pick IFDS" or "copy CodeQL." The research points to a layered native engine:

```text
CFG/local facts
  -> sparse value-flow graph
  -> bounded access paths and heap/property abstraction
  -> function summaries
  -> call-graph-aware interprocedural propagation
  -> query-scoped path search
  -> optional IFDS/IDE/relational solvers
```

Accuracy comes from the composition of representation, call graph quality, access-path precision, summary design, source/sink/sanitizer models, and domain-specific entrypoint recovery. The solver alone does not make the engine state of the art.

For polint, build a native `DataFlow<'_>` fact family that is general enough for taint, nilness, constants, typestate, dependency flow, secret flow, and agent/tool-boundary policies. Taint should be a rule/query layer over data-flow facts, not the whole engine.

The product-specific conclusion is stronger: polint should be an agent-extensible analysis framework, not a black-box analyzer that tries to infer every source, sink, sanitizer, summary, and framework lifecycle convention by itself. The native engine supplies the substrate; agents and repo-local rules add validated models for the codebase.

## Research-Level Accuracy Lessons

### IFDS/IDE is precise only for the right problem class

The IFDS paper proves polynomial-time, meet-over-all-valid-paths precision for finite, distributive subset problems. The headline bound is `O(E * D^3)` for exploded-supergraph tabulation, where `E` is supergraph edge count and `D` is the number of data-flow facts. Locally separable problems can be solved in `O(E * D)`.

That is powerful, but it has boundaries:

- facts must be finite;
- transfer functions must be distributive;
- aliasing/heap modeling is outside the theorem unless encoded into facts;
- the ICFG and call graph must already be good;
- `D` can grow quickly if access paths, heap objects, contexts, or labels are too broad.

Polint should implement IFDS later as an internal solver for finite clients. It should not make IFDS the first representation.

### Summary-based analysis is the practical first global layer

Pysa and CFTaint show why summaries work in production:

- summarize each function once;
- reuse summaries at call sites;
- iterate callers when callee summaries change;
- avoid eagerly enumerating every source-to-sink path;
- store TITO, source-return, param-sink, return, receiver, field, and unknown effects.

CFTaint reports **96.09% recall** and sensitive-data precision **93.51%** in its production benchmark framing, with average time around **127.31 seconds** and total time around **3.86%** of ANTaint. It achieves this by using field-based, compositional summaries, accepting some broad true positives for sensitive-data tracing.

The key lesson is not "field-insensitive is fine." The lesson is that industrial tools deliberately choose abstractions that preserve useful recall and make cost predictable.

### State-of-the-art multi-language analysis is representation design

YASA's 2026 paper is important because it focuses on representation, not just a solver. It argues that a unified AST plus language-specific semantic handlers can support Java, JavaScript, Python, and Go. Its reported xAST results outperform CodeQL and Joern on the benchmark; in deployment it analyzed over **100M LOC** across **7.3K applications**, surfaced **314** previously unknown taint paths, and had **92** confirmed as 0-day vulnerabilities. It reports average throughput around **31.8 KLOC/min**, versus **9.3 KLOC/min** for CodeQL and **17.1 KLOC/min** for Joern in its evaluation.

The caveat is important: the benchmark is an industrial micro-benchmark, and the real-world confirmed-path number is not a conventional precision/recall measure. Still, the engineering result is clear: multi-language capability depends on a common semantic substrate plus per-language handlers.

### Domain-specific entrypoint recovery changes recall

MCP-BiFlow is a direct agent-era lesson. It reports **93.8% recall** on 32 confirmed MCP vulnerability cases and finds **118** confirmed paths across **87** servers from **15,452** repositories. Its advantage is not a novel generic taint lattice; it recovers MCP entrypoints, models MCP request/return trust boundaries, and performs bidirectional interprocedural propagation.

For polint, this matters more than the MCP domain itself. AI-agent rule engines will need domain packs that define:

- entrypoints;
- source boundaries;
- sink boundaries;
- return-side flows;
- protocol/framework dispatch;
- guards and sanitizers.

### LLMs help with specifications, not with replacing analysis

SemTaint reports detecting **106 of 162** vulnerabilities previously undetectable by CodeQL, a **65.43% recall** over that selected CodeQL-missed set. It also reports reducing unresolved-call candidates from **94,909** to **10,184** with taint-reachability filtering, an **89.2% reduction**, before invoking LLM assistance.

The research lesson is strong:

- static analysis should remain the source of truth for facts;
- LLMs can propose sources, sinks, call edges, and summaries;
- every LLM-proposed fact must bind back to source spans/static facts;
- demand-driven candidate selection is necessary for cost.

This maps directly to polint's AI-agent rule-writing goal.

## Agent-Extensible Data Flow

The major product shift is that repo-specific models are part of the intended analysis lifecycle. Classic tools require the vendor or analyzer author to pre-model every framework. polint can let AI agents inspect the repository and add native model inputs for the exact internal APIs being analyzed.

Agent-authored data-flow models can define:

- sources: request bodies, queue messages, CLI args, env vars, secrets stores, MCP tool inputs, generated client responses;
- sinks: SQL, shell, files, network calls, templates, logs, telemetry, authorization decisions, agent tool outputs;
- sanitizers and barriers: validators, schema parsers, escaping functions, allowlist checks, permission guards;
- additional flow steps: builder chains, fluent APIs, context bags, serialization wrappers, dependency-injection lookups;
- function summaries: param-to-return, receiver-to-return, param-to-sink, field mutation, source-to-return, unknown effects;
- entrypoints and trust boundaries: HTTP routes, jobs, tests, MCP tools, generated RPC handlers.

These models should bind to static facts and carry provenance:

```text
provider = "repo_model"
model_id = "repo.auth_and_sql"
validation_status = "validated" | "unvalidated" | "failed"
confidence = "high" | "medium" | "low"
```

This matters because data-flow precision is dominated by modeling. A missing sanitizer creates false positives. An over-broad sanitizer creates false negatives. A missing source or entrypoint hides entire vulnerability classes. A missing call edge breaks interprocedural paths. polint should surface these as explicit model gaps:

```text
default unknown/havoc edges: 219
repo models loaded: 5
model sources added: 17
model summaries added: 43
unknown/havoc reduced: 88
paths added by models: 31
paths pruned by sanitizers: 12
unvalidated assumptions: 6
```

The engine remains fully native. Agents do not replace the solver; they author model facts that the native engine validates, caches, and exposes through typed SDK views.

### Incrementality belongs in the architecture early

IncIDFA proves a generic incremental algorithm for monotone iterative data-flow analyses with from-scratch precision. It reports update-time speedups up to **11x** with **2.6x geomean**, and total compilation-time improvement up to **46%** with **15.1% geomean**.

The architecture lesson is to store enough dependency information to re-run affected SCCs/summaries, not to bolt incrementality on after all solvers are built.

## Algorithm Complexity And Fit

Notation:

- `N`: CFG/IR nodes.
- `E`: CFG/value-flow/ICFG edges.
- `H`: lattice height.
- `D`: number of finite data-flow facts.
- `S`: function summary size.
- `C`: call graph edges.
- `A`: access-path count.
- `Q`: number of source/sink queries.

| Algorithm family | Cost model | Accuracy strengths | Accuracy weaknesses | Polint priority |
|---|---:|---|---|---|
| Intraprocedural worklist | `O(E * H * transfer_cost)` | Simple, predictable, exact for modeled local semantics. | No interprocedural flow; path-insensitive unless domain tracks guards. | First. |
| Sparse value-flow | Build `O(N + def_use_edges)`, queries `O(V + E)` | Avoids dense CFG propagation for value reachability. | Needs good def-use/place lowering. | First. |
| Taint graph reachability | `O(V + E)` per bounded source set; all-pairs can be expensive | Excellent for source-to-sink rules with path evidence. | Quality depends on flow edges, sanitizers, call graph. | First query layer. |
| Bounded access paths | Multiplies fact space by selected paths, roughly `O(E * A)` | Field/property precision for real policies. | Depth explosion; wildcarding creates false positives. | First, with low default depth. |
| Function summaries | Fixed point bounded by summary lattice height; roughly `O(iterations * (local + C * S))` | Practical global flow; cacheable and explainable. | Summary abstraction loses path/detail precision. | First global layer. |
| IFDS | General `O(E * D^3)`, locally separable `O(E * D)` | Precise valid-call/return matching for finite distributive facts. | Fact explosion; depends on ICFG and heap modeling. | Later internal solver. |
| IDE | IFDS plus edge functions/lattice values | Constants/ranges/typestate-style value propagation. | More complex joins/functions; same ICFG dependency. | Later. |
| Andersen points-to | Classic worst-case cubic; practical optimized cost varies | Heap/call/alias precision. | Memory and context/field sensitivity tradeoffs. | Later provider. |
| Datalog/semi-naive | Rule/join dependent; SCC and indexing dominate | Excellent for whole-program relation experiments. | Hard to expose safely as public API; joins can explode. | Internal engine inspiration. |
| Incremental IDFA | Worst case can approach recomputation; best case affected SCCs only | Preserves precision while reducing repeated-update cost. | Needs dependency tracking and stable fact IDs. | Design for early. |

## Implementation Accuracy By System

| System | Reported accuracy/cost | What the number means | What polint should learn |
|---|---|---|---|
| FlowDroid | **93% recall**, **86% precision** on DroidBench; InsecureBank in ~31 seconds with 7 verified leaks and no FP/FN in the paper's evaluation. | High-precision Android taint when lifecycle/callbacks are modeled. | Entrypoints/lifecycle matter as much as IFDS. |
| Heros | IFDS/IDE solver implementation, no product accuracy number by itself. | Solver correctness does not solve modeling. | Keep solver internal; public facts should show paths/precision. |
| Pysa | Summary fixed point over Python call graph; no universal benchmark precision in docs. | Production architecture emphasizes TITO/source/sink summaries. | Use summaries as first interprocedural layer. |
| CodeQL | Strong local/global/taint API; docs distinguish local precision from global cost. | Query model is proven ergonomically, not a native dependency. | Copy source/sink/barrier/additional-step API ideas. |
| Semgrep/OpenGrep | Docs explicitly say no path sensitivity, no soundness guarantee, limited pointer/shape analysis. | Lightweight multi-language rule engine trades precision for reach. | Expose heuristic status loudly. |
| Joern | CPG plus reachableBy/data-flow overlays. | Graph substrate enables rich queries, but semantics files are critical. | Use internal graph facts; keep SDK typed. |
| YASA | **31.8 KLOC/min**, **100M LOC**, **314** paths, **92** confirmed 0-days. | Multi-language semantic representation and handlers can scale. | Build common operations plus language-specific semantics. |
| CFTaint | **96.09% recall**, sensitive-data precision **93.51%**, ~**127.31s** average. | Field-based compositional summaries can beat heavier taint on industrial microservices. | Start summary-based, not path-enumeration-based. |
| MCP-BiFlow | **93.8% recall** on confirmed MCP cases; **118** confirmed real-world paths. | Domain entrypoint/source/sink modeling drives recall. | Add domain packs over generic data-flow facts. |
| SemTaint | **65.43% recall** over selected CodeQL-missed npm vulns; unresolved calls reduced by **89.2%** before LLM work. | LLMs improve specs when grounded by static facts. | Let agents propose models, then validate/bind them. |

## What "World-Class" Means For polint

World-class does not mean claiming complete static truth across Go, TS/JS, Java, and Python. It means:

- every data-flow fact has provenance;
- every path has precision/status;
- every unsupported construct creates a diagnostic or unknown edge;
- global flow is query-scoped or summary-based;
- rules can choose precision budgets;
- domain packs can define entrypoints and trust boundaries;
- AI-generated models bind to static facts before affecting analysis;
- the engine has benchmark fixtures and reports precision/cost deltas.

## Revised Implementation Path

1. **Local substrate:** CFG facts, places, operations, value-flow edges.
2. **Typed SDK:** `DataFlow<'_>` with node/edge/path inspection and precision labels.
3. **Bounded access paths:** default low depth, wildcard overflow, digest-aware config.
4. **Direct-call interprocedural:** use call graph facts for param/return/receiver edges.
5. **Function summaries:** TITO, param-return, source-return, param-sink, field/receiver effects, unknown effects.
6. **Repo-local model layer:** sources, sinks, sanitizers, barriers, summaries, additional steps, entrypoints, trust boundaries, and model validation.
7. **Source/sink/sanitizer query API:** rule-authored and config-authored matchers.
8. **Domain packs:** web handlers, MCP/tool boundaries, logging/secrets, DB/shell/file APIs.
9. **IFDS/IDE:** internal finite-fact solver after ICFG/summaries stabilize.
10. **Points-to/access-path refinement:** selected languages and high-value rule families.
11. **Incremental fixed points:** summary and relation invalidation by changed files/functions.

## What To Avoid

- Do not build a taint-only graph.
- Do not run whole-program global flow for every rule by default.
- Do not expose solver internals in the SDK.
- Do not hide call graph uncertainty.
- Do not treat LLM-generated source/sink specs as trusted unless they bind to static facts.
- Do not merge repo-model edges into native facts without provenance and validation status.
- Do not compare algorithms by graph size alone.

The research supports a native engine that is empirical, layered, and honest about uncertainty. That is the path that makes AI-agent-written rules genuinely useful instead of merely syntactic.

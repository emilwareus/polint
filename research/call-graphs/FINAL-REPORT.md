# Final Report: Multi-Language Call Graphs

Date: 2026-05-15

## Executive Conclusion

A call graph is not a single artifact. It is an approximation family parameterized by language semantics, entrypoints, dependency scope, dispatch model, heap/type abstraction, context sensitivity, and feature models for reflection, callbacks, dynamic imports, decorators, async dispatch, and framework lifecycle.

The research result is sharper than the original implementation report:

- **Accuracy is algorithm-plus-implementation, not algorithm alone.** Java papers show that the theoretical ordering `CHA >= RTA >= VTA/points-to` does not reliably hold in real tools when lambdas, reflection, native models, and configuration interact.
- **Precision and recall must be measured on edges, not just reachable methods or graph size.** Total Recall shows graph size is not a reliable proxy; smaller graphs can have lower precision and larger graphs can still miss true edges.
- **Dynamic languages need value-flow/name-flow, but still remain heuristic.** PyCG, JARVIS, Jelly, TAJS, and CodeQL all demonstrate that callable values, object properties, imports, callbacks, and dynamic dispatch are the actual hard problem.
- **A full project call graph should include unresolved facts.** Omitting unresolved calls hides false-negative risk; treating them as first-class facts lets rules choose their precision budget.
- **polint can raise the ceiling with repo-local models.** Unlike a generic black-box analyzer, polint can let AI agents add validated call models for the specific repository's routers, decorators, dependency injection, generated clients, event buses, and tool registration patterns.

For polint, the correct product is a layered `Calls<'_>` / `CallGraph<'_>` fact family with algorithm provenance, confidence, status, and unresolved reasons on every edge.

## Metrics And Evaluation Caveats

Call graph evaluation is easy to get wrong:

- **Precision**: fraction of emitted static call edges that can execute at runtime.
- **Recall**: fraction of executable runtime call edges present in the static graph.
- **Micro-benchmark soundness**: feature tests passed. Useful for known language features, weak as a whole-program accuracy metric.
- **Dynamic baseline**: observed runtime edges from selected entrypoints and inputs. Useful but only approximates the true graph.
- **Graph size**: not a valid accuracy metric by itself.

The best research practice is to report precision and recall against entrypoint-aligned dynamic baselines, plus feature-level micro-benchmark results for known dynamic features.

## Algorithm Cost And Accuracy

Notation:

- `N`: AST/IR instructions.
- `C`: call sites.
- `F`: functions/methods.
- `T`: classes/types.
- `A`: allocation sites.
- `E`: constraint or flow edges.
- `K`: context variants.

| Algorithm family | Approximate cost | Precision profile | Recall/soundness profile | Where it fits in polint |
|---|---:|---|---|---|
| Syntactic call-site extraction | `O(N)` | Exact for finding call expressions, no callee precision. | High call-site recall if parser covers language; zero target recall for dynamic dispatch. | Always-on baseline. |
| Direct lexical/import binding | `O(N + imports + refs)` with indexes | High precision for named/static calls. | Misses dynamic calls, interface dispatch, callbacks, reflection. | First useful target facts for Go and TS/JS. |
| CHA | `O(T + C * subtype_lookup)` after hierarchy indexing; naive subtype closure can approach `O(T^2)` | Low precision for OO dispatch because every subtype is considered. | Good only under complete hierarchy and modeled language features; Java research shows real implementations still miss lambdas/reflection. | Java/JVM first semantic algorithm; maybe Go interface over-approximation later. |
| RTA | Worklist over reachable functions/types, roughly `O(F + A + C * reachable_types)` | More precise than CHA by restricting to allocated reachable concrete types. | Less sound if entrypoints, allocation discovery, reflection, or dynamic loading are incomplete. | Best first "serious" algorithm for Go/Java-style closed-world roots. |
| VTA/XTA/MTA/FTA | Constraint propagation over variables/types, roughly `O(E * T / word_size)` with bitsets, worse with fields/context | More precise receiver sets than RTA. | Can recover edges RTA misses when type flow is needed; more setup-sensitive. | Experimental provider after call facts and summaries exist. |
| Andersen/points-to | Classic worst case `O(n^3)`; practical solvers depend on SCCs, bitsets/BDDs, field/context choices | Can be highly precise for heap/call targets when field/context sensitivity is tuned. | Over-approximates; context sensitivity can improve precision but can hit a tractability wall. | Later optional provider, not a v1 default. |
| `k`-CFA / context-sensitive CFA | State space grows with context abstraction; commonly expensive as `K` grows | Better higher-order/callback precision. | Sensitive to context explosion and library scope. | Not default; useful for Java/JS research mode. |
| JS/Python function-token flow | Fixed point over assignments/imports/properties/returns: roughly `O(E * function_tokens)` | Good for first-class functions, callbacks, module exports. | Heuristic around dynamic property names, `eval`, monkeypatching, decorators. | Best TS/JS and Python direction after direct binding. |
| Demand-driven call resolution | Query cost proportional to reachable slice, not whole program | High precision for selected questions. | Can miss edges outside seed slice. | Good for rule-specific queries and agent workflows. |

The implementation should record these algorithm classes directly in facts so a rule can ask, for example, "only direct/binding/RTA edges" or "include heuristic value-flow edges."

## Agent-Extensible Call Graphs

The product path changes one old assumption in call graph research: polint does not need to ship one analyzer that magically fits every codebase. The native engine should expose what it can prove, what it can approximate, and what it cannot resolve. AI agents can then inspect the repository and add explicit repo-local call graph models for the patterns that matter in that codebase.

Examples:

- TypeScript route registration: `router.get(path, handler)` creates an entrypoint edge to `handler`.
- Python decorators: `@app.tool` or `@mcp.tool` creates a tool-entrypoint edge.
- Java dependency injection: an interface call can be connected to repo-configured bindings.
- Go registries: `registry.Register(name, handler)` creates a synthetic entrypoint to `handler`.
- Generated clients: service stubs can be connected to generated or remote boundary symbols.

Model-produced facts should not be hidden inside generic "heuristic" edges. They need their own provenance:

```text
provider = "repo_model"
model_id = "repo.fastify.routes"
validation_status = "validated" | "unvalidated" | "failed"
confidence = "high" | "medium" | "low"
```

This makes unresolved calls actionable. Instead of asking the native analyzer to auto-discover every internal framework, polint can report:

```text
default unresolved: 142
repo models loaded: 3
model edges added: 87
unresolved reduced: 61
unvalidated model assumptions: 4
```

Rules should be able to choose graph tiers: direct only, native semantic, heuristic, repo-model, or all. Debug exports should always separate default and extended graphs so model quality can be measured rather than assumed.

## Empirical Research By Language

### Python

PyCG is the classic whole-program Python baseline. Its paper reports high precision around **99.2%** and recall around **69.9%** on macro-benchmarks, with average speed around **0.38 seconds per 1 KLOC**. That is an important result: even a purpose-built Python call graph generator with high precision still misses roughly 30% of call edges in real packages.

JARVIS attacks PyCG's scale problem by making call graph construction application-centered and reusing function type graphs. Its paper reports average application-centered whole-program generation around **8.16 seconds**, and claims improvements over PyCG of at least **67% faster runtime**, **84% higher precision**, and at least **20% higher recall** in the application-centered whole-program setting. It also reports that PyCG can time out or run out of memory under exhaustive library-inclusive analysis.

Research implication:

- Python needs entrypoint-aware, application-centered analysis.
- Whole dependency closure is often the wrong default.
- Name/return/argument points-to is essential, but must be bounded.
- Precision labels are mandatory because dynamic attribute writes, monkeypatching, decorators, `__getattr__`, `__call__`, and import side effects cannot be globally resolved with confidence.

### JavaScript / TypeScript

The comparative JavaScript call graph study is the clearest evidence that "the JS call graph" is tool-dependent. On SunSpider:

- ACG reported about **99% precision** and **91% recall**.
- TAJS reported about **98% precision** and **71% recall**.
- Closure reported about **81% precision** and **89% recall**.
- WALA reported about **87% precision** and **49% recall**.
- ACG + TAJS together reached about **98% precision** and **99% recall** under the study's union-based recall metric.
- Combining all tools reached full union recall but precision dropped to about **74%**.

The study also warns that this recall is not absolute ground truth; it is recall against validated edges found by the tool set. Still, the result matters: independent tools find different true edges, so a single JS algorithm is unlikely to dominate.

Research implication:

- JS/TS should start with Oxc call-site and binding facts, then add bounded function-token value-flow.
- Dynamic property calls, callbacks, module systems, prototype chains, framework routing, JSX components, async continuations, and `eval` must be modeled as separate precision tiers.
- A "complete" JS graph should include unresolved/dynamic call facts, not just resolved edges.

### Java / JVM

Java has the richest call graph literature, but the latest research is also the strongest warning against naive trust in algorithm names.

Unimocg found that Soot and WALA algorithms passed only **41-53%** of a 123-case JVM feature benchmark across tested algorithms/configurations. Unimocg's modular design, which decouples type producers from call resolvers, raised feature support to **79-81%** across CHA/RTA/XTA/0-CFA/1-1-CFA while keeping support more consistent across algorithms.

The 2026 unsoundness paper shows that expected precision partial orders break in actual frameworks. Examples:

- WALA object sensitivity can reshape call graphs with only about **39.1%** similarity to baseline in some configurations because reflection handling changes.
- SootUp CHA and RTA can be **95.8%** similar but still have algorithm-level violations.
- Soot/SootUp/WALA equivalent algorithms can have very low similarity, often around **10-33%**, due to lambdas, invokedynamic, reflection, native methods, static initializers, and entrypoint scope.
- Doop object-sensitive variants hit a **600 second timeout** in high-precision comparisons.

Total Recall adds another key point: reachable-method metrics can look good while edge precision and recall remain poor. For Batik, all-edge precision/recall numbers were dramatically lower than reachable-method metrics. The paper's takeaway is that call graph size and reachable methods are insufficient proxies.

Research implication:

- Java support must make classpath, entrypoints, library scope, reflection policy, native policy, and invokedynamic/lambda support explicit.
- Provider modularity matters more than choosing "CHA vs RTA" by name.
- polint should copy Unimocg's separation: type producers, type iterators, call resolvers, and graph consumers.

### Go

Go is structurally easier than JS/Python but not trivial:

- Direct/static SSA callees are precise but miss interfaces and dynamic function values.
- CHA over Go method sets is cheap and partial-program friendly but broad for interface dispatch.
- RTA is a strong choice when roots are known because it restricts dispatch by reachable allocations.
- VTA is more precise but experimental in `x/tools`, and its quality depends on package loading, SSA construction, test inclusion, build tags, and module roots.

Research implication:

- Go should be polint's first semantic call graph because the language lifecycle is controllable.
- The Go provider must reuse the project's existing Go module-root/build-tag/test lifecycle contract.
- `static`, `CHA`, `RTA`, and `VTA` should be separate algorithm labels, not one merged "Go call graph."

## Implementation Accuracy Notes

| Implementation | Accuracy lesson | Cost/scaling lesson | Polint lesson |
|---|---|---|---|
| Go `x/tools` | Multiple algorithms expose different assumptions. | RTA/VTA need roots and SSA/package setup. | Mirror the algorithm ladder; do not hide lifecycle diagnostics. |
| CodeQL | Strong query ergonomics and explicit incompleteness/imprecision in JS data-flow call nodes. | Database extraction amortizes cost, but not a native option. | Copy the API honesty, not the dependency. |
| Jelly | Function tokens and value-flow fit JS/TS better than class hierarchy thinking. | Constraint solving must be bounded by package/module scope. | Implement a lightweight native function-token provider later. |
| PyCG | High precision but moderate recall and whole-program scaling limits. | Exhaustive dependency analysis is expensive. | Python should be entrypoint/application-centered by default. |
| JARVIS | Application-centered analysis improves practical precision/recall and runtime. | Reuse function type graphs and avoid dependency bloat. | Good model for future Python provider. |
| Soot/WALA/SootUp | Mature but feature support varies sharply by algorithm/configuration. | Higher precision can trigger semantic differences and timeouts. | Keep configuration and feature support explicit in facts. |
| Unimocg/OPAL | Modular type producers/resolvers improve consistency. | Avoids emulating simple algorithms through heavy points-to when not needed. | Best architecture pattern for native polint providers. |
| Doop | Datalog is excellent for experimenting with points-to families. | Heavy relation solving and high precision can be expensive. | Useful inspiration for internal relation engine, not v1 dependency. |

## Revised Polint Direction

The next call graph phase should not say "build a call graph." It should say:

> Add language-neutral call-site and call-edge facts, explicit unresolved call facts, direct/binding resolution for Go and TS/JS, a provider registry for semantic algorithms, and a repo-local call-model layer with measured precision/cost metadata.

Minimum first version:

1. `CallSiteFact` for every syntactic call expression.
2. `CallEdgeFact` for direct/binding-resolved targets only.
3. `UnresolvedCallFact` or unresolved edge status for dynamic/interface/unsupported calls.
4. `Calls<'_>` SDK view for call-site queries.
5. `CallGraph<'_>` SDK view for graph traversal over selected algorithm tiers.
6. Repo-local call graph model loading with model identity and validation status.
7. Debug export that reports counts by language, algorithm, status, unresolved reason, and model provenance.

Then add:

1. Go SSA static/RTA provider.
2. Go interface/method-set CHA provider.
3. TS/JS bounded function-token provider.
4. Go VTA experimental provider.
5. Java CHA/RTA when Java lifecycle exists.
6. Python application-centered name/return/argument provider.

## Research-Driven Defaults

Default graph:

- syntactic call sites;
- direct lexical/import/static edges;
- unresolved facts;
- no whole dependency closure;
- no high-cost points-to unless requested.

Optional precision tiers:

- `semantic_basic`: Go static/CHA, Java CHA, TS/JS module binding.
- `semantic_reachable`: Go/Java RTA with configured roots.
- `value_flow`: TS/JS/Python callable-token flow.
- `points_to`: Java/Python/Go advanced analysis.
- `heuristic_framework`: framework-specific routers, decorators, reflection, dynamic imports.
- `repo_model`: agent-authored repository models that bind framework, generated-code, DI, router, callback, and tool-registration patterns to native facts.

Every tier should be benchmarked by:

- edge count;
- unresolved count;
- unresolved reduction relative to the default graph;
- runtime;
- memory;
- model edges by model id;
- model validation failures;
- precision/recall on fixtures where ground truth exists;
- diff against lower tiers.

This turns call graphs into an experimental analysis platform, which is what polint needs for AI-agent-written policies.

# Deep Research Analysis

## Problem Statement

polint needs a kernel before it grows more analysis families.

The risk is not that we cannot implement a call graph or data-flow pass. The risk is that each pass gets its own private:

- scheduling model;
- cache key;
- invalidation semantics;
- extension format;
- provenance fields;
- precision vocabulary;
- validation behavior;
- unknown handling;
- evidence/path export.

That would create a set of individually useful features that cannot compose.

The kernel's job is composition.

## The Two Axes That Matter

Most systems we researched fall along two axes:

1. **How computations are scheduled**
   - eager fixed pipeline;
   - demand-driven queries;
   - dependency-tracked incremental queries;
   - bottom-up relation/fixpoint evaluation;
   - build-style affected-work scheduling.

2. **How facts are represented**
   - typed in-memory objects;
   - relations/tuples;
   - property graph nodes/edges;
   - compiler IR objects;
   - source-code index entries.

polint needs a hybrid because its workloads are mixed.

Per-file syntax facts are cheap, parallel, and deterministic. Whole-repo symbol resolution is graph-shaped. Call graph and data flow can be recursive. Agent extensions need validation and stable identity. Rules need ergonomic typed views.

No single existing architecture maps perfectly.

## Why Not Pure Salsa

Salsa is extremely attractive:

- global revisions;
- tracked dependencies;
- red-green reuse;
- backdating when outputs do not change;
- durability levels;
- snapshots for parallel reads;
- LRU controls.

rust-analyzer proves that this model can power a serious language server.

But pure Salsa is a bad first kernel choice for polint:

- polint's fact-family boundaries are still changing;
- many outputs are large relation sets, not scalar query results;
- the current product is batch CLI/CI first, not live IDE first;
- adopting Salsa forces storage and lifetime decisions too early;
- relation/fixpoint analyses still need explicit solvers inside queries;
- extension merge/validation is not solved by Salsa.

Recommendation: copy the algorithmic lessons, not the crate dependency, in the first implementation.

### What to copy from Salsa and rust-analyzer

- revision counters for inputs;
- "maybe changed after" checks;
- dependency edges between layer outputs;
- output equality/backdating;
- durability classes:
  - low: source files and local config;
  - medium: repo-local rules/extensions;
  - high: downloaded dependencies, locked toolchain metadata, built-in schemas;
- snapshots for read-only parallel providers;
- LRU limits for large memoized outputs;
- invalidation barriers like rust-analyzer's item trees/def maps.

The invalidation barrier idea is especially important. A whitespace edit should not invalidate exported symbols. A function-body edit should not invalidate module imports. A private helper edit should not invalidate package-level call graph summaries unless the summary shape changes.

## Why Not Pure Datalog

Souffle, Doop, CodeQL, and FlowLog show the power of facts plus rules.

They get:

- relations with typed columns;
- indexes chosen by access pattern;
- SCC scheduling;
- semi-naive recursive fixpoint;
- least fixed point semantics;
- explainable tuple derivations;
- clean extraction boundary.

This is ideal for call graph reachability, data-flow propagation, summaries, and effects.

But pure Datalog is not the right first public shape:

- polint's rule authoring model is Rust typed fact views;
- agent extensions are Rust code by product choice, not only declarative tuples;
- full query-language design would delay native kernel work;
- codebase-specific analysis may need arbitrary Rust logic;
- full proof-tree provenance is expensive.

Recommendation: implement relation/fixpoint internals where needed, with typed Rust provider APIs.

## Why Not a Joern-Style Public CPG

Joern's code property graph and overlays are useful:

- named layers;
- layer dependency declarations;
- graph traversal ergonomics;
- data-flow overlays added after base/controlflow/type/callgraph layers.

But a mutable global property graph as the SDK surface would fight polint's current contract:

- public graph schemas become semver liabilities;
- mutable overlays make provenance and cache invalidation harder;
- graph traversal APIs are powerful but easy to make imprecise;
- typed fact views are a better public rule surface.

Recommendation: copy named overlays/layer manifests, not the public mutable graph.

## Why WALA Still Matters

WALA is a reminder that mature static analysis systems expose typed analysis products:

- IR;
- SSA;
- CFG;
- call graph;
- pointer analysis;
- IFDS supergraph;
- slicer.

This is closer to polint's desired internal shape than one global graph. Each analysis product has explicit options, caches, and consumers.

Recommendation: treat future `CFG`, `CallGraph`, `DataFlow`, `Effects`, and `Slices` as typed products/fact families with dependencies, not as arbitrary graph edges.

## Why TypeScript and gopls Matter

TypeScript and gopls are practical production references for cache keys and affected work.

TypeScript distinguishes file versions from public signatures. Its incremental compiler can avoid downstream work when public declaration shape is unchanged. That maps to polint's need for family-level shape digests:

- import shape;
- export symbol shape;
- function signature shape;
- call-site shape;
- local CFG shape;
- summary shape.

gopls uses recipe keys over analyzers, packages, facts, dependencies, and source inputs. It deduplicates in-flight work and persists serialized outputs. That is a good model for polint provider outputs.

Recommendation: every provider should be able to compute a recipe/layer key before running, and a normalized output digest after running.

## Why Pyre Matters

Pyre separates value dependencies from presence dependencies. This is subtle and important.

polint will need to know whether a result depends on:

- the value of a symbol;
- the existence of a symbol;
- the absence of a symbol;
- an unresolved import remaining unresolved;
- a model selector matching zero entities.

Without presence dependencies, incremental behavior becomes unsound for "missing" facts.

Recommendation: represent both value dependencies and membership/presence dependencies in the kernel.

## Why Kythe and SCIP Matter

Kythe and SCIP are not full analysis kernels, but they are strong references for cross-language identity.

Kythe's VName design separates:

- signature;
- corpus;
- root;
- path;
- language.

It explicitly does not include revision in the name; revisions are facts. This is a useful warning: stable names should describe what an entity is, not when it was observed.

SCIP separates:

- documents;
- occurrences;
- symbols;
- relationships;
- tool metadata;
- position encodings.

It also acknowledges that indexers can range from compiler-backed precision to local syntax-directed heuristics.

Recommendation: polint stable keys should combine language, repo/root, relative path, semantic signature, build/lifecycle discriminator, and fact-family-specific payload. Revisions and validation status should be metadata, not part of identity unless they change the represented entity.

## Accuracy And Complexity

The kernel itself is not a precision algorithm, but it controls precision accounting.

### Scheduling complexity

Provider DAG scheduling is:

```text
O(P + E)
```

for `P` providers and `E` provider dependency edges, plus provider runtime.

Deterministic merge adds sorting cost:

```text
O(N log N)
```

for `N` emitted facts in a family when stable ordering/dedup is required. Many families can use hash/set dedup followed by final stable sort.

### Relation/fixpoint complexity

For recursive monotone analyses, worst-case cost is bounded by the size of the finite relation domain and rule joins. In practice, cost depends on:

- relation cardinality;
- join fanout;
- index choice;
- delta size per iteration;
- SCC size;
- precision knobs such as context sensitivity and access-path depth.

Semi-naive evaluation reduces repeated derivation by using deltas, but it does not make inherently explosive analyses cheap. The kernel must expose budgets and precision/cost knobs per provider.

### Cache complexity

Layer cache lookup is approximately:

```text
O(size_of_key_inputs)
```

plus serialization/deserialization. The expensive part is computing stable output digests for large relation layers. This should be streamed and normalized deterministically.

### Validation complexity

Validation cost depends on fact type:

- schema validation: linear in emitted facts;
- referential validation: linear if indexes exist;
- selector resolution: `O(log N + matches)` with indexes, potentially worse without;
- conflict detection: hash/dedup plus final sort;
- fixture validation: bounded by fixture suite runtime.

Validation should be mandatory for extension facts and test-mode native facts. CLI can make deep invariant validation debug-only after confidence grows, but lightweight validation should always run.

## Precision Lessons

Do not use one global "confidence" field as a substitute for precision.

Examples:

- A parser span can be exact and high-confidence.
- A CHA call edge can be conservative and high-confidence.
- A JavaScript dynamic call heuristic can be heuristic and medium-confidence.
- A repo extension summary can be user-asserted and high-confidence after fixture validation.
- A generated barrier can be low-confidence even if its selector resolves.

The kernel should preserve both fields.

## Unknowns As Data

Unknowns should be facts, not absence.

Examples:

```text
UnresolvedImport:
    import_id
    reason: package_not_loaded | dynamic_expression | setup_missing | unsupported

UnresolvedCall:
    call_site_id
    reason: dynamic_dispatch | missing_type | framework_callback | reflection | budget_exceeded

UnknownDataFlowStep:
    node_id
    reason: missing_summary | unsupported_heap_path | unknown_sanitizer
```

This enables the product loop:

```text
unknown exposed
  -> agent investigates
  -> extension adds model/fact
  -> unknown becomes resolved or narrowed
  -> delta report proves improvement
```

## Risk Areas

### Risk: premature generic engine

Building a universal query system before knowing fact families can waste months. Mitigation: first implement provider manifests and layer caches around existing passes.

### Risk: public API leakage

Kernel types are powerful and tempting to expose. Do not put them in `sdk::prelude`. Public SDK should stay typed views and stable rule ergonomics.

### Risk: extension facts silently weaken analysis

Generated sanitizers/barriers/suppressions can hide real problems. Mitigation: additive-only first, strict validation, fixtures for negative facts, precision ceilings.

### Risk: cache invalidation bugs

Layer keys that miss config/lifecycle/provider inputs will create stale facts. Mitigation: provider manifests declare all key inputs; regression tests mutate one input at a time.

### Risk: provenance overhead

Naively storing proof trees for every fact will hurt memory and runtime. Mitigation: sidecar provenance with compressed parent refs; detailed proof paths only for requested diagnostics/debug mode.

### Risk: recursive analysis blowups

Call graph/data-flow relation sizes can explode. Mitigation: budgets, precision knobs, context-depth limits, access-path depth, SCC scheduling, stats reports, explicit `BudgetExceeded`.

## Polint-Specific Implications

The current `AnalysisDb` can survive the first kernel slice. Do not rewrite storage immediately.

The first change should be orchestration:

```text
runner::analyze_and_run
  -> analysis_kernel::AnalysisKernel::run
  -> rules
```

Then internal providers:

- Go syntax provider;
- TS syntax provider;
- module graph provider;
- symbol/reference provider;
- metrics provider.

Only after those are manifests should we add extension providers.

The most valuable first extension family is `Entrypoints<'_>` because it exercises:

- extension discovery;
- provider inputs;
- fact validation;
- provenance;
- merge;
- capability planning;
- downstream rule consumption;
- default-vs-extended delta reports.

Call graph and data flow should consume this kernel instead of defining their own extension story.


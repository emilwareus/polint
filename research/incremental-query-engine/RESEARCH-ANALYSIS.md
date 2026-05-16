# Research Analysis

## The Design Space

There are four major ways to build incrementality for static analysis.

| Family | Examples | Good at | Weak at |
|---|---|---|---|
| Demand query engine | Salsa, DICE, Adapton | On-demand computation, dependency recording, equality pruning, interactive latency. | Requires strict input discipline; can be overkill for cheap layers. |
| Snapshot/module invalidation | gopls, Pyright, Pyrefly, TypeScript builder | Practical language-service performance, file/module/package invalidation, shape checks. | Can over-invalidate expensive interprocedural facts. |
| Build graph engine | Bazel Skyframe, Buck2 DICE | Huge dependency graphs, reverse deps, dirty checking, equality pruning, transactions. | Build artifacts are not the same as semantic fact layers; public SDK fit is poor. |
| Relation/fixpoint engine | Souffle, Differential Dataflow, FlowLog, CodeQL | Recursive graph relations, Datalog-like closure, delta evaluation, provenance. | Memory cost, relation modeling complexity, weaker fit for file/lifecycle/rule/extension cache keys. |

Polint needs a hybrid. The core product is not a compiler, language server,
build system, or Datalog database. It is a typed fact framework for repo-local
rules and agent-authored analysis extensions.

## Why A Pure Salsa Design Is Too Early

Salsa is a strong Rust-native query engine, but making the whole kernel Salsa
first would force early decisions about:

- how every fact family maps to query functions;
- how extension providers enter the query graph;
- how persistent batch caches relate to in-memory revisions;
- how fact provenance and validation status are stored;
- whether every rule view is a query;
- how official language tool invocations are tracked;
- how multi-language layer digests are represented.

Those are not stable yet. The first implementation needs explicit layer
manifests and dependency digests regardless of whether a long-lived daemon is
running. Salsa's ideas should be copied:

- revision counter;
- query dependency recording;
- shallow and deep verification;
- durability;
- backdating when recomputed output equals old output;
- cancellation on new changes.

The dependency on the Salsa crate can be revisited later for the demand-query
subsystem, but it should not shape the public fact architecture now.

## Why A Pure Datalog Design Is Too Early

Datalog is attractive for call graphs, data flow, reachability, and summaries.
Souffle and CodeQL prove that relation engines can scale. FlowLog and
Differential Dataflow show where incremental relation systems are going.

But polint also needs:

- parser/lifecycle caches;
- source spans and stable anchors;
- extension validation gates;
- fact precision/status;
- diagnostics/evidence;
- official language-tool provider boundaries;
- per-rule options and public SDK views.

A Datalog engine can be a sub-engine for recursive relation families. It should
not be the first storage model for every fact.

The first relation path should be:

```text
typed Rust facts
  -> selected relations for recursive analysis
  -> semi-naive delta evaluation
  -> indexed joins
  -> provenance labels
  -> query/layer output digest
```

Only after benchmarks show large relation volumes should polint consider a more
differential backend.

## Why A Pure Module-Level Cache Is Insufficient

Pyrefly, Pyright, and gopls show module/file-level incrementality can be very
fast and maintainable. That is enough for parsing, package metadata, imports,
symbols, and many type-checking flows.

It is not enough for:

- one diagnostic evidence path;
- one alias query;
- one source-to-sink path;
- one function summary;
- one framework dispatch expansion;
- one rule-specific expensive graph query.

Without demand queries, the engine either computes too much or cannot answer
precise questions cheaply. Therefore polint should start coarse, but design the
boundary so fine-grained queries can be added without a rewrite.

## Algorithm Family Analysis

### Red-Green Verification

The red-green algorithm checks whether a memoized query result is still valid
after input changes. A query is green if all dependencies are green or unchanged.
It is red if a dependency changed in a way that affects the result.

Accuracy:

- very strong if every dependency read is tracked;
- unsafe if providers read files/env/global state without declaring inputs;
- equality/backdating can preserve downstream results even after recompute.

Time:

- hot hit: usually constant-time revision/durability checks;
- cold verify: proportional to previous dependency edges;
- red recompute: query-specific.

Polint use:

- demand queries in daemon/watch mode;
- expensive local facts and interprocedural facts;
- not the first mechanism for batch file parsing caches.

### Shape-Signature Invalidation

TypeScript's builder and rust-analyzer's item trees show that not every source
edit matters to every dependent. The engine should compute shape digests that
match the dependency's semantic need.

Examples:

| Shape | Affects |
|---|---|
| Text | parser output, local diagnostics, local summaries. |
| Import shape | module graph, package ownership, import resolution. |
| Public API shape | cross-file references, call targets, external summaries. |
| Framework boundary shape | entrypoint graph, trust boundaries, dispatch edges. |
| Summary effect shape | caller summaries, interprocedural data flow. |

Accuracy:

- strong when shape extraction is language-correct;
- must be conservative for dynamic languages and generated code;
- should represent unknown shape when extraction fails.

Time:

- extracting shapes is usually linear in parsed file size;
- propagation is proportional to reverse dependency closure whose required
  shape changed.

Polint use:

- mandatory from the first layer cache;
- especially important for TypeScript/JavaScript, Go package exports, Python
  imports, and Java public methods/classes.

### Reverse Dependency Dirtying

Pyright, TypeScript, Skyframe, and DICE all propagate changes through reverse
dependencies. This is the default invalidation planner.

Accuracy:

- conservative if reverse dependencies overapproximate;
- unsafe if reverse dependencies are missing.

Time:

- planning cost is changed nodes plus reverse dependency closure.

Polint use:

- input to layer;
- import to module graph;
- public API to references;
- summary to caller summaries;
- extension/model to emitted and influenced facts.

### Equality Pruning And Backdating

If a dirty node recomputes to the same value, downstream dependents can stay
green. Salsa calls this backdating; Skyframe and DICE use equivalent value
version behavior; TypeScript uses signature equality.

Accuracy:

- equality must include semantic payload and precision/status fields;
- provenance changes may or may not invalidate downstream facts depending on
  consumer requirements.

Time:

- requires stable hashing or equality comparison of output;
- saves downstream recomputation when edits are local.

Polint use:

- public API signatures;
- summary payloads;
- call graph edge sets;
- data-flow step sets;
- diagnostics/evidence fingerprints.

### Semi-Naive Relation Evaluation

Souffle's semi-naive evaluation computes recursive relations using deltas so
each round joins only newly discovered facts where possible.

Accuracy:

- exact for the encoded Datalog rules and input facts;
- precision depends on relation encoding and abstraction choices.

Time:

- data-dependent: roughly iterations times indexed delta join cost;
- indexes are essential;
- dense relations can still be expensive.

Polint use:

- transitive reachability;
- recursive call graph expansion;
- summary/data-flow propagation;
- slice/evidence reachability;
- only after typed fact layer is stable.

### Differential Dataflow

Differential Dataflow maintains changes over partially ordered timestamps and
can update nested iterative computations.

Accuracy:

- strong for monotone/differentializable relation programs;
- requires careful representation of negation, deletes, and timestamps.

Time and memory:

- updates can be proportional to deltas;
- maintained traces can be memory-heavy;
- the CodeQL incremental paper is a practical warning: fine-grained
  incrementality can trade large memory for fast updates.

Polint use:

- future high-volume relation backend if semi-naive recompute becomes a limit;
- not first implementation.

### Incremental Iterative Dataflow

IncIDFA targets monotone iterative data-flow analyses. Instead of resetting an
affected SCC to the least informative value, it updates more selectively.

Accuracy:

- applies to monotone data-flow frameworks;
- not a universal replacement for all analyses.

Time:

- paper reports meaningful speedups, but worst-case can still approach full
  recompute when the affected SCC is large or the change is broad.

Polint use:

- future optimization for interprocedural data-flow domains after the summary
  SCC cache exists;
- first version should recompute affected SCC closure and backdate equal output.

### Demand-Driven Abstract Interpretation

Demanded abstract interpretation builds and updates only the parts needed to
answer a query. This is aligned with polint rule views.

Accuracy:

- can preserve soundness/termination if the abstract interpreter records
  dependencies and fixpoint obligations;
- risks unsoundness if query cuts ignore needed dependencies.

Time:

- cheap for narrow questions;
- can degrade to broader fixpoint when queries touch global state.

Polint use:

- nilness/string/typestate/alias/evidence queries requested by rules;
- should be bounded by budgets and precision labels.

## Per-Language Implications

### Go

Go has strong official tooling and clear module/package lifecycle. Polint should
use Go lifecycle inputs from `go.mod`, `go.sum`, `go.work`, build tags,
package patterns, test inclusion, and Go toolchain version.

Recommended invalidation:

- body edit: parse file, local MIR, local domains, local summaries;
- export/signature edit: package public API, importing packages, call targets,
  summaries;
- `go.mod`/`go.sum`/`go.work` edit: module graph, package loading, imports,
  tool-reported type facts;
- build tag edit: affected file membership and package facts.

### TypeScript And JavaScript

TypeScript already has a strong incremental design. Polint should copy the
shape idea but own the normalized facts.

Recommended invalidation:

- source text edit: parse, local symbols, local MIR;
- export/type declaration shape edit: dependent module references and call
  target facts;
- `tsconfig` path/module options edit: resolution and module graph;
- package lock/manifest edit: dependency topology and import ownership;
- extension model edit: framework dispatch/data-flow facts.

### Python

Python import and type behavior is dynamic. Polint should be conservative.
Pyright/Pyrefly/Pyre show import-graph dirtying and module-level transactions
are practical.

Recommended invalidation:

- source edit: parse, imports, local facts, summaries;
- import shape edit: dependents through import graph;
- package metadata/interpreter policy edit: module graph and import resolution;
- type/provider facts should carry precision labels and unknowns.

### Java/JVM

JVM analysis is classpath/build-lifecycle sensitive. Polint should use official
JDK/JVM metadata and build-tool outputs where appropriate, normalized into
polint-owned facts.

Recommended invalidation:

- method body edit: local CFG/MIR/domains/summaries;
- public class/method signature edit: classpath dependents, call resolution,
  overrides, summaries;
- build file/classpath edit: module graph, type facts, resolution;
- generated/annotation processor inputs: explicit lifecycle or extension inputs.

## Product-Specific Consequences

### Extension-Aware Incrementality

Traditional analyzers often hide models inside the tool. Polint lets agents
write Rust code that extends analysis. That means:

- extension code digest is an analysis input;
- extension read-set is an analysis dependency;
- extension validation digest is a cache input;
- extension precision ceiling affects downstream facts;
- extension facts need provenance and merge status;
- extension failures should not poison native caches;
- extension output can be quarantined independently.

This is a fundamental shift from old black-box static analysis.

### Unknowns Are Cacheable Outputs

An unknown call target, missing framework model, unsupported language construct,
or setup gap is a fact. It should have a stable key, provenance, and cache
digest. If an extension later resolves it, the cache should show:

```text
unknown fact removed
extension fact added
downstream call/data-flow/evidence facts recomputed
diagnostic precision changed
```

That is the workflow agents need.

### Cache Keys Must Include Precision And Validation

Two fact sets with identical payloads but different validation status are not
always equivalent.

Example:

```text
call edge A -> B emitted by heuristic resolver
call edge A -> B emitted by fixture-validated framework extension
```

The payload edge is the same, but diagnostics, evidence, and confidence may
change. Consumers should decide whether provenance-only changes invalidate them,
but the cache must be able to represent the difference.

## Risks

| Risk | Mitigation |
|---|---|
| Cache keys become too broad and invalidate everything. | Use shape digests and projection keys. Measure hit/miss by layer. |
| Cache keys become too narrow and reuse stale facts. | Fail closed, record dependency traces, require extension read declarations. |
| Query engine becomes public API too early. | Keep all query internals `pub(crate)` and expose typed SDK views only. |
| Persistent cache is hard to debug. | Emit explainable cache manifests and `polint debug cache` later. |
| Extension model changes invalidate too much. | Track extension output layers and declared read-set precisely. |
| Relation engine is built before it is needed. | Start with SCC closure recompute; benchmark before adding differential machinery. |
| Official language tool output is nondeterministic. | Include invocation, version, environment policy, and output digest. |

## Final Technical Recommendation

Implement a conservative batch incremental substrate now:

```text
InputSnapshot
LayerKey
LayerCacheManifest
DependencyIndex
ChangeSet
InvalidationPlan
CacheStats
```

Then implement demand queries for expensive views:

```text
QueryKey
QueryTrace
InRunMemo
SelectedPersistentQueryCache
SummarySccCache
DiagnosticCache
```

Only then add daemon red-green mode and relation/differential sub-engines.

This path avoids building into a corner because it does not commit the whole
engine to one framework, but it preserves the strongest ideas from the state of
the art.

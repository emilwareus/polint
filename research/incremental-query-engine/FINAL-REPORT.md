# Final Report: Incremental Query Engine And Caching

## Executive Decision

Build a native layered incremental engine. Do not adopt Salsa, Datalog,
Differential Dataflow, Skyframe, or DICE as the first hard runtime dependency.

The implementation path should be:

```text
InputSnapshot
  -> per-layer content-addressed cache
  -> DependencyIndex
  -> InvalidationPlan
  -> demand QueryEngine for expensive facts
  -> SummaryCache with SCC/equality pruning
  -> extension-aware cache validation and quarantine
  -> later daemon/watch red-green validation
  -> later relation/differential sub-engine for high-volume recursive facts
```

This is the right fit for polint because the engine has unusual constraints:

- multi-language facts;
- official language-tool inputs where useful;
- native Rust implementation preference;
- repo-local Rust extensions written by agents;
- provenance, precision, and validation status on facts;
- public SDK stability separate from internal query mechanics;
- default mode and agent-extended mode;
- expensive global analyses such as call graphs, data flow, summaries, alias
  queries, slicing, and evidence paths.

Salsa is the best conceptual reference for demand queries, red-green
verification, durability, and backdating. Skyframe and DICE are the best
references for dependency graphs, equality pruning, projections, transactions,
and invalidation paths. TypeScript, gopls, Pyright, Pyrefly, and rust-analyzer
show the language-server/analyzer reality: most useful incrementality is a
careful mix of file snapshots, shape digests, reverse dependency graphs,
module/package lifecycle inputs, and selective recomputation.

## Why This Research Matters Now

The first kernel design already recommends typed fact layers, provider
scheduling, provenance, precision, validation, extension merges, and cache keys.
That is enough to avoid a pile of unrelated analyzers. It is not enough to scale
the engine once polint adds:

- direct and indirect call graphs;
- local and interprocedural data flow;
- summary fixpoints;
- alias and points-to queries;
- abstract interpretation domains;
- evidence paths and slices;
- agent-authored Rust model providers;
- rule SDK queries that may request expensive views repeatedly during editing.

Without a real incremental model, every agent edit becomes either:

- a full recompute, which kills interactive use; or
- a stale-cache risk, which kills trust.

For polint, stale analysis is worse than slow analysis because the product is
about turning codebase-specific knowledge into executable facts. If extension
code changes and the engine silently reuses old call edges or summaries, the
analysis becomes untrustworthy.

## Core Insight

Incrementality has to be a semantic contract, not just an optimization.

Every cached result must answer:

- Which source/config/lifecycle/toolchain/rule/extension inputs did I depend on?
- Which fact layers and query results did I read?
- Which provider version and output schema produced me?
- Which precision and validation status did I claim?
- Which extension facts or model files influenced me?
- Which downstream layers, queries, summaries, and diagnostics depend on me?
- If a dependency changed, can I verify reuse, backdate equality, recompute only
  my SCC, or must I drop/quarantine the result?

If the engine cannot answer those questions, the cache is only a best-effort
key-value store. That is not strong enough for agent-extensible static analysis.

## State Of The Art Patterns

The research converges on seven patterns.

### 1. Snapshot The World

Modern analyzers do not reason directly from a mutable filesystem. They create a
snapshot:

```text
source files
open-buffer overlays
config files
language lifecycle files
package manifests and lockfiles
toolchain versions
rule code and options
extension code and declared inputs
generated model files
provider versions and schemas
environment inputs allowed by policy
```

gopls snapshots, rust-analyzer VFS changes, Pyright file info, TypeScript
builder state, Pyrefly transactions, Skyframe versions, and DICE transactions
all enforce the same principle: the computation sees a coherent input view.

### 2. Separate Text Digests From Shape Digests

TypeScript's shape signatures and rust-analyzer's ItemTree are the most
important analyzer-specific idea. A function body edit often should not
invalidate every dependent module, symbol, or call graph fact.

Polint needs multiple shape levels:

```text
text digest
syntax shape digest
import/export digest
public symbol signature digest
framework boundary digest
summary/effect digest
diagnostic digest
```

This is more work than one file hash, but it is the difference between useful
interactive analysis and whole-repo churn.

### 3. Keep Reverse Dependency Edges

Bazel Skyframe is blunt: an edgeless graph cannot do precise incrementality.
Salsa, DICE, TypeScript, Pyright, gopls, and Pyrefly all keep some form of
dependency/reverse-dependency data.

Polint should keep a dependency index for:

- input to layer;
- layer to layer;
- layer to query;
- query to query;
- summary to caller summary;
- extension/model to affected facts;
- rule option to diagnostics;
- diagnostics to evidence bundles.

The dependency index should be persisted for cacheable layers and held in memory
for run-local demand queries.

### 4. Use Equality To Prune Downstream Work

Salsa backdating, Skyframe equal-value pruning, DICE key equality, and TypeScript
shape-signature checks all use the same trick: a dirty input does not always
mean the output changed.

Example:

```text
edit function body
  -> file text digest changed
  -> public API digest unchanged
  -> import digest unchanged
  -> symbol export layer can be reused
  -> dependent modules do not need symbol recompute
```

For summaries:

```text
edit function body
  -> local summary recomputed
  -> summary digest equal
  -> callers stay green
```

Equality pruning is mandatory for summary SCCs and interprocedural analyses.

### 5. Demand Queries Should Wrap Expensive Global Facts

Adapton, Salsa, demanded abstract interpretation, and DICE all show why demand
matters. Not every rule needs whole-program data flow or every possible alias
query.

Polint should compute cheap structural layers eagerly and expensive precision
views on demand:

Eager by default:

- file discovery and input snapshots;
- source digests and cheap shape digests;
- parsing for selected files;
- imports and module/package topology;
- baseline symbols/references;
- capability planning;
- diagnostics for setup gaps.

Demand-driven:

- CFG details beyond direct local lowering;
- control dependence;
- local data-flow paths;
- call graph precision tiers above direct calls;
- summary domains;
- alias/points-to;
- abstract interpretation domains;
- evidence paths and slices;
- framework-specific expansion;
- extension-produced expensive views.

### 6. Recursive Relation Analyses Need A Separate Plan

Call reachability, data flow, alias propagation, summary fixpoints, and some
framework dispatch models are recursive. There are three plausible approaches:

| Approach | Strength | Weakness |
|---|---|---|
| Recompute affected SCC closure | Simple, deterministic, good first step | Can over-recompute large SCCs. |
| IncIDFA-like incremental SCC updates | Better for monotone iterative data-flow | More complex; applies to specific analysis shape. |
| Differential/Souffle-like relation backend | Powerful for large recursive relations | More memory and implementation complexity. |

Recommendation: start with affected SCC closure plus equality/backdating. Add
IncIDFA-style updates only for monotone IDFA domains after the baseline is
measured. Add a relation/differential backend only when benchmarks show the
relation volume justifies it.

### 7. Extension Inputs Must Be First-Class Cache Inputs

This is where polint differs from classic tools. An agent can write Rust code
that alters the analysis engine. That means cache correctness must include:

- extension crate digest;
- extension API version;
- declared extension-provided fact families;
- extension precision ceiling;
- validation fixture digest;
- generated model file digests;
- files read by extension providers;
- extension provider config and options;
- extension output schema;
- extension validation status.

If an extension reads undeclared files or environment state, dependent caches
should be quarantined or broadly invalidated. Untracked state is a correctness
bug, not an implementation detail.

## Tool Findings

| Tool | What to copy | What to avoid |
|---|---|---|
| Salsa | Revisions, dependency tracing, red-green verification, durability, backdating. | Making the whole polint kernel a Salsa DB before layer/cache/extension boundaries are stable. |
| rust-analyzer | Stable file IDs, VFS changes, ItemTree/body-shape separation, cancellation. | Assuming language-server-style daemon state is required for v1. |
| gopls | Snapshots, parse cache, lifecycle-aware package metadata invalidation, analysis cache keys. | Hidden Go-specific lifecycle special cases outside the shared kernel. |
| TypeScript | Text version versus declaration signature, affected-file propagation, `.tsbuildinfo` style persisted metadata. | TS-only assumptions about declaration emit as shape. |
| Pyright | Import graph dirtying, conservative resolver invalidation, library change batching. | Over-invalidating all analysis for every ordinary edit. |
| Pyrefly | Module-level epochs, transactions, require levels, retained state. | Coarse-only incrementality once polint has expensive interprocedural queries. |
| Pyre/Pysa | Saved-state cache, interprocedural analysis cache contents, config/source invalidation. | Giant shared-memory cache as the first architecture. |
| Skyframe | Reverse deps, dirty node lifecycle, equality pruning, edge-retention policy. | Build-system-specific semantics as a direct model for facts. |
| DICE | Projection keys, injected values, transactions, invalidation paths. | Exposing query internals as public SDK concepts. |
| Souffle | Semi-naive deltas, relation indexes, provenance, fixpoint loops. | Datalog as the entire product or first public extension model. |
| Ty | Salsa in a modern Rust Python analyzer, stable project handles, file-set walking. | Untracked project state and incomplete persistent cache identity. |

## Complexity And Accuracy

Incremental systems do not change worst-case cost. They improve common-case cost
when edits are local, outputs are equal, or demand is narrow.

| Operation | Expected cost model |
|---|---|
| Layer cache hit | `O(number of declared input digests + manifest validation)` |
| Invalidation planning | `O(changed inputs + reverse dependency closure)` |
| Red-green hot query hit | Usually `O(1)` plus current revision checks. |
| Red-green cold verification | `O(previous dependency edges visited)` before possible recompute. |
| Query recompute | Provider-specific. Parsing is file-local, call/data-flow/summaries can be graph-wide. |
| Summary SCC update, first version | `O(affected SCC closure * transfer cost)` with equality pruning. |
| Semi-naive relation evaluation | `O(iterations * indexed delta join cost)`, highly data-dependent. |
| Differential relation update | Often proportional to deltas and maintained traces, but memory can be high. |

Accuracy risks:

- missing dependency edges cause unsound cache reuse;
- over-broad invalidation causes poor performance but preserves correctness;
- extension undeclared inputs create stale facts;
- equality checks must include precision, validation, provenance-relevant status,
  not just fact payloads;
- language lifecycle changes can invalidate facts even when source text is
  unchanged;
- official tool outputs must include invocation and tool version digests;
- diagnostics must not be reused if evidence paths or uncertainty status changed.

## Recommended Architecture

Add an internal incremental module:

```text
crates/polint/src/analysis/incremental/
  mod.rs
  input_snapshot.rs
  digest.rs
  keys.rs
  dependency_index.rs
  change_set.rs
  invalidation.rs
  layer_cache.rs
  query.rs
  query_trace.rs
  summary_cache.rs
  diagnostic_cache.rs
  extension_inputs.rs
  stats.rs
```

The public SDK should not see this directly. Rules should continue to request
typed views. The kernel decides whether a view comes from fresh computation,
verified cache reuse, an extension provider, or a demand query.

## Cache Key Model

Use separate key types:

```text
InputKey
LayerKey
QueryKey
SummaryKey
DiagnosticKey
ExtensionKey
ToolInvocationKey
```

Do not use one global analysis hash. It will either over-invalidate everything
or miss important dependencies.

Minimum `LayerKey` inputs:

```text
provider id and version
output schema version
language id
normalized provider parameters
input source or shape digests
language lifecycle digest
config digest
toolchain digest
dependency layer digests
extension/model digests when consumed
precision tier and budget when relevant
polint internal schema version
```

Minimum `DiagnosticKey` inputs:

```text
rule id and rule version
rule code digest
rule option digest
requested fact view digests
diagnostic schema version
evidence query digest when evidence is emitted
SDK/protocol version
```

## Invalidation Actions

Every affected node should get one of five actions:

| Action | Meaning |
|---|---|
| `Reuse` | Declared dependencies unchanged; result can be loaded. |
| `Verify` | A dependency changed, but shape/equality checks may prove the result green. |
| `Recompute` | The result is invalid or verification failed. |
| `Drop` | Inputs/schema/provider changed so old output is unusable. |
| `Quarantine` | Extension/model/input dependency is untrusted, undeclared, failed, or validation status changed. |

Quarantine is important. It prevents an invalid extension cache from looking
like an ordinary cache miss.

## Implementation Sequence

1. **Cache vocabulary and instrumentation.** Add key structs, digest helpers,
   cache stats, and provider output metadata before optimizing behavior.
2. **InputSnapshot and layer cache.** Persist syntax/import/module/symbol layer
   manifests with dependency digests.
3. **DependencyIndex and invalidation planner.** Record input-to-layer and
   layer-to-layer dependencies. Classify changes.
4. **Demand QueryEngine.** Add in-run memoization and trace recording for
   expensive views. Persist only selected query results.
5. **Summary SCC cache.** Store function summaries by summary key, recompute
   affected SCC closures, and backdate equal summaries.
6. **Extension-aware validation.** Track extension crates, model files,
   declared reads, validation fixtures, precision ceilings, and quarantine.
7. **Watch/daemon red-green mode.** Add revision-based red-green verification
   after the batch cache is correct.
8. **Relation/fixpoint sub-engine.** Add semi-naive or differential-inspired
   backend only after call/data-flow benchmarks justify it.

## Decision

The next implementation step should not be a generic "cache everything" pass.
It should be the first native incremental substrate:

```text
InputSnapshot + LayerKey + LayerCacheManifest + DependencyIndex + InvalidationPlan
```

That gives polint a correct baseline for syntax, module, symbol, bootstrap MIR,
direct calls, P0 domains, and direct summaries. Then the demand query engine can
be added before expensive global call graph/data-flow/alias/evidence work.

This keeps the engine flexible, native, measurable, and extension-safe without
building into a corner.

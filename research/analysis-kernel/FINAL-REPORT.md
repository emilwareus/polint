# Final Report: Analysis Kernel

## Executive Decision

Build an internal hybrid kernel.

```text
Rule/extension demand
  -> provider DAG
  -> typed fact layers
  -> validation/provenance gates
  -> deterministic merge
  -> layer cache
  -> typed SDK views
```

Use a relation/fixpoint sub-engine for recursive families such as reachability, call graph expansion, summary propagation, and data flow. Keep ordinary syntax/module/symbol/metric facts in efficient Rust-native typed storage.

Do not make Salsa, Datalog, CodeQL-style QL, Joern-style CPG, or a graph database the first core abstraction. Each is too large or too opinionated for polint's current product shape. The kernel should be designed so parts can later be backed by a query engine, but the first implementation should be explicit, typed, deterministic, and debuggable.

## The Critical Insight

The kernel is where polint's product thesis becomes real.

Old static analysis assumes the analyzer must be a mostly black-box system that fits every codebase. polint assumes AI agents can inspect a specific repo and write Rust extensions that improve analysis accuracy. That means the kernel must support two equally important modes:

1. **Default mode:** native facts, useful approximations, explicit unknowns.
2. **Agent-extended mode:** validated repo-local extensions add framework semantics, entrypoints, summaries, call edges, sources/sinks, barriers, type hints, generated-code mappings, and effect facts.

If those extension facts merge invisibly, the engine becomes untrustworthy. If they cannot merge at all, the product loses its differentiator.

The kernel must therefore make every nontrivial fact answer these questions:

- What does the fact say?
- Which entity or source span does it apply to?
- Who emitted it?
- Which input facts, files, config, and extension code does it depend on?
- Is it exact, conservative, under-approximate, heuristic, lossy, user-asserted, or unknown?
- Was it merely schema-valid, resolved, fixture-validated, or rejected?
- What cache key invalidates it?
- What downstream facts/rules depended on it?

That is the analysis kernel.

## What Current polint Already Gets Right

polint already has several good primitives:

- deterministic file discovery and source storage with `Arc<str>`;
- a monolithic internal `AnalysisDb` that stores typed fact families;
- rule capability planning through `AnalysisPlan`;
- typed SDK fact views rather than broad public `AnalysisDb` access;
- parser cache keys with source/config/rule/plan/schema/version digests;
- language adapters that parse in parallel and restore facts deterministically;
- derived module graph, symbol graph, and metrics providers;
- capability diagnostics that block rules instead of giving them fake facts.

The current weakness is not correctness of this simple pipeline. The weakness is that the pipeline is hardcoded and the cache/fact model will not scale to extension-provided entrypoints, call graphs, data flow, effects, and incremental invalidation.

## What Must Change

### 1. Split "facts exist" from "facts are supported"

Today capabilities are mostly planned up front and then support is adjusted by module/symbol providers. In the kernel, support should be provider output:

```text
ProviderResult:
    produced_layers
    diagnostics
    capability_support_delta
    layer_output_digest
    validation_summary
```

The final support view should be the result of all required providers and validation gates.

### 2. Add fact layers

`AnalysisDb` currently acts like a single bag of facts. That is fine in memory for v1, but the kernel needs a sidecar layer model:

```text
source files
  -> native syntax layer
  -> resolved imports layer
  -> module graph layer
  -> symbol/reference layer
  -> extension entrypoints layer
  -> call graph layer
  -> data-flow/effects layer
  -> diagnostics
```

Layering gives us scheduling, cache keys, provenance, invalidation, and extension diff reports.

### 3. Separate run-local IDs from stable keys

Dense IDs are good inside one run. They are not enough for persistent caches, extension facts, or cross-language facts.

Use both:

- run-local IDs for memory and joins;
- stable keys for cacheable, extension-visible, and cross-run facts.

Kythe's VName and SCIP's symbol/occurrence schema show the lesson: cross-language indexes need stable names and source anchors, not just in-memory IDs. Kythe also warns that build configuration can be part of anchor identity. That directly applies to Go build tags, TS config, generated code, and future Java/Python environments.

### 4. Use sidecar provenance

Do not inflate every fact struct with metadata fields yet. Add a side table:

```text
FactRef(family, run_id) -> FactMeta {
    stable_key,
    layer_id,
    provenance_id,
    precision,
    confidence,
    validation,
}
```

Existing facts can get default native metadata when inserted. Extensions and future analyses can provide richer metadata.

### 5. Add validation before merge

Extension facts must not write directly into `AnalysisDb`.

Flow:

```text
extension emits facts
  -> schema validation
  -> referential validation
  -> selector/span/access-path validation
  -> precision ceiling check
  -> deterministic normalization
  -> merge
```

Rejected facts become diagnostics and do not affect rules.

### 6. Make unknowns first-class

For call graphs and data flow, silent absence is poison. A missing edge can mean:

- no call exists;
- call exists but cannot be resolved;
- provider skipped;
- setup missing;
- extension failed;
- budget exceeded;
- language feature unsupported.

These are different facts. The kernel needs explicit unknown/unresolved/setup facts so agents can improve them.

## Scheduling Recommendation

Use a demand-driven provider DAG at the fact-family level.

Rules and extensions declare required fact views. The kernel computes the transitive provider plan:

```text
requested rule views
  + extension inputs
  + extension outputs that satisfy requested views
  -> required fact families
  -> provider DAG
  -> topological batches
  -> recursive SCC/fixpoint groups where needed
```

Run cheap per-file providers in deterministic parallel batches. Run package/module/whole-repo providers after their inputs. For recursive relation families, use an explicit SCC/fixpoint executor.

This is more practical than pure query-level demand for the first version and avoids recomputing unused expensive facts.

## Cache And Invalidation Recommendation

Current per-file cache keys include `rule_hash` and `plan_hash`. That is safe but over-invalidates parser facts. The kernel should split caches by layer:

```text
Go syntax layer key =
    source digest
  + Go lifecycle digest
  + parser provider digest
  + syntax schema
  + polint version

Module graph layer key =
    import facts output digest
  + language lifecycle digest
  + resolver provider digest
  + module graph schema

Rule diagnostics key =
    requested fact layer digests
  + rule digest
  + rule options digest
  + SDK/protocol version
```

This gives precise invalidation:

- body-only source edit should not invalidate module graph if imports/export shape did not change;
- rule edit should not invalidate parser facts;
- extension edit should invalidate dependent facts and rules, not all syntax;
- config/lifecycle edit invalidates only affected language/provider layers;
- rejected extension facts should not poison valid caches.

## Relation/Fixpoint Recommendation

Do not expose Datalog to rule authors first. But internally, recursive analyses should be represented like relations:

```text
Relation[CallEdge]
Relation[ReachableFunction]
Relation[DataFlowStep]
Relation[Summary]
Relation[Effect]
```

Use:

- relation-local indexes chosen by query patterns;
- worklists and delta sets;
- SCC/fixpoint scheduling;
- monotonic merge for additive facts;
- iteration budgets with explicit `BudgetExceeded` capability status;
- provenance compression for paths.

This copies Souffle/CodeQL/FlowLog's strengths without committing to a public Datalog language.

## Extension Merge Recommendation

Default merge: append-only union after validation.

Do not let normal extensions delete native facts. Do not use last-writer-wins. Do not let generated facts suppress exact native facts.

Supported first:

- additive entrypoints;
- additive call edges;
- additive source/sink/barrier/sanitizer declarations;
- additive data-flow summaries;
- additive effect summaries;
- evidence annotations;
- "resolved alternative for unknown X" facts.

Delayed:

- replacement of native facts;
- broad suppressions;
- global trust overrides;
- negative facts that kill flows;
- extension-driven schema mutation.

Sanitizers/barriers/suppressions are higher risk than sources and call edges because they can hide real findings. Require stricter validation and fixture coverage.

## Implementation Shape

First slice:

```rust
pub(crate) mod analysis_kernel;

struct AnalysisKernel;
struct KernelInput<'a>;
struct KernelOutput;
struct ProviderManifest;
struct ProviderResult;
struct LayerManifest;
struct LayerId;
struct FactFamily;
struct CapabilitySupportDelta;
```

Move the current runner phase sequence into `AnalysisKernel::run` while preserving behavior:

```text
load files
  -> go syntax provider
  -> ts syntax provider
  -> module graph provider
  -> symbol graph provider
  -> metrics provider
  -> rules
```

After that, add layer manifests, provenance side tables, validation, and layer cache keys.

## Decision Summary

| Decision | Recommendation |
|---|---|
| Kernel style | Hybrid provider DAG plus relation/fixpoint sub-engine. |
| Public API | Keep typed SDK views; do not expose kernel or mutable graph. |
| Storage | Keep `AnalysisDb` initially, add sidecar metadata/layers, evolve to `FactStore` later. |
| Scheduling | Demand at fact-family/provider level, not query-level first. |
| Recursive analyses | Internal relation engine with SCC/fixpoint and delta sets. |
| Provenance | Sidecar fact metadata and evidence tables. |
| Precision | Common precision labels plus family-specific details. |
| Extension merge | Validated append-only union first. |
| Cache | Layer-specific content-addressed keys. |
| Invalidation | Batch digests first, finer dependency graph later. |
| Salsa | Copy concepts; do not adopt as hard dependency yet. |
| Datalog/CodeQL | Copy relation/fixpoint/product model; do not expose public QL/Datalog yet. |

## Bottom Line

The next implementation should not be "build call graph" or "build data flow." It should be "build the analysis kernel that makes call graph and data flow safe to add."

The kernel should make polint capable of this loop:

```text
native analysis exposes uncertainty
  -> agent writes a repo-local extension
  -> extension emits typed facts
  -> kernel validates and merges them
  -> downstream analyses improve
  -> reports show delta, precision, provenance, and cache impact
```

That loop is the path to max capability.


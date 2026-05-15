# Recommended Implementation: Analysis Kernel

## Goal

Create an internal kernel that lets polint grow from parser facts and rule execution into a multi-language, agent-extensible static-analysis engine without losing determinism, provenance, precision accounting, cache correctness, or public API discipline.

The goal is not to expose a new public graph/query API. The public rule-author surface should remain typed SDK views.

## Target Architecture

```text
polint check
  -> load config and files
  -> collect rule and extension demands
  -> build kernel plan
  -> execute provider DAG
  -> validate provider outputs
  -> merge fact layers
  -> compute final capability support
  -> run rules against typed SDK views
```

Internal modules:

```text
crates/polint/src/analysis_kernel/
  mod.rs
  family.rs
  layer.rs
  provider.rs
  scheduler.rs
  cache.rs
  provenance.rs
  validation.rs
  merge.rs
  stats.rs
```

Keep all of this `pub(crate)` unless a specific SDK view is intentionally promoted.

## Phase 1: Move Orchestration, Preserve Behavior

Add:

```rust
pub(crate) struct AnalysisKernel;

pub(crate) struct KernelInput<'a> {
    pub(crate) loaded: &'a LoadedConfig,
    pub(crate) cache: &'a Cache,
    pub(crate) config_digest: &'a str,
    pub(crate) rule_digest: &'a str,
    pub(crate) plan: &'a AnalysisPlan,
    pub(crate) parallel: bool,
}

pub(crate) struct KernelOutput {
    pub(crate) db: AnalysisDb,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: CapabilitySupportView,
}
```

Move this existing order from `runner::analyze_and_run` into the kernel:

```text
load_analysis_files
go::analyze_with_plan_options
ts::analyze_with_plan_options
module_graph::derive_requested_module_graph
symbol_graph::derive_requested_symbols
metrics::derive_requested_metrics
```

No behavior change. The purpose is to establish the ownership boundary.

Acceptance:

- existing tests pass;
- rule behavior unchanged;
- no new public API;
- runner just calls kernel then runs rules.

## Phase 2: Provider Manifests

Wrap current passes in internal provider descriptors:

```rust
pub(crate) struct ProviderManifest {
    pub(crate) id: ProviderId,
    pub(crate) kind: ProviderKind,
    pub(crate) inputs: Vec<FactFamily>,
    pub(crate) outputs: Vec<FactFamily>,
    pub(crate) language_scope: LanguageScope,
    pub(crate) cache_policy: CachePolicy,
    pub(crate) schema_versions: BTreeMap<FactFamily, SchemaVersion>,
    pub(crate) precision_ceiling: Precision,
}
```

Initial providers:

| Provider | Inputs | Outputs |
|---|---|---|
| `polint.source` | config/file discovery | files |
| `polint.go.syntax` | files | packages, functions, imports, tests, branches, literals |
| `polint.ts.syntax` | files | functions, imports, TS components/classes, literals, JSX |
| `polint.module_graph` | files, imports, packages | resolved imports, module nodes, module edges |
| `polint.symbol_graph` | files, imports, module graph, syntax | symbols, definitions, references |
| `polint.metrics` | files, functions, branches | file/function/complexity metrics |

Provider manifests should initially document dependencies; the scheduler can still execute the old order.

Acceptance:

- debug output can print planned providers and skipped providers;
- tests assert provider dependencies for existing facts;
- capability support comes from provider result deltas.

## Phase 3: Layer Manifests And Layer Cache Keys

Introduce:

```rust
pub(crate) struct LayerManifest {
    pub(crate) id: LayerId,
    pub(crate) provider_id: ProviderId,
    pub(crate) input_layer_digests: Vec<LayerDigest>,
    pub(crate) parameters_digest: String,
    pub(crate) output_families: Vec<FactFamily>,
    pub(crate) output_digest: Option<LayerDigest>,
}
```

Add cache keys per provider/layer:

```text
LayerCacheKey =
    provider_id
  + provider_version
  + provider_schema_version
  + output_family_schema_versions
  + input_layer_digests
  + config/lifecycle digest
  + provider parameters digest
  + polint version
```

Do not use `rule_digest` for syntax cache keys unless the rule options affect parsing. Keep current keys for compatibility at first, but introduce new layer-key code and migrate one provider at a time.

Acceptance:

- syntax cache is not invalidated by an unrelated rule edit;
- module graph cache invalidates when imports or lifecycle config change;
- cache debug stats report per-layer hits/misses.

## Phase 4: Fact Metadata Side Tables

Add side tables to `AnalysisDb` or a nested `FactMetaStore`:

```rust
pub(crate) struct FactMeta {
    pub(crate) stable_key: StableKey,
    pub(crate) layer_id: LayerId,
    pub(crate) provenance_id: ProvenanceId,
    pub(crate) precision: Precision,
    pub(crate) confidence: Confidence,
    pub(crate) validation: ValidationStatus,
}

pub(crate) struct FactRef {
    pub(crate) family: FactFamily,
    pub(crate) run_id: u64,
}
```

Existing facts can get default metadata:

```text
origin = native
provider = current provider
precision = exact for pure syntax, conservative/heuristic where appropriate
confidence = high unless setup degraded
validation = NativeTrusted
```

Acceptance:

- debug JSON can show provenance for at least files, imports, symbols, and references;
- missing metadata is a test failure for new kernel providers;
- simple rules remain ergonomic.

## Phase 5: Validation And Merge Gates

Add `ProviderOutput`:

```rust
pub(crate) struct ProviderOutput {
    pub(crate) layers: Vec<LayerOutput>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) support: Vec<CapabilitySupport>,
    pub(crate) stats: ProviderStats,
}
```

Every output goes through:

```text
schema validation
referential validation
span bounds validation
stable-key uniqueness validation
precision-ceiling validation
merge conflict validation
deterministic normalization
```

Native providers can have trusted fast paths, but tests should run stricter invariants.

Acceptance:

- duplicate conflicting stable keys fail deterministically;
- invalid extension-like outputs are rejected before merge;
- capability diagnostics name the failing provider/layer.

## Phase 6: Demand Scheduler

Replace hardcoded order with a scheduler:

```text
requested capabilities
  -> required fact families
  -> provider selection
  -> dependency closure
  -> topological batches
```

Rules:

- a provider can run only after required input families are supported;
- if setup is missing, dependent providers become blocked;
- a provider can be skipped if none of its outputs are demanded;
- output order is deterministic;
- per-file providers can run in parallel;
- whole-repo providers run after merges.

Acceptance:

- requesting only syntax does not run module/symbol graph providers;
- requesting references runs syntax, module graph, and symbol graph;
- blocked provider produces explicit support status.

## Phase 7: Extension Providers

Use the agent extension research, but route all extension outputs through the kernel:

```text
.polint/extensions/<name>
  -> handshake manifest
  -> declared inputs/outputs
  -> run provider
  -> validate facts
  -> merge as extension layer
```

First fact family should be `Entrypoints<'_>`.

Why entrypoints first:

- high value for call graph and data flow;
- narrow schema;
- clear validation against files/symbols;
- easy fixtures;
- good demonstration of default-vs-extended delta.

Acceptance:

- extension emits entrypoints;
- rule reads `Entrypoints<'_>`;
- invalid entrypoint facts are rejected;
- `polint extension diff` shows added facts and provenance;
- cache invalidates on extension source/options changes.

## Phase 8: Relation/Fixpoint Sub-Engine

Only after provider/layer/provenance/cache foundations exist, add relation support for recursive families.

Start internal:

```rust
Relation<T>
Delta<T>
FixpointGroup
RelationIndex
```

First uses:

- module reachability;
- call graph closure;
- simple summary propagation;
- data-flow local-to-interprocedural steps.

Do not expose Datalog. Do expose stats:

```text
relation rows
delta rows per iteration
iterations
max fanout
budget exceeded
precision knobs
```

Acceptance:

- deterministic fixed point;
- budget failures produce `BudgetExceeded`;
- path/evidence can be reconstructed for selected diagnostics.

## API Discipline

Do not expose these in `sdk::prelude`:

- `AnalysisKernel`;
- `Provider`;
- `Layer`;
- `FactStore`;
- mutable `AnalysisDb`;
- relation engine internals.

Expose only stable fact views:

```rust
Entrypoints<'_>
Calls<'_>
CallGraph<'_>
DataFlow<'_>
Effects<'_>
```

Each view can expose provenance/evidence methods for advanced users without making the kernel public.

## First Vertical Slice

Recommended first implementation milestone:

```text
analysis_kernel module
  + current provider wrappers
  + provider DAG debug report
  + layer cache key type
  + provenance side table for new facts
  + validation helpers
  + Entrypoints extension provider
  + one external temp-repo test
```

This proves the architecture without waiting for full call graph/data flow.

## Anti-Goals

- Do not rewrite `AnalysisDb` first.
- Do not expose a public graph query DSL.
- Do not adopt Salsa as a hard dependency in the first slice.
- Do not implement a full Datalog engine before fact families stabilize.
- Do not let extension facts bypass validation.
- Do not let normal extensions delete native facts.
- Do not cache parser facts with rule hashes long term.
- Do not run rules with placeholder facts for failed capabilities.


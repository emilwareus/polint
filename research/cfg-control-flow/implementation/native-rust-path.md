# Native Rust Implementation Path

## First Principle

The CFG engine should be native Rust and polint-owned. External projects are references and differential oracles, not runtime dependencies.

## Internal Module Boundaries

```text
cfg::facts
  stable data structs and internal IDs

cfg::builder
  language-independent block builder, control-context stack, edge emission helpers

cfg::providers
  Go and TS/JS provider implementations

cfg::analysis
  reachability, dominators, postdominators, control dependence, loops, invariants

cfg::extension
  validated extension sinks and merge policies

cfg::sdk
  adapter from internal facts to future public `Cfg<'_>` view
```

## Data Structures

Use arenas plus sorted adjacency lists:

```rust
pub(crate) struct CfgStore {
    functions: Vec<CfgFunctionFact>,
    nodes: Vec<CfgNodeFact>,
    blocks: Vec<BasicBlockFact>,
    edges: Vec<CfgEdgeFact>,
    succs_by_node: Vec<Vec<CfgEdgeId>>,
    preds_by_node: Vec<Vec<CfgEdgeId>>,
    succs_by_block: Vec<Vec<CfgEdgeId>>,
    preds_by_block: Vec<Vec<CfgEdgeId>>,
}
```

Rationale:

- deterministic IDs;
- fast iteration;
- easy snapshot output;
- no public dependency on `petgraph`;
- cache serialization is straightforward.

`petgraph` may still be used internally for algorithms, but do not let it shape public facts.

## Provider Contract

```rust
pub(crate) trait CfgProvider {
    fn provider_id(&self) -> ProviderId;
    fn build_cfgs(&self, input: CfgProviderInput<'_>, sink: &mut CfgSink) -> CfgProviderResult;
}
```

Input includes:

- parsed source facts;
- semantic index facts;
- module/source-set context;
- language lifecycle config;
- requested graph views/capabilities;
- extension summaries such as no-return facts.

Output goes only through `CfgSink`, which validates fact shape before insertion.

## Validation Pipeline

```text
provider emits facts
  -> structural validation
  -> source/span validation
  -> precision validation
  -> extension merge validation
  -> deterministic sort/dedupe
  -> derived analyses
```

Validation failures should become internal errors or capability diagnostics, not panics.

## Go Provider Path

Initial source-level provider:

- tree-sitter Go body traversal;
- language-owned builder;
- control-context stack for loops/switch/select/goto/defer;
- no exact semantic claims for panic/recover until semantic package loading exists.

Semantic provider later:

- package loading/type facts;
- Go SSA-grade lowering;
- no-return summaries;
- compare against `go/ssa` fixture output.

## TS/JS Provider Path

Initial provider:

- Oxc AST/semantic traversal;
- optionally borrow Oxc CFG internally if stable enough;
- translate to polint facts;
- no public Oxc IDs.

Construct priority:

1. functions/module bodies;
2. branches/loops;
3. return/throw/break/continue;
4. short-circuit/optional/nullish expressions;
5. try/catch/finally;
6. await/yield markers;
7. class static blocks/top-level await.

## Extension Path

Additive extension sinks first:

- no-return summaries;
- guard/assert summaries;
- cleanup summaries;
- synthetic/generated nodes;
- synthetic edges in extension overlay view.

Merge rules:

- extension facts are separate layer by default;
- exact native facts are not removed;
- replacement requires explicit operation, validation, and retained evidence;
- cache keys include extension digest and declared dependencies.

## Public SDK Promotion Criteria

Promote `Cfg<'_>` only after:

- Go and TS/JS fixtures cover the first construct matrix;
- graph invariants are stable;
- docs/facts page lists supported and unsupported constructs;
- capability diagnostics are emitted for unsupported requested CFG facts;
- cache keys include CFG inputs;
- at least one temp-repo style rule consumes `Cfg<'_>` through public SDK only.

## Suggested SDK API

```rust
pub struct Cfg<'a> { /* private */ }

impl<'a> Cfg<'a> {
    pub fn functions(&self) -> impl Iterator<Item = CfgFunction<'a>>;
    pub fn graph(&self, function: CfgFunction<'a>, view: CfgView) -> RuleResult<CfgGraph<'a>>;
}

pub struct CfgGraph<'a> { /* private */ }

impl<'a> CfgGraph<'a> {
    pub fn entry(&self) -> CfgNode<'a>;
    pub fn normal_exit(&self) -> CfgNode<'a>;
    pub fn nodes(&self) -> impl Iterator<Item = CfgNode<'a>>;
    pub fn blocks(&self) -> impl Iterator<Item = CfgBlock<'a>>;
    pub fn edges(&self) -> impl Iterator<Item = CfgEdge<'a>>;
    pub fn successors(&self, node: CfgNode<'a>) -> impl Iterator<Item = CfgEdge<'a>>;
    pub fn is_reachable(&self, node: CfgNode<'a>) -> bool;
}
```

Keep control dependence as a separate typed view:

```rust
pub struct ControlDependence<'a> { /* private */ }
```

## Acceptance Tests

Minimum before public SDK:

- deterministic snapshots under parallel execution;
- panic isolation if a provider panics;
- malformed file produces diagnostics, not crash;
- unsupported control-flow constructs produce capability diagnostics;
- extension-added no-return summary changes reachability in extension view only;
- postdominance handles multiple exits through artificial exit.

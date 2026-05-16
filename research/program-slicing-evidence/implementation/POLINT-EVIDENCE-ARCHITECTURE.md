# Polint Evidence Architecture

This note maps the research into the existing polint architecture direction:
private native analysis modules first, then typed SDK views after stabilization.

## Module Boundary

Recommended internal modules:

```text
crates/polint/src/analysis/evidence/
  mod.rs
  graph.rs
  bundle.rs
  query.rs
  provenance.rs
  render_json.rs
  render_sarif.rs

crates/polint/src/analysis/slicing/
  mod.rs
  local.rs
  interprocedural.rs
  thin.rs
  chop.rs
  ranking.rs
```

Keep these `pub(crate)` initially. Do not add them to `polint::sdk::prelude`
until after diagnostics, JSON output, and harness fixtures prove the shape.

## Inputs

The evidence graph should consume facts through provider traits rather than
direct parser ASTs.

```rust
pub(crate) trait EvidenceProvider {
    fn graph_version(&self) -> GraphVersion;
    fn nodes_for_query(&self, query: &EvidenceQuery) -> Vec<EvidenceNode>;
    fn outgoing_edges(&self, node: EvidenceNodeId, mode: EdgeMode) -> Vec<EvidenceEdge>;
    fn incoming_edges(&self, node: EvidenceNodeId, mode: EdgeMode) -> Vec<EvidenceEdge>;
}
```

Provider adapters:

- semantic operation provider;
- CFG/control-dependence provider;
- def-use/data-dependence provider;
- data-flow provider;
- call graph provider;
- summary provider;
- alias/place provider;
- framework/entrypoint provider;
- extension/model provider;
- diagnostic provider.

## Store Shape

Use compact ids and side tables. Evidence queries need graph traversal speed and
deterministic output, but should not clone source text.

```rust
pub(crate) struct EvidenceStore {
    nodes: IdVec<EvidenceNodeId, EvidenceNode>,
    edges: IdVec<EvidenceEdgeId, EvidenceEdge>,
    out_index: FxHashMap<EvidenceNodeId, SmallVec<[EvidenceEdgeId; 4]>>,
    in_index: FxHashMap<EvidenceNodeId, SmallVec<[EvidenceEdgeId; 4]>>,
    provenance: ProvenanceStore,
    expansions: ExpansionStore,
}
```

The store can be query-local at first. Later, hot graph views can be cached.

## Replay Key

Every evidence bundle should include a replay key that captures enough inputs to
re-run or invalidate the query.

```rust
pub(crate) struct EvidenceReplayKey {
    pub query_kind: QueryKind,
    pub criterion_digest: Hash,
    pub graph_version: GraphVersion,
    pub semantic_digest: Hash,
    pub cfg_digest: Hash,
    pub call_graph_digest: Hash,
    pub summary_digest: Hash,
    pub data_flow_digest: Hash,
    pub alias_digest: Hash,
    pub extension_digest: Hash,
    pub config_digest: Hash,
    pub provider_versions: ProviderVersionDigest,
    pub budget_digest: Hash,
}
```

This should align with the analysis-kernel and summary cache decisions.

## Diagnostic Integration

Initial internal shape:

```rust
pub(crate) struct InternalDiagnostic {
    pub diagnostic: Diagnostic,
    pub evidence_bundle: Option<EvidenceBundleId>,
}
```

If changing the internal diagnostic type is too invasive, use a side table keyed
by stable diagnostic fingerprint. Avoid adding a public constructor until the
schema is settled.

Human output:

- keep compact default text;
- show the top path in a readable "source -> ... -> sink" form when available;
- show unknown/model/summary markers;
- provide a debug flag later for full path/slice expansion.

JSON output:

- add `evidence_v2` or an equivalent versioned field;
- preserve multiple paths and structured provenance;
- keep existing scalar `evidence` for compatibility.

SARIF output:

- map selected evidence paths to `codeFlows` and `threadFlows`;
- include precision/provenance in messages/properties;
- do not attempt to encode every internal alternative path.

## Query Budgets

Default budgets should be conservative:

```rust
pub(crate) struct EvidenceBudget {
    pub max_nodes: u32,
    pub max_edges: u32,
    pub max_paths: u16,
    pub max_path_len: u16,
    pub max_call_depth: u8,
    pub max_summary_expansion_depth: u8,
    pub max_unknown_edges_per_path: u8,
    pub time_budget_ms: u32,
}
```

Budget exhaustion should produce `BudgetExceeded` status and omitted-region
metadata, not silent truncation.

## Agent Extension Hooks

Extension crates should not receive raw mutable access to the evidence graph.
They should register typed facts that providers turn into evidence edges.

Candidate sinks:

- `emit_call_model`;
- `emit_summary_model`;
- `emit_flow_step_model`;
- `emit_source_model`;
- `emit_sink_model`;
- `emit_barrier_model`;
- `emit_evidence_label`;

Every emitted fact must include:

- model id;
- source span in extension/model file;
- target ids or validated selectors;
- precision claim;
- trust level;
- validation state;
- cache digest.

## First Vertical Slice

A good first implementation target:

1. Local def-use evidence for one language adapter path.
2. `ThinBackward` slice for a variable use.
3. JSON debug output with top path and slice node spans.
4. Harness fixture asserting path nodes, edge kinds, and source spans.

Example fixture:

```go
func handler(req Request) {
    name := req.Query("name")
    cmd := "echo " + name
    exec.Command("sh", "-c", cmd)
}
```

Expected evidence:

```text
req.Query("name") -> name -> cmd -> exec.Command argument
```

The first fixture should also assert:

- source span ids are stable;
- edge kinds are `DataValue` or `ParameterIn`;
- no control edge is included in thin mode;
- full mode includes the enclosing statement/control context where applicable;
- replay key changes when the source or model changes.

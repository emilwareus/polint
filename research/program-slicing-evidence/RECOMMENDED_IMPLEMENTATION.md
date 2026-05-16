# Recommended Implementation Path

This is the concrete path for implementing program slicing, path explanation,
and evidence in polint without depending on external analysis libraries.

## Target Architecture

```text
parse/scope/symbol/reference facts
  -> semantic operation facts
  -> CFG/control dependence
  -> place/type/value/alias facts
  -> call graph and summaries
  -> data-flow/dependence facts
  -> evidence graph views
  -> slice/path queries
  -> diagnostics, JSON, SARIF, SDK views
```

The implementation should be Rust-native. Official language tools may feed
lower-level facts when they are the compatibility authority, but slicing and
evidence should normalize into polint-owned ids, edges, provenance, and cache
keys.

## Phase 0: Keep Current Evidence Backward Compatible

Current diagnostics support scalar evidence:

```rust
pub struct Evidence {
    pub label: String,
    pub value: String,
}
```

Do not remove this. Add a structured internal evidence bundle separately and
render selected scalar evidence for existing consumers.

```rust
pub(crate) struct EvidenceBundleId(NonZeroU32);

pub(crate) struct EvidenceBundle {
    pub id: EvidenceBundleId,
    pub primary: EvidenceLocation,
    pub labels: Vec<EvidenceLabel>,
    pub paths: Vec<EvidencePath>,
    pub slices: Vec<SliceSummary>,
    pub unknowns: Vec<EvidenceUnknown>,
    pub provenance: Vec<ProvenanceId>,
    pub replay_key: EvidenceReplayKey,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
}
```

Diagnostics can carry `Option<EvidenceBundleId>` internally before the public
JSON schema is finalized.

## Phase 1: Evidence Node And Edge Store

Add an internal evidence graph view. It should not own every underlying fact.
Instead, it should reference stable fact ids and materialize a query-specific
view.

```rust
pub(crate) enum EvidenceNodeKind {
    Operation(OperationId),
    Statement(SourceSpanId),
    Symbol(SymbolId),
    Place(PlaceId),
    CallSite(CallSiteId),
    FunctionEntry(CallableId),
    FunctionExit(CallableId, ExitKind),
    Summary(SummaryId),
    Model(ModelFactId),
    Diagnostic(DiagnosticId),
}

pub(crate) enum EvidenceEdgeKind {
    DataValue,
    DataTaint,
    DataAddress,
    Control,
    Call,
    Return,
    ParameterIn,
    ParameterOut,
    Summary,
    Model,
    Alias,
    Unknown,
    ExplanationOnly,
}

pub(crate) struct EvidenceEdge {
    pub id: EvidenceEdgeId,
    pub from: EvidenceNodeId,
    pub to: EvidenceNodeId,
    pub kind: EvidenceEdgeKind,
    pub provenance: ProvenanceId,
    pub precision: EvidencePrecision,
    pub expandable: Option<ExpansionKey>,
    pub label: Option<InternedString>,
}
```

Required properties:

- stable ids;
- deterministic edge ordering;
- source span for every user-rendered node when available;
- edge provenance;
- edge precision;
- optional expansion handle for summaries and hidden internal nodes.

## Phase 2: Local Slicing

Start with local slices inside one function. This is the safest vertical slice
because it depends only on semantic operation facts, def-use/data dependence, and
control dependence.

```rust
pub(crate) struct SliceQuery {
    pub criterion: EvidenceNodeId,
    pub direction: SliceDirection,
    pub mode: SliceMode,
    pub edge_filter: EdgeFilter,
    pub budget: SliceBudget,
    pub replay_key_inputs: ReplayKeyInputs,
}

pub(crate) enum SliceMode {
    Thin,
    DataOnly,
    ControlOnly,
    DataAndControl,
    FullLocal,
    FullInterprocedural,
}
```

Default local modes:

- `Thin`: value-producing data edges plus selected summaries.
- `DataOnly`: data value/taint/address edges.
- `DataAndControl`: data plus control dependence.
- `FullLocal`: all local dependence edges and unknowns.

The query result:

```rust
pub(crate) struct SliceResult {
    pub nodes: Vec<EvidenceNodeId>,
    pub edges: Vec<EvidenceEdgeId>,
    pub omitted: Vec<OmittedRegion>,
    pub unknowns: Vec<EvidenceUnknown>,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
    pub stats: SliceStats,
}
```

## Phase 3: Diagnostic Path Evidence

Add path queries for source-to-sink and fact-to-diagnostic explanations.

```rust
pub(crate) struct PathQuery {
    pub starts: Vec<EvidenceNodeId>,
    pub ends: Vec<EvidenceNodeId>,
    pub mode: PathMode,
    pub ranking: PathRanking,
    pub budget: PathBudget,
}

pub(crate) struct EvidencePath {
    pub nodes: Vec<EvidenceNodeId>,
    pub edges: Vec<EvidenceEdgeId>,
    pub score: PathScore,
    pub status: EvidenceStatus,
    pub precision: EvidencePrecision,
}
```

Implement:

- BFS shortest path for unweighted local evidence;
- weighted shortest path for ranked diagnostic paths;
- bounded k-path extraction with deterministic tie-breaks;
- path compression for hidden/internal nodes;
- summary-edge expansion handles.

Never enumerate unbounded paths.

## Phase 4: Interprocedural Direct-Call Evidence

Add context-matched interprocedural traversal only after direct call facts and
minimal summaries exist.

Use a call-site stack:

```rust
pub(crate) struct PathContext {
    pub call_stack: SmallVec<[CallSiteId; 8]>,
    pub depth: u8,
}

pub(crate) struct EvidenceTaskKey {
    pub query_id: QueryId,
    pub node: EvidenceNodeId,
    pub context: PathContext,
    pub edge_filter_digest: Hash,
    pub graph_version: GraphVersion,
}
```

Rules:

- entering a callee pushes the call site;
- returning to caller must pop the matching call site;
- summary edges can skip push/pop but must keep an expansion key;
- unresolved dynamic calls create explicit unknown/havoc nodes, not silent
  missing paths.

This copies the core lesson from WALA and Joern: context matters, and the cache
key must include context.

## Phase 5: Summary Edge Compression And Expansion

Summary edges should be first-class evidence edges:

```rust
pub(crate) struct SummaryEvidence {
    pub summary_id: SummaryId,
    pub subject: CallableId,
    pub domain: SummaryDomainId,
    pub input: FlowEndpoint,
    pub output: FlowEndpoint,
    pub status: SummaryStatus,
    pub precision: SummaryPrecision,
    pub expansion: SummaryExpansion,
}

pub(crate) enum SummaryExpansion {
    Expandable(ExpansionKey),
    Opaque { reason: UnknownReason },
    ExternalModel { model_fact: ModelFactId },
}
```

Rendering default:

- show summary as one path step;
- include precision/provenance;
- allow debug JSON to expand it if available.

## Phase 6: JSON And SARIF Output

Add structured JSON evidence first. SARIF should be a lossy renderer from the
same internal model.

JSON should preserve:

- all candidate paths up to configured limit;
- edge kinds;
- provenance;
- precision/status;
- hidden node counts;
- unknowns;
- replay key;
- summary expansion keys.

SARIF should map selected evidence paths to:

- `codeFlows`;
- `threadFlows`;
- `threadFlowLocations`;
- related locations and messages.

Do not force internal evidence to match SARIF's shape exactly. Semgrep's source
shows why: renderer formats can force a single path even when the engine knows
more.

## Phase 7: Agent Extension Integration

Agent-authored Rust extensions should be able to add evidence-relevant facts:

- source/sink/barrier/sanitizer models;
- framework dispatch edges;
- call graph edges;
- summary edges;
- custom data-flow steps;
- evidence labels and grouping hints.

They should not be able to:

- forge native provenance;
- silently suppress native may edges;
- claim exactness without validation;
- add unbounded path expansion;
- attach evidence to nonexistent source spans.

Merge policy:

```rust
pub(crate) enum EvidenceMergeVerdict {
    Accept,
    AcceptWithPrecisionDowngrade,
    CandidateOnly,
    Reject,
}
```

Validation must check:

- referenced ids exist;
- access paths are valid;
- model source spans exist;
- claimed precision is allowed;
- fixture coverage exists for suppressive or strengthening behavior;
- cache keys include extension digest.

## Phase 8: Public SDK Views

Expose SDK views only after internal use stabilizes.

Candidate future views:

```rust
Slices<'_>
Paths<'_>
Evidence<'_>
```

Rule authors should request typed views in `#[polint::rule]` signatures, not pull
raw graph internals from `RuleCtx`.

Example future shape:

```rust
#[polint::rule]
fn no_untrusted_shell(
    ctx: &mut RuleCtx<'_>,
    flows: DataFlows<'_>,
    paths: Paths<'_>,
) -> RuleResult {
    for hit in flows.source_to_sink("http_request", "shell_exec") {
        let evidence = paths.explain(hit.source(), hit.sink())?;
        ctx.report(
            Diagnostic::warning("untrusted input reaches shell execution")
                .with_primary(hit.sink().span())
                .with_evidence_bundle(evidence),
        );
    }
    Ok(())
}
```

This is not a v1 public API. It is the target shape.

## Implementation Sequence

Recommended order:

1. Add internal evidence ids, node/edge kinds, provenance fields, and query
   budgets.
2. Add local evidence graph adapters over semantic operation, def-use, and
   control-dependence facts.
3. Implement `ThinBackward` and `DataAndControl` local slices.
4. Implement local `PathQuery` with deterministic ranked shortest path.
5. Attach `EvidenceBundleId` to internal diagnostics and JSON debug output.
6. Add source-to-sink diagnostic evidence for the first data-flow rule family.
7. Add summary edges and expansion keys.
8. Add direct-call interprocedural context matching.
9. Add SARIF code-flow rendering.
10. Add extension merge validation.
11. Add harness fixtures and external benchmarks.
12. Promote stable SDK views after API review.

## What Not To Build First

- executable transformed slices;
- whole-program SDG materialized for every run;
- generic IFDS engine before the semantic bootstrap;
- public raw graph API;
- unbounded path enumeration;
- LLM-generated slices as trusted facts;
- a one-path-only internal evidence model.

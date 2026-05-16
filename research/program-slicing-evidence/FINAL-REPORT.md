# Final Report: Program Slicing, Path Explanation, And Evidence

## Practical Conclusion

Build slicing as the evidence/query layer over polint's native analysis facts.
Do not build a standalone slicer, do not make executable code slicing the first
goal, and do not use generated LLM slices as trusted facts.

The first useful product should be:

```text
diagnostic
  + primary location
  + related source labels
  + one or more ranked paths
  + optional thin slice region
  + explicit unknowns
  + provenance for every modeled/native/heuristic segment
  + replayable query key
```

This aligns with polint's agent-extensible thesis. A human or AI agent should be
able to inspect a finding, see exactly what facts and modeled edges produced it,
then add a repo-local Rust extension that improves the next scan.

## What State Of The Art Means Here

The strongest systems are not just "slicers." They combine graph construction,
summary reasoning, context handling, and explanation rendering:

- WALA: mature SDG/PDG slicing with data/control/heap/exception options and
  tabulation-based context handling.
- CodeQL: path graph and path-query rendering with hidden nodes and expandable
  subpaths.
- Joern: practical CPG data-flow paths, call-site stack fingerprints, and
  composable semantics overlays.
- Semgrep: accessible source/intermediate/sink traces and SARIF/JSON/text
  output, with practical limits visible in code.
- Frama-C: slicing as criteria plus dependency-kind selection and PDG mark
  propagation.
- JavaSlicer: readable Java SDG/ICFG construction with actual-in/out call
  expansion.

The foundational papers remain directly relevant:

- Program Dependence Graphs make data and control dependencies explicit.
- System Dependence Graphs add call/parameter/summary structure.
- Interprocedural slicing must preserve calling context.
- Thin slicing reduces human inspection cost by prioritizing producer
  statements.

Recent neural/agentic papers are useful, but they do not replace native
analysis. SliceFormer, SliceMate, and SliceT5 show that learned systems benefit
from data-flow structure, constrained decoding, verification, and benchmarks.
For polint, they are best used as benchmark inspirations, extension-generation
assistants, and comparison points.

## Recommended Architecture

Add two internal modules after the semantic bootstrap and core dependence facts
exist:

```text
crates/polint/src/analysis/evidence/
crates/polint/src/analysis/slicing/
```

The modules should depend on existing and planned fact families:

```text
semantic operation facts
CFG and control dependence
place/type/value/alias facts
call graph facts
summary facts
data-flow facts
framework/entrypoint facts
extension/model facts
diagnostic facts
```

The evidence graph should be a typed view:

```text
EvidenceNodeId
EvidenceEdgeId
EvidenceNodeKind
EvidenceEdgeKind
EvidenceMeta
EvidenceBundle
SliceQuery
SliceResult
PathQuery
PathResult
```

Do not expose this as stable SDK immediately. First use it internally for
diagnostics, JSON debug output, and harness assertions.

## Query Modes

Support explicit query modes from the beginning:

| Mode | Default use | Edge selection |
|---|---|---|
| `ThinBackward` | Diagnostic "why this value?" | Value-producing data edges, selected summaries. |
| `FullBackward` | Deep debug | Data, address, control, call/return, summary, model, unknown. |
| `ForwardImpact` | "What can this affect?" | Outgoing dependence/data-flow edges. |
| `Chop` | Source-to-sink diagnostics | Source forward reachability intersect sink backward reachability. |
| `Path` | SARIF/JSON/human trace | Ranked path over filtered evidence edges. |
| `Expansion` | Agent debug | Expand hidden nodes or summary subpaths. |

Thin mode should be the default user-facing evidence because full slices are too
large. Full mode should remain available for debugging, validation, and agents
that need context.

## Provenance And Trust

Every node and edge must carry provenance. This is not optional for polint's
agent-extension model.

```text
Native(provider, version, input_digest)
OfficialTool(tool, version, invocation_digest)
Heuristic(provider, reason)
BuiltinModel(model_id, version)
AgentExtension(crate_id, version, validation_state)
GeneratedModel(model_id, generator, validation_state)
BenchmarkOracle(suite, case_id)
Unknown(reason)
```

Merge policy:

- native may facts cannot be silently suppressed by a lower-trust extension;
- extension facts cannot claim `ExactSemantic`;
- unvalidated extensions can produce candidate evidence but cannot strengthen or
  suppress diagnostics without validation;
- unknowns resolved by extensions must remain traceable to the model span and
  fixture evidence;
- conflicting edges should produce validation diagnostics, not silent wins.

## How To Keep Slices Useful

Default evidence should be small:

- use thin slices first;
- hide parser/MIR/internal nodes unless requested;
- compress summaries and allow expansion;
- rank paths by native precision, fewer unknowns, fewer model edges, shorter
  path, and source proximity;
- show unresolved dynamic behavior as actionable unknowns;
- cap paths, nodes, recursion, and expansion.

Recommended path ranking:

```text
score(path) =
  native_edge_weight
  - unknown_penalty
  - unvalidated_model_penalty
  - heuristic_penalty
  - path_length_penalty
  - summary_opaque_penalty
  + direct_source_sink_bonus
```

This score is for display ordering only. It must not change solver soundness.

## Implementation Dependency Order

This research should be implemented after the following are in place:

1. Semantic bootstrap operation ids and source spans.
2. CFG and control dependence facts.
3. Place/def-use and basic data dependence.
4. Direct call facts and unresolved-call facts.
5. Minimal function summaries.
6. Diagnostic JSON shape that can carry structured evidence.

Then implement:

1. Internal evidence graph view.
2. Local backward/forward slice over one function.
3. Thin slice mode.
4. Path query over local data-flow/dependence edges.
5. Diagnostic evidence bundles in JSON/debug output.
6. Summary-edge path compression and expansion.
7. Direct-call interprocedural evidence with call-site stack matching.
8. SARIF `codeFlows`/`threadFlows` rendering.
9. Agent extension evidence merge and validation.
10. Public SDK views only after internal shape stabilizes.

## What This Enables

For rules:

- attach rich explanations without hand-building trace strings;
- ask "why did this source reach this sink?";
- compare default analysis against extension-improved analysis;
- expose uncertainty to users instead of hiding false negatives.

For AI agents:

- inspect the smallest meaningful slice around a diagnostic;
- see unresolved calls/framework gaps as implementation tasks;
- write validated Rust extensions that add summaries, flow steps, sources,
  sinks, barriers, or framework dispatch edges;
- rerun the harness and compare precision deltas.

For polint:

- path quality becomes a measurable product feature;
- false positives become debuggable;
- missing models become concrete work items;
- future advanced domains have a shared explanation substrate.

## Open Questions

- How much of `EvidenceBundle` should be exposed in the first public SDK version
  versus kept internal?
- Should JSON output always include all candidate paths, or only include a
  compact default plus a debug flag for alternatives?
- What is the minimum path model that maps cleanly to SARIF without losing
  polint-specific provenance?
- How should branch/path predicates from future abstract interpretation domains
  integrate with slice/path evidence?
- Should a query result be cacheable across rules, or should query caching be
  scoped by capability request and rule options?

## Final Recommendation

Make evidence a first-class internal product surface before exposing it as a
stable public SDK. Build the engine so every diagnostic can eventually answer:

```text
What facts led here?
Which code locations are on the path?
Which summary/model edges were used?
Which unknowns remain?
What would an agent need to model to improve this?
```

That is the implementation path most aligned with building a max-capability
static-analysis engine for AI-assisted development.

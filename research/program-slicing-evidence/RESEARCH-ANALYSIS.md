# Research Analysis

## Executive Analysis

The state of the art is not one algorithm. It is a layered architecture:

```text
semantic graph construction
  + dependence graph construction
  + context-correct interprocedural reachability
  + summary edges
  + small human-oriented path/slice rendering
  + provenance and precision controls
```

The key research lesson is old and still decisive: a slice over a fixed
intraprocedural Program Dependence Graph is just graph reachability, but
interprocedural slicing is not naive graph reachability. It must preserve valid
call/return matching or use summary edges that encode the same effect.

For polint, the right design is a native evidence graph that can answer slice
and path queries over existing fact layers. The graph should be demand-built
from semantic operation facts, CFG/control dependence, def-use/data dependence,
call graph edges, summaries, data-flow edges, alias facts, framework models, and
extension facts.

## Algorithm Families

### PDG Slicing

The Program Dependence Graph makes data and control dependence explicit. Given a
criterion node, slicing is reachability over incoming or outgoing dependence
edges.

Accuracy:

- Strong for local explanation when CFG and def-use facts are precise.
- Control dependence makes slices semantically richer but often much larger.
- Heap, aliases, exceptions, dynamic calls, callbacks, reflection, async, and
  generated code dominate false positives and false negatives.

Complexity:

- Building the local graph is roughly `O(CFG_edges + def_use_edges +
  control_dep_edges)` after CFG and use-def facts exist.
- A single slice over a fixed graph is `O(V + E)` for selected edge classes.
- Memory cost is proportional to retained graph edges and metadata; avoid
  materializing derived slices permanently.

polint fit:

- Good first local evidence engine.
- Requires semantic operation ids and source spans from the bootstrap.
- Should expose mode filters: `data_only`, `control_only`, `data_and_control`,
  `thin`, `full`.

### SDG And Interprocedural Slicing

The System Dependence Graph extends PDG with calls, parameter-in/out nodes, and
summary edges. Horwitz/Reps/Binkley show why summary edges are necessary:
interprocedural slices must avoid unrealizable paths that enter through one call
site and return through another.

Accuracy:

- Much stronger than naive whole-program graph reachability.
- Precision depends on call graph, summaries, alias/mod-ref, and context
  abstraction.
- Context matching can be exact for bounded call strings but can still explode
  in recursive or highly higher-order code.

Complexity:

- Intraprocedural reachability remains linear in graph size.
- Interprocedural reachability adds context state. Cost is roughly
  `O(E * contexts)` for call-string style traversal, and can become large when
  context keys multiply across call sites, receivers, allocation sites, or
  abstract values.
- Summary-edge computation is a fixpoint over procedures and call edges. It
  should reuse the function-summary kernel and SCC scheduling already researched
  for polint.

polint fit:

- Use parameter-in/out and summary-edge concepts, but build on polint's
  `SummaryKey`, call graph, and semantic operation store.
- Implement direct-call and summary-based interprocedural evidence before
  attempting broad higher-order precision.

### IFDS/IDE-Style Path Reachability

WALA's slicer uses a partially balanced tabulation solver. The broader
Reps/Horwitz/Sagiv IFDS/IDE family solves finite distributive data-flow
problems with summary reuse.

Accuracy:

- Excellent when the problem fits finite facts and distributive transfer.
- Handles call/return structure explicitly.
- Less natural for arbitrary rich state unless encoded into finite facts or IDE
  edge functions.

Complexity:

- Classic IFDS is polynomial in program size and fact-domain size, often
  described around `O(E * D^3)` in worst-case formulations, with better behavior
  in practical sparse engines.
- Cost is dominated by the number of data-flow facts, call contexts, and summary
  edges.

polint fit:

- Do not start with a generic IFDS engine for all evidence. It is too much
  infrastructure before the native semantic bootstrap exists.
- Preserve compatibility with future IFDS-like providers by making evidence
  edges typed and summary-expandable.

### Thin Slicing

Thin slicing intentionally removes many explanatory dependencies to show the
producer statements most likely to matter. Sridharan/Fink/Bodik report large
human-effort reductions: 3.3x fewer inspected statements for debugging and 9.4x
fewer for program understanding.

Accuracy:

- Less semantically complete than full slices.
- Better human precision for "where did this value come from?"
- Must be honest that omitted control/base-pointer edges can matter.

Complexity:

- Same graph traversal shape as ordinary slicing, but over fewer edge kinds.
- Result size is often much smaller, which lowers rendering and agent-context
  cost.

polint fit:

- This should be the default view for diagnostics and AI-agent context.
- Provide expansion handles for "show controls", "show calls", "show heap",
  "show summary expansion", and "show unknowns".

### Chops

A chop asks for the relevant region between source and sink.

```text
chop(source, sink) =
  forward_reachable(source) intersect backward_reachable(sink)
```

In an interprocedural graph, this must still preserve call/return feasibility.

Accuracy:

- Good for source-to-sink diagnostics and "why is this sink reachable?"
- Can be much smaller than a full backward slice from the sink.
- Still suffers from path explosion when many alternative routes exist.

Complexity:

- Two reachability queries plus intersection: `O(V + E)` over a fixed
  context-insensitive graph.
- Context-sensitive chop can multiply by context count.
- Ranking and k-path extraction add cost.

polint fit:

- Use chops for diagnostic evidence when a rule identifies a source and sink.
- Pair with path ranking rather than dumping the whole chop by default.

### Path Explanation

Path explanation returns one or more human/agent-readable paths. CodeQL,
Semgrep, Joern, and SARIF all show the same product lesson: the path is often
the most valuable output, even when the underlying graph is approximate.

Accuracy:

- A path can be true under the chosen abstraction without being feasible in the
  concrete program.
- Call/return matching, summary expansion, branch conditions, and sanitizers are
  the main differentiators between good and misleading paths.

Complexity:

- Shortest path by BFS over unweighted edges is `O(V + E)`.
- Weighted/ranked path search is roughly `O(E log V)` for one shortest path.
- Enumerating many paths can explode exponentially. Always cap path count,
  length, repeated nodes, and summary expansion depth.

polint fit:

- Store many candidate paths internally when cheap, but render a small ranked
  subset.
- Include path status and precision in output.
- Keep a replay key so a path can be regenerated after edits or invalidated when
  inputs change.

## What The Implementations Teach

### WALA

WALA is the strongest direct model for slicing architecture. It explicitly
separates dependence options, control options, SDG construction, and tabulation
solving. It also includes thin slicing as a practical mode.

polint should copy:

- explicit precision knobs;
- separate data/control/heap/exception options;
- SDG-style parameter-in/out and summary boundaries;
- context-correct interprocedural traversal;
- thin slicing as a first-class mode.

polint should not copy:

- JVM-specific class hierarchy and pointer-analysis assumptions;
- exposing every low-level analysis knob to the public SDK too early.

### CodeQL

CodeQL is the strongest model for path rendering. Its path graph signature,
hidden nodes, `PathNode`, and `subpaths` give a clean separation between solver
facts and user-facing explanation.

polint should copy:

- path nodes only on source-to-sink relevant regions;
- hidden node compression;
- summary subpath expansion;
- explicit selected source/sink locations;
- path graph separate from raw data-flow graph.

polint should not copy:

- database-first architecture;
- stringly public query language for core engine internals.

### Joern

Joern is the strongest model for CPG-style practical data-flow evidence and
extension semantics. `TaskFingerprint(sink, callSiteStack, callDepth)` is a
particularly useful shape for caching and path validity.

polint should copy:

- call-site stack in path/evidence queries;
- task fingerprints for memoization;
- composable semantics/model layers;
- visible path filtering.

polint should not copy:

- broad untyped CPG as the internal public contract;
- deferring too much precision to query-time filtering without recording
  precision/provenance.

### Semgrep

Semgrep shows how valuable practical traces are, and how output formats can
collapse internal possibilities. Its limitation of selecting a first trace for
some output paths is exactly what polint should avoid internally.

polint should copy:

- source/intermediate/sink trace shape;
- clear human text output;
- SARIF/JSON path rendering;
- lightweight shape/access-path sensitivity.

polint should not copy:

- internal trace collapse to one path;
- path-insensitive ambiguity without explicit uncertainty.

### Frama-C

Frama-C shows that slicing criteria and dependency kinds must be first-class
controls. It also shows the power of selection marks: slicing is often a
propagation problem over marked graph regions.

polint should copy:

- explicit selection kinds: data, address, control, node plus dependencies;
- criterion APIs for calls, returns, assertions, reads/writes, and values;
- caller/callee propagation modes;
- reverse topological propagation through call structure.

polint should not copy first:

- transformed code output as the primary product.

### JavaSlicer

JavaSlicer is less mature than WALA but readable. Its ICFG expansion into
actual-in, call, return, and actual-out nodes reinforces the SDG design shape.

polint should copy:

- explicit call expansion nodes;
- SCC/condensation awareness.

polint should not copy:

- hard failure when every method call cannot resolve. polint should emit
  explicit unknown/setup diagnostics and continue conservatively.

## Recent Neural And Agentic Slicing

SliceFormer, SliceMate, and SliceT5 are important because they show how current
research is trying to use learned models and agents for slicing. They do not
change polint's core architecture recommendation.

Lessons:

- Learning-based slicers still benefit from data-flow-aware structure and
  constrained decoding.
- Agents can synthesize and refine slices, but those outputs need verification.
- Benchmarks such as CodeNetSlice and SliceBench can become evaluation inputs.
- Generated slices should not be trusted as native facts. They can be candidate
  evidence, extension suggestions, benchmark comparisons, or model-generation
  aids.

For polint's product thesis, this is exactly the split:

```text
native engine produces stable facts and uncertainty
AI agent reads evidence and writes validated Rust extensions
extension facts merge back with provenance and tests
```

## Accuracy Discussion

Accuracy has multiple dimensions:

| Dimension | Failure mode | Required mitigation |
|---|---|---|
| Data dependence | Missing def-use, alias, heap, or field relation. | Place/access-path model, alias provider stack, summary TITO, unknown edges. |
| Control dependence | Over-large slices or missing exceptional/async controls. | CFG/control-dep provider with edge classes and expansion controls. |
| Interprocedural context | Unrealizable paths across mismatched calls/returns. | Call-site stack or summary-edge traversal. |
| Dynamic dispatch | Missing targets or too many targets. | Call graph precision tiers, type facts, points-to, extension call models. |
| Framework dispatch | Missing synthetic routes/jobs/callbacks. | Entrypoint/framework model layer and extension validation. |
| Sanitizers/barriers | False positives or false negatives in data-flow paths. | Validated model facts with labels, kinds, and provenance. |
| Summaries | Opaque edge hides why path exists. | Expandable summary subpaths when available; opaque summary with evidence when not. |
| Agent extensions | Extension can lie or over-suppress. | Merge policy, trust level, fixture requirements, precision downgrade. |

The engine should not pretend all evidence is equal. A path with native
context-matched edges and an expandable summary is not the same as a path that
depends on unknown dynamic dispatch and an unvalidated model.

## Complexity And Cost Controls

Polint should treat slicing as a demand query, not a precomputed global product.

Required controls:

- edge-kind filters;
- max nodes in slice;
- max path length;
- max paths;
- max summary expansion depth;
- max call-string depth;
- max unknown/havoc edges per path;
- max branch fanout;
- deterministic tie-breaking;
- query result caching by graph version, query key, options, and extension
  digest.

Default diagnostic mode should use:

```text
thin chop/path first
small ranked path set
hidden internal nodes
summary edges compressed
unknowns shown explicitly
expandable details available in JSON/debug output
```

## Rejected Paths

### Build Executable Slices First

Rejected for v1. Executable slicing requires statement-preserving
transformation, declarations, imports, side effects, formatting, and language
specific repair. It is valuable later for code-reduction workflows, but not
needed to make diagnostics explainable.

### Use LLM Slices As Trusted Facts

Rejected. Recent papers are useful, but a native static-analysis engine cannot
base trusted diagnostics on generated slice text. LLM/agent output can propose
models, generate tests, or compare benchmark outputs, then validated Rust code
can add facts.

### One Global Evidence Graph Materialized Up Front

Rejected. The graph will be too large and too dependent on requested rule
capabilities. Build indexed base facts and materialize evidence graph views on
demand.

### Public Raw AST Evidence

Rejected. Raw AST nodes are parser-specific and unstable. Evidence must use
stable semantic ids and source spans.

### Collapse To A Single Trace Internally

Rejected. Output formats may display one path, but the engine must preserve
alternatives, ambiguity, and rankings internally.

## Product Conclusion

Evidence is the user-facing form of the engine. The engine can be conservative,
approximate, or extension-driven, but it must explain which facts participated,
which assumptions were made, which unknowns remain, and what an agent could do
to improve precision.

# Standard: How To Talk About Slicing, Paths, And Evidence

This standard exists so future research and implementation notes use the same
words. Slicing, path explanation, and evidence are related, but they are not the
same thing.

## Core Terms

| Term | Meaning |
|---|---|
| Slice criterion | The starting point for a slice: a statement, expression, variable, operation id, fact id, diagnostic id, or source/sink pair. |
| Backward slice | Program elements that may influence the criterion. |
| Forward slice | Program elements that may be influenced by the criterion. |
| Chop | The relevant region between a source and a sink, usually source forward reachability intersected with sink backward reachability. |
| Thin slice | A smaller slice that follows value-producing data dependence first and suppresses many explainers such as control/base-pointer edges until expanded. |
| Full slice | A slice over selected data, control, call, return, summary, heap, and exception edges. |
| Dependence graph | A graph where edges represent data dependence, control dependence, call/return structure, summary transfer, model edges, or explanation-only links. |
| Path explanation | One or more concrete paths through evidence edges that explain why a source can reach a sink or why a diagnostic fired. |
| Evidence bundle | A structured explanation attached to a diagnostic or query result. It can include paths, slices, related locations, provenance, uncertainty, and replay keys. |
| Summary edge | A compressed interprocedural edge representing callee behavior. It must be expandable or explicitly marked as opaque. |
| Provenance | Where a node or edge came from: native provider, official language tool, heuristic, extension crate, generated model, benchmark oracle, or imported external model. |
| Precision | What the engine is allowed to claim: exact local, summary-based, heuristic, unknown, model-declared, etc. |

## Evidence Is Not Just Text

The current polint diagnostic model has simple evidence pairs:

```rust
pub struct Evidence {
    pub label: String,
    pub value: String,
}
```

That is useful for scalar facts, but insufficient for path explanations. The
research target is a structured evidence model that can still render down to
human text, JSON, and SARIF.

```text
EvidenceBundle =
  primary diagnostic location
  + related labels
  + one or more paths
  + optional slice regions
  + graph nodes and edges
  + provenance and precision
  + unknowns and budget notes
  + replayable query key
```

## Edge Kinds

Keep edge kinds typed. A single untyped "depends_on" edge loses too much
information for ranking, filtering, provenance, and extension validation.

| Edge kind | Meaning |
|---|---|
| `DataValue` | Value definition contributes to a use. |
| `DataTaint` | Taint-like or label-like data propagates. |
| `DataAddress` | Address/reference/container dependence. |
| `Control` | Control dependence from branch/exception/loop condition. |
| `Call` | Caller to callee entry. |
| `Return` | Callee exit to caller continuation/result. |
| `ParameterIn` | Actual argument to formal parameter. |
| `ParameterOut` | Formal output/return/mutation to actual result/location. |
| `Summary` | Compressed callee or library behavior. |
| `Model` | Agent-authored or built-in framework/source/sink/model fact. |
| `Alias` | Alias or points-to relation used by another edge. |
| `Unknown` | Conservative unknown/havoc relation. |
| `ExplanationOnly` | Display breadcrumb that should not affect solver semantics. |

## Node Kinds

| Node kind | Meaning |
|---|---|
| `Operation` | A semantic MIR operation or expression-level operation. |
| `Statement` | Source statement span. |
| `Symbol` | Function, method, variable, field, type, package, or module symbol. |
| `Place` | Access path such as `param[0].user.id`. |
| `CallSite` | Call expression plus resolved/unresolved targets. |
| `FunctionEntry` | Function or synthetic entrypoint boundary. |
| `FunctionExit` | Return/throw/panic/yield/await boundary. |
| `SummaryNode` | Opaque or expandable summary application. |
| `ModelNode` | Extension/model source span or generated synthetic node. |
| `DiagnosticNode` | The diagnostic being explained. |

## Precision And Status

Every evidence path and slice should carry precision. Unknown is not empty.

| Status | Meaning |
|---|---|
| `Complete` | Query completed within its selected graph and budget. |
| `Partial` | Useful result, but one or more edges/nodes were omitted due to budget or unsupported expansion. |
| `NoPath` | No path under the selected mode. |
| `SetupMissing` | Required type/module/classpath/toolchain input is missing. |
| `Unresolved` | A callee/reference/type/source/sink could not be resolved. |
| `BudgetExceeded` | Query stopped due to size, path count, recursion, or time budget. |
| `Invalidated` | Replay key no longer matches current facts. |
| `Rejected` | Extension/model evidence failed validation. |

| Precision | Meaning |
|---|---|
| `ExactLocal` | Exact within one body under the selected semantic operation graph. |
| `ContextMatched` | Interprocedural path preserves a valid call/return context. |
| `SummaryBased` | Uses expandable or opaque summaries. |
| `FrameworkModeled` | Relies on framework/lifecycle model facts. |
| `Heuristic` | Useful approximation; cannot claim full coverage. |
| `DeclaredExternal` | Supplied by extension/model rather than inferred. |
| `UnknownTop` | Conservative top/havoc edge participates. |

## Report Template For Implementations

Every tool report should answer:

1. What graph does it build?
2. What can it slice or explain?
3. How does it preserve call/return context?
4. How are summaries represented and expanded?
5. How are paths compressed, hidden, ranked, or rendered?
6. What precision knobs exist?
7. How does it handle heap, exceptions, reflection, dynamic dispatch, callbacks,
   and missing models?
8. What is the cost model?
9. What evidence/provenance is visible to users?
10. What should polint copy, adapt, or reject?

## Pseudo-code Conventions

Pseudo-code is Python-ish and intentionally stripped down:

```python
def backward_slice(graph, criterion, edge_filter):
    seen = set([criterion])
    work = [criterion]
    while work:
        node = work.pop()
        for edge in graph.in_edges(node):
            if edge_filter(edge) and edge.src not in seen:
                seen.add(edge.src)
                work.append(edge.src)
    return seen
```

The real implementation should be Rust-native and use stable ids, typed edges,
deterministic ordering, bounded caches, provenance, and validation gates.

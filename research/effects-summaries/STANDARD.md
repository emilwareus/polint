# Standard: How To Talk About Summaries And Effects

This standard exists so future research and implementation notes use the same
words. A "summary" in this research is not one thing. It is a typed, versioned,
provenance-labeled approximation of a program element's behavior under an
analysis domain.

## Summary Object

```text
Summary =
  subject identity
  + domain id and domain version
  + context abstraction
  + transfer/effect payload
  + precision/status/provenance
  + dependencies and cache key inputs
  + validation record
```

The subject is normally a function, method, closure, synthetic framework entry,
library API, generated client method, or cross-language boundary.

## Required Fields

| Field | Meaning |
|---|---|
| `summary_id` | Stable content-addressed id for this summary instance. |
| `subject_id` | Stable symbol/callable id, not just a display name. |
| `language` | Language/provider family, such as Go, TS, JS, Python, JVM, Rust. |
| `package_or_module_id` | Module/package/source-set identity. |
| `domain` | Typed summary domain, such as `ControlEffects` or `DataFlowTito`. |
| `domain_version` | Semantic version of the payload shape and transfer rules. |
| `context_key` | Context abstraction: context-insensitive, call-string, allocation context, receiver type, specialization, or demand query. |
| `payload` | Domain-specific summary data. |
| `status` | Whether the summary is usable, incomplete, heuristic, setup-missing, etc. |
| `precision` | What kind of exactness is claimed. |
| `provenance` | Native analyzer, official language tool, extension crate, generated model, handwritten model, imported benchmark, or oracle. |
| `trust` | How much the engine may rely on it for exact claims. |
| `evidence` | Source spans, model file spans, fixture ids, benchmark ids, or derivation traces. |
| `deps` | Source digests, config digests, dependency summaries, model pack digests, provider versions. |

## Precision And Status

Every summary must carry precision. Unknown is not empty.

| Status | Meaning |
|---|---|
| `Complete` | Provider completed within its domain and budget. |
| `Incomplete` | Provider produced useful facts but not a full domain result. |
| `SetupMissing` | Required toolchain, dependency, module root, classpath, or type info was missing. |
| `Unsupported` | Provider cannot model this construct yet. |
| `Ambiguous` | Multiple plausible interpretations remain. |
| `Unresolved` | Required reference/callee/type/target was unresolved. |
| `BudgetExceeded` | Widening, size, time, or memory budget caused loss of precision. |
| `Invalidated` | Cache input changed; summary cannot be used. |
| `Rejected` | Extension/model failed validation. |

| Precision | Meaning |
|---|---|
| `ExactSemantic` | Derived from official language semantics or a trusted complete provider for that domain. |
| `ExactLocal` | Exact for one body without assuming callees/frameworks are complete. |
| `ModuleLinked` | Resolved through configured package/module/classpath/type setup. |
| `SummaryBased` | Depends on summaries of callees or dependencies. |
| `FrameworkModeled` | Depends on framework/lifecycle model overlays. |
| `Heuristic` | Useful approximation; cannot claim completeness. |
| `DeclaredExternal` | Supplied by model/extension, not independently inferred. |
| `UnknownTop` | Conservative top value: may do the domain's maximum behavior. |

## Summary Domains

Do not collapse all of these into one untyped object.

| Domain | Payload Shape |
|---|---|
| `ControlEffects` | normal return, no return, throw/panic/reject, exit, await/yield, defer/finally, callback scheduling. |
| `CallEffects` | direct calls, indirect calls, dynamic dispatch candidates, callback invocations, unresolved calls, synthetic framework edges. |
| `DataFlowTito` | parameter/receiver/global/capture to return/parameter/receiver/global flows, with access paths and flow kind. |
| `MemoryEffects` | read/write/mod-ref over abstract resources and locations. |
| `AliasEscapeEffects` | escaped parameters, captured allocations, returned aliases, stored callbacks, unknown heap exposure. |
| `ResourceEffects` | acquire/release/open/close/lock/unlock/await/free/drop obligations. |
| `TaintEffects` | sources, sinks, sanitizers, barriers, transforms, labels, features/breadcrumbs. |
| `ConcurrencyEffects` | goroutine/thread/task/promise spawn, lock use, channel send/receive, async callback after return. |
| `ExternalEffects` | file system, network, database, shell/process, environment, clock/randomness, logging, telemetry, AI/tool calls. |

## Report Template For Implementations

Every tool report should answer:

1. What subject is summarized?
2. What domain is summarized?
3. Is the summary inferred, declared, generated, or manually modeled?
4. What lattice or transfer representation is used?
5. How are recursion and SCCs handled?
6. How are unknown calls/frameworks/native/reflection handled?
7. What cache/invalidation shape exists?
8. What provenance/precision is visible?
9. What is the accuracy/cost tradeoff?
10. What should polint copy, adapt, or reject?

## Pseudo-code Conventions

Pseudo-code is Python-ish. It is intentionally stripped down:

```python
def analyze_function(fn, input_summaries):
    state = Domain.bottom()
    for block in cfg.postorder(fn):
        state = transfer_block(block, state, input_summaries)
    return state.to_summary()
```

When the real implementation should be Rust-native, pseudo-code still names
the data structures that should become Rust traits/enums/structs.

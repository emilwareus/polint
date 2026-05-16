# Function Effects And Summaries: Final Report

Date: 2026-05-16

## Executive Summary

Summaries are the scaling boundary for polint. They are the point where local
analysis becomes reusable, where whole-program guesses become demandable facts,
and where agent-authored repo knowledge can safely improve precision.

The key design choice is:

```text
Do not build one generic summary format.
Build a stable summary kernel plus typed, versioned summary domains.
```

The summary kernel owns identity, provenance, precision, validation, cache keys,
invalidation, extension merge policy, and scheduling. Each domain owns its own
lattice and transfer semantics: calls, control effects, data-flow TITO, memory
mod/ref, taint, resource/typestate, concurrency, external effects, and
alias/escape.

This gives polint maximum long-term capability without building into a corner:

- call graphs can use summaries for callbacks, dynamic dispatch, no-return
  calls, framework edges, and unresolved-call triage;
- data flow can use summaries for parameter-to-return, parameter-to-sink,
  receiver mutation, sanitizers, barriers, and access-path flow;
- alias and points-to can use summaries for escape, returned alias, stored
  callback, and unknown heap exposure;
- framework models can add lifecycle and synthetic-call summaries without
  pretending they are parser facts;
- AI agents can write Rust extension providers that add summary facts with
  strong validation, fixtures, provenance, and precision ceilings.

## Why Summaries Are The Boundary

Without summaries, every analysis has two bad options:

1. Stay local and miss important behavior behind helper functions, wrappers,
   generated clients, frameworks, callbacks, and libraries.
2. Inline whole programs and explode across recursion, dependency graphs,
   framework entrypoints, dynamic dispatch, async scheduling, and unresolved
   code.

A summary is a compact replacement for re-entering a callee every time:

```python
def transfer_call(call, state):
    callee = resolve(call)
    if callee.has_summary(domain=state.domain):
        return apply_summary(callee.summary, call, state)
    return state.join(Domain.unknown_call_effect(call))
```

That is more than a performance optimization. It is the only scalable way to
make interprocedural facts stable enough for caching, incremental invalidation,
agent extension, and evidence generation.

## What The State Of The Art Converges On

Across CodeQL, Pysa, Infer, LLVM/MLIR, Go analysis, WALA/Soot/Doop/OPAL,
Heros/PhASAR, and Semgrep, the successful systems converge on the same pattern:

```text
local analysis
  -> typed summary/model
  -> global propagation/fixpoint/demand query
  -> explicit unknown/model/fallback behavior
```

They differ in domain and product tradeoff:

- CodeQL is the strongest model for declarative library summaries and access
  paths. `summaryModel` encodes input/output flow such as `Argument[0]` to
  `ReturnValue`, with provenance and exactness in the QL layer.
- Pysa is the strongest summary-first taint reference. A model contains returned
  sources, parameter-reached sinks, and taint-in-taint-out. It iterates a global
  fixpoint over callable models.
- Infer/Pulse is the strongest heap/effect summary reference. Summaries are
  pre/post abstract states with allocation, invalidation, resource, taint, and
  latent issue state.
- LLVM has the cleanest compact memory-effect lattice: mod/ref crossed with
  memory-location kind.
- MLIR has the cleanest resource-scoped effect interface: `Read`, `Write`,
  `Allocate`, and `Free` over resource hierarchies.
- Go `analysis` has the best official-language modular fact model: analyzer
  dependencies, package/object facts, and imported/exported facts.
- WALA and Soot show JVM library and read/write summaries tied to SSA,
  points-to, call graph, and native/reflection modeling.
- OPAL shows that effect properties work best as separate fixed-point facts,
  not one monolithic effect object.
- Heros and PhASAR show the classical IFDS/IDE interface boundary:
  normal/call/return/call-to-return flow functions, plus explicit summary-flow
  hooks in PhASAR.
- Semgrep shows the right UX pressure: users understand sources, sinks,
  sanitizers, propagators, exactness, and by-side-effect, but OSS Semgrep's
  default taint precision is mostly local compared to a full summary engine.

## The Product Shift For polint

Classic analyzers assume they must work as black boxes across arbitrary code.
polint has a different product path: the advanced user is an AI coding agent
that can inspect the repo, write Rust code, run fixtures, and improve the
analysis engine for that specific codebase.

That changes the design:

```text
generic static analyzer:
  "infer everything automatically, hide internals, keep config small"

polint:
  "provide sane defaults, expose typed extension points, validate repo-local
   summary providers, and make uncertainty actionable"
```

The engine should not try to auto-discover every framework endpoint or every
library flow perfectly by default. It should emit high-quality local summaries,
mark unknowns explicitly, and let an agent add repo-specific summary providers
where precision matters.

This is why a Rust-code extension surface matters. Configuration files are good
for simple source/sink entries. They are not enough for maximum capability:

- generated clients may need signature-aware summaries;
- custom framework dispatch may need repo-specific call edge creation;
- sanitizers may depend on predicates, type refinements, or guard dominance;
- wrappers may preserve value flow only under certain argument states;
- lifecycle models may create synthetic async/callback entrypoints;
- resource/typestate summaries may depend on project-specific state machines.

## Core Architecture Recommendation

Implement a summary kernel with typed domains.

```text
SummaryKernel
  - SummaryKey
  - SummaryStore
  - SummaryScheduler
  - ProvenanceStore
  - ValidationStore
  - ExtensionMerge
  - CacheInvalidation
  - Trace/Evidence

SummaryDomain
  - DomainId
  - Payload
  - bottom/top/join/less_equal/widen
  - transfer_local
  - apply_call_summary
  - validate_extension_payload
```

Do not expose raw summaries directly to normal rule authors first. Expose typed
SDK views after they are stable:

```rust
Effects<'_>
CallGraph<'_>
TaintFlows<'_>
ResourceFlows<'_>
ConcurrencyFacts<'_>
Aliases<'_>
```

Rules should ask these views questions. Extension providers should emit
validated summary facts.

## Required Summary Domains

### 1. ControlEffects

Control summaries capture how a callable exits and schedules work:

- normal return;
- no-return;
- throw/panic/reject;
- process exit;
- deferred cleanup/finally;
- async suspend/await/yield;
- callback invocation;
- callback stored for later;
- goroutine/thread/task/promise spawn.

Why first: control effects affect CFG, call graph, data flow, dead code,
resource cleanup, and diagnostics.

### 2. CallEffects

Call summaries capture what a callable may invoke:

- direct callee ids;
- method/interface dispatch candidates;
- function-valued parameter calls;
- callback/lambda invocation;
- framework synthetic calls;
- reflection/dynamic import/call edges;
- unresolved call placeholders.

Why first: call graph construction should not be one isolated pass. It should
consume type/value facts, framework facts, and summary facts.

### 3. DataFlowTito

Data-flow summaries capture value/taint movement through a callable:

- argument to return;
- receiver to return;
- argument to argument/receiver mutation;
- parameter to sink;
- source to return;
- global/captured value flow;
- barrier/sanitizer/guard effects;
- access paths and flow kind.

This domain should distinguish:

- value-preserving flow;
- taint-preserving flow;
- field/index access-path flow;
- by-side-effect flow;
- async/deferred flow;
- exceptional-return flow.

### 4. MemoryEffects

Memory summaries should copy LLVM/MLIR lessons:

```text
access kind: none | read | write | readwrite
resource: arg(i) | receiver | return | local | global | module | heap
          | file_system | network | env | process | database | time | unknown
```

This should not pretend to solve full aliasing. It should be a resource/mod-ref
lattice that alias/points-to can refine.

### 5. AliasEscapeEffects

Alias and escape summaries should say:

- parameter escapes to heap/global;
- parameter returned;
- parameter stored in receiver;
- callback stored;
- allocation returned;
- allocation captured;
- unknown external exposure.

This is the bridge between local allocation tokens and scalable alias queries.

### 6. ResourceEffects

Resource summaries capture typestate-relevant transitions:

- open/close;
- acquire/release;
- lock/unlock;
- allocate/free/drop;
- await/forget awaitable;
- transaction begin/commit/rollback;
- project-specific lifecycle transitions.

This should be extension-friendly because repo-specific resources are often the
most valuable custom policies.

### 7. TaintEffects

Taint summaries are a specialized domain over sources, sinks, sanitizers,
barriers, transforms, labels, and features/breadcrumbs.

Do not make taint a generic data-flow mode only. CodeQL and Pysa show that taint
needs domain-specific metadata, including source/sink kinds, sanitizer kinds,
trace features, and model provenance.

### 8. ExternalEffects

External effects are product-important because repo-local rules often ask
policy questions:

- can this handler write files?
- can this path execute shell?
- can this code send network requests?
- can this function log secrets?
- can this MCP/tool handler access external systems?

Model these as resource effects with domain-specific resources, not as ad hoc
string tags.

## Summary Key And Cache Inputs

A summary key must include more than function name.

```text
SummaryKey =
  subject_id
  + subject_signature_hash
  + language
  + package/module/source_set
  + provider_id/provider_version
  + domain_id/domain_version
  + context_key
  + source_digest
  + semantic_input_digest
  + config_digest
  + setup_digest
  + dependency_summary_digests
  + extension_model_digest
  + budget_digest
```

This is non-negotiable. If extension code, model packs, build tags, `go.mod`,
`tsconfig`, classpath, source-set roots, or dependency summaries change, cached
summaries may become unsound.

## Extension Merge Policy

Extensions must not silently override native facts.

Use explicit merge modes:

| Merge mode | Meaning |
|---|---|
| `augment` | Add extra possible behavior. Safe for may analyses. |
| `refine` | Narrow unknown/ambiguous behavior only if validation proves coverage. |
| `replace` | Replace a native summary only under strict trust and fixture gates. |
| `suppress` | Hide behavior only for diagnostics, never for internal may facts unless audited. |
| `conflict` | Native and extension facts disagree; emit model diagnostic. |

AI-authored summaries default to `DeclaredExternal` or `Heuristic`. They should
not claim `ExactSemantic` unless produced by a trusted native/official provider
or verified through a domain-specific proof/fixture contract.

## Scheduling And Recursion

Implement SCC fixed points from day one.

```python
def compute_summaries(call_graph, domain):
    for scc in reverse_topological_sccs(call_graph):
        if len(scc) == 1 and not self_recursive(scc[0]):
            summarize_once(scc[0], domain)
        else:
            iterate_scc_to_fixpoint(scc, domain)
```

Each domain must provide:

- `bottom`;
- `join`;
- `less_equal`;
- `widen`;
- `apply_call_summary`;
- budget collapse behavior;
- precision loss metadata.

If a recursive SCC exceeds budget, return a summary with `BudgetExceeded`, not
an empty summary.

## Accuracy And Complexity

There is no single best summary algorithm.

| Approach | Best for | Complexity shape | Main risk |
|---|---|---|---|
| Intraprocedural local summaries | cheap default effects | roughly linear in body CFG size | misses callees/frameworks. |
| Bottom-up SCC summaries | scalable may effects and TITO | sum of function transfer cost times SCC iterations | recursion and widening precision loss. |
| IFDS | finite distributive subset data-flow | classic bound `O(E D^3)` general case | only fits distributive finite domains. |
| IDE | value/environment transformers over IFDS | higher constant factors than IFDS | edge-function complexity and non-distributive domains. |
| WPDS | precise matched call/return pushdown queries | expensive but powerful for weighted domains | implementation complexity. |
| Demand-driven summaries | alias/path/rare queries | cost proportional to query frontier | cache/invalidation and repeated-query cost. |
| Abstract interpretation summaries | numeric, heap, typestate, resource | domain dependent; needs widening | false positives from widening/top. |
| Declarative model packs | library/framework APIs | cheap at runtime | model drift and precision claims. |

For polint, the winning strategy is hybrid:

1. Build cheap local summaries for every function.
2. Build bottom-up SCC summaries for high-value domains.
3. Use IFDS/IDE-style engines only for domains that fit the assumptions.
4. Add demand-driven refinement for expensive alias/path questions.
5. Let agents add repo-specific model providers when unknowns block precision.

## Rejected Paths

### Rejected: One Generic Summary Bag

This looks tempting:

```json
{
  "kind": "effect",
  "subject": "foo",
  "reads": ["x"],
  "writes": ["y"],
  "flows": [["arg0", "return"]],
  "tags": ["network"]
}
```

It fails because it erases domain semantics. A taint summary, a memory mod/ref
summary, and a resource typestate summary do not share the same lattice,
precision, conflict rules, or validation criteria.

### Rejected: One Giant Effect Enum

A global enum with every possible effect becomes language-biased and brittle:

```rust
enum Effect {
    ReadsFile,
    WritesFile,
    SendsHttp,
    MutatesArg,
    Throws,
    ...
}
```

It cannot represent access paths, conditionals, barriers, no-return calls,
callback scheduling, extension-specific resource state machines, or domain
versions cleanly.

### Rejected: Public Raw Summary API First

If normal rules consume raw summaries, internal representation freezes too
early. The supported rule surface should remain typed views with honest query
methods.

### Rejected: Whole-Program Alias/Flow Before Summaries

Whole-program analysis before summaries makes the first implementation slow and
fragile. Build summaries and unknown reporting first; then let alias/data-flow
engines refine them.

## Recommended First Vertical Slice

Build the kernel and four summary domains first:

1. `SummaryKey`, `SummaryStore`, `SummaryStatus`, `SummaryPrecision`,
   `SummaryProvenance`.
2. Local `ControlEffects` summary.
3. Local `CallEffects` summary over syntactic/direct calls and unresolved calls.
4. Local `MemoryEffects` summary over reads/writes to parameters, receiver,
   globals, module variables, and external resource calls.
5. Local `DataFlowTito` summary for obvious argument-to-return and
   argument-to-receiver mutations.
6. Extension summary sink with validation and fixture requirements.
7. SCC scheduler for summary closure over call edges.
8. SDK views that ask questions through `Effects<'_>` and `CallGraph<'_>`, not
   raw summary payloads.

For Go, use official Go tooling where it makes sense: `go/packages`,
`go/analysis`, and `buildssa` can act as provider inputs, normalized into
polint-owned facts.

For TS/JS, keep Oxc for syntax/scope/fast facts, and allow optional TypeScript
compiler sidecar facts for narrowing, assertion functions, `never`, `.d.ts`
summaries, and type predicates.

For Python, use CPython-compatible AST/typing semantics as official language
metadata, with third-party checkers as references/oracles rather than runtime
core dependencies.

For JVM later, normalize classpath/JDK/javac/JVM metadata into polint-owned
facts. WALA/Soot/OPAL are references, not dependencies.

## What This Enables

Once summaries exist, polint can answer questions that are currently too global:

```text
Can this request handler reach a shell execution?
Can tainted request data reach this generated client call?
Does this helper preserve validation before a sink?
Can this background job write user secrets to logs?
Does this service method mutate its receiver?
Can this function call a callback after the caller's resource is closed?
Does this path allocate a resource without release?
Can this API wrapper escape an internal token to global state?
```

Without summaries, these questions either require whole-program analysis or
become local heuristics. With summaries, they become cached, explainable,
extensible facts.

## Final Recommendation

Make summaries the next implementation foundation after the existing kernel,
CFG, type/value/alias, module graph, and evaluation research.

The design should optimize for:

- typed domains over generic bags;
- precision/status/provenance on every summary;
- SCC fixpoint and widening from day one;
- official language-tool inputs where they are the compatibility authority;
- Rust-code extension providers for maximum agent-authored capability;
- validation gates before extension summaries influence high-confidence results;
- public SDK views only after internal domains stabilize.

This aligns with the product goal: a native, multi-language static-analysis
engine where AI agents can add real semantic power, not just write lint rules
over local syntax.

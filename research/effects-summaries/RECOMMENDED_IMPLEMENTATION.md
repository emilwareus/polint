# Recommended Implementation Path

This is the concrete path for implementing function summaries and effects in
polint without depending on random external analysis libraries.

## Target Architecture

```text
source files
  -> parse/scope/symbol/reference facts
  -> CFG/place/type/value/call-site facts
  -> local summaries
  -> SCC summary closure
  -> typed SDK views
  -> rules and agent-authored extensions
```

Official language tooling may be used as provider input when it is the
compatibility authority: Go toolchain/x/tools, TypeScript compiler behavior,
JVM/JDK metadata, `javac`, CPython AST/typing metadata. Third-party OSS systems
such as CodeQL, Pyre, WALA, Soot, OPAL, Infer, Semgrep, and PhASAR should be
references and validation oracles, not runtime core dependencies.

## Phase 1: Summary Kernel

Add internal kernel types. Keep them outside the supported public SDK at first.

```rust
pub(crate) struct SummaryKey {
    pub subject: CallableId,
    pub signature_hash: Hash,
    pub language: LanguageId,
    pub package: PackageId,
    pub domain: SummaryDomainId,
    pub domain_version: DomainVersion,
    pub context: ContextKey,
    pub source_digest: Hash,
    pub semantic_input_digest: Hash,
    pub config_digest: Hash,
    pub setup_digest: Hash,
    pub dependency_summary_digest: Hash,
    pub extension_digest: Hash,
}

pub(crate) enum SummaryStatus {
    Complete,
    Incomplete,
    SetupMissing,
    Unsupported,
    Ambiguous,
    Unresolved,
    BudgetExceeded,
    Invalidated,
    Rejected,
}

pub(crate) enum SummaryPrecision {
    ExactSemantic,
    ExactLocal,
    ModuleLinked,
    SummaryBased,
    FrameworkModeled,
    Heuristic,
    DeclaredExternal,
    UnknownTop,
}

pub(crate) struct SummaryMeta {
    pub status: SummaryStatus,
    pub precision: SummaryPrecision,
    pub provenance: ProvenanceId,
    pub evidence: SmallVec<[EvidenceId; 2]>,
    pub validation: ValidationState,
}
```

Implement a `SummaryStore` as a typed internal store:

```rust
pub(crate) trait SummaryDomain: 'static {
    type Payload: Clone + Send + Sync;

    const ID: SummaryDomainId;
    const VERSION: DomainVersion;

    fn bottom() -> Self::Payload;
    fn unknown_top(reason: UnknownReason) -> Self::Payload;
    fn join(a: &Self::Payload, b: &Self::Payload) -> Self::Payload;
    fn less_equal(a: &Self::Payload, b: &Self::Payload) -> bool;
    fn widen(iteration: u32, previous: &Self::Payload, next: &Self::Payload) -> Self::Payload;
}
```

The store should reject a payload if:

- its key omits required digests;
- its domain version does not match;
- extension provenance lacks validation;
- it claims a precision level the provider is not allowed to claim;
- it conflicts with a higher-trust summary under the domain merge policy.

## Phase 2: Minimal Domains

Start with four domains. They feed existing future work directly.

### ControlEffects

```rust
pub(crate) struct ControlSummary {
    pub exits: ExitSet,
    pub async_kind: AsyncKind,
    pub cleanup: CleanupEffects,
}

bitflags! {
    pub(crate) struct ExitSet: u16 {
        const RETURNS = 1 << 0;
        const THROWS = 1 << 1;
        const PANICS = 1 << 2;
        const REJECTS = 1 << 3;
        const EXITS_PROCESS = 1 << 4;
        const DOES_NOT_RETURN = 1 << 5;
        const UNKNOWN = 1 << 6;
    }
}
```

First facts:

- explicit `return`;
- explicit `throw`/`raise`/`panic`;
- `process.exit`, `os.Exit`, known no-return calls;
- `await`, `yield`, `defer`, `finally`;
- callback invoked immediately versus stored for later when syntactically clear.

### CallEffects

```rust
pub(crate) struct CallSummary {
    pub calls: Vec<SummaryCallEdge>,
    pub unresolved: Vec<UnresolvedCall>,
    pub callback_uses: Vec<CallbackUse>,
}
```

Edges include:

- direct symbol callee;
- method receiver candidate;
- function-valued parameter call;
- framework synthetic target;
- unresolved dynamic target.

### MemoryEffects

```rust
pub(crate) enum AccessKind {
    None,
    Read,
    Write,
    ReadWrite,
}

pub(crate) enum Resource {
    Receiver,
    Param(u16),
    Return,
    Local,
    Global(SymbolId),
    Module(ModuleId),
    Heap(AbstractLocationId),
    FileSystem,
    Network,
    Database,
    Env,
    Process,
    Time,
    UnknownExternal,
}

pub(crate) struct MemorySummary {
    pub accesses: ResourceAccessSet,
}
```

This should copy LLVM/MLIR's product-lattice idea, but use polint resources.

### DataFlowTito

```rust
pub(crate) enum FlowKind {
    Value,
    Taint,
    BySideEffect,
    Barrier,
    Sanitizer,
}

pub(crate) struct FlowEndpoint {
    pub root: FlowRoot,
    pub path: AccessPath,
}

pub(crate) struct DataFlowSummary {
    pub edges: Vec<FlowEdge>,
    pub sources: Vec<TaintSourceDecl>,
    pub sinks: Vec<TaintSinkDecl>,
    pub sanitizers: Vec<SanitizerDecl>,
    pub barriers: Vec<BarrierDecl>,
}
```

First facts should be conservative:

- argument returned directly;
- receiver/argument mutation by direct assignment/update;
- source-like calls returned;
- sink-like calls consuming arguments when modeled;
- sanitizer-like calls only when modeled or obvious.

## Phase 3: Local Summary Builders

Create one local summary provider per language adapter. The provider reads
polint-owned facts, not raw parser internals where avoidable.

```python
def build_local_summary(fn, facts):
    cfg = facts.cfg(fn)
    places = facts.places(fn)
    calls = facts.calls(fn)

    control = ControlSummary.bottom()
    calls_summary = CallSummary.bottom()
    memory = MemorySummary.bottom()
    flow = DataFlowSummary.bottom()

    for op in cfg.ops():
        control = transfer_control(op, control)
        calls_summary = transfer_call(op, calls_summary, facts)
        memory = transfer_memory(op, memory, places)
        flow = transfer_local_flow(op, flow, places)

    return SummaryBundle(control, calls_summary, memory, flow)
```

This gives useful summaries before full interprocedural analysis exists.

## Phase 4: SCC Summary Scheduler

Add recursive summary closure early. Even a simple implementation should be
correct about recursion and cache invalidation.

```python
def close_domain(domain, call_graph):
    for scc in reverse_topological_sccs(call_graph):
        if not recursive(scc):
            fn = only(scc)
            store.put(analyze_with_callee_summaries(fn, domain))
            continue

        state = {fn: domain.bottom() for fn in scc}
        for iteration in range(MAX_ITERS):
            changed = False
            for fn in scc:
                next_summary = analyze_with_callee_summaries(fn, domain, state)
                widened = domain.widen(iteration, state[fn], next_summary)
                changed |= not domain.less_equal(widened, state[fn])
                state[fn] = domain.join(state[fn], widened)
            if not changed:
                break
        else:
            mark_budget_exceeded(state)

        store.put_many(state)
```

Always preserve `BudgetExceeded` metadata. Never collapse a failed fixpoint to
empty.

## Phase 5: Extension Provider Sink

Agent-authored Rust extensions should emit summary candidates through typed
sinks:

```rust
pub trait SummaryExtension {
    fn provide_summaries(&self, ctx: &mut SummaryExtensionCtx<'_>) -> RuleResult;
}

impl SummaryExtensionCtx<'_> {
    pub fn add_control_summary(&mut self, subject: CallableSelector, summary: ControlSummaryDecl);
    pub fn add_flow_summary(&mut self, subject: CallableSelector, summary: FlowSummaryDecl);
    pub fn add_memory_summary(&mut self, subject: CallableSelector, summary: MemorySummaryDecl);
}
```

Validation gates:

- selector resolves to existing symbol/callable or a declared synthetic subject;
- signature hash matches unless wildcard is explicitly allowed;
- access paths are valid for the subject signature;
- domain precision claim is permitted for the provider trust level;
- source/model spans are attached;
- fixture expectations exist for high-impact summaries;
- cache digest includes extension crate code, extension config, and model data.

Activation levels:

| Level | Allowed impact |
|---|---|
| `candidate` | Stored but not used by analysis. |
| `diagnostic_only` | Can explain unknowns but cannot alter facts. |
| `augment_may` | Can add possible behavior. |
| `refine_unknown` | Can refine unresolved/unknown facts if validated. |
| `trusted_replace` | Can replace native summaries only with strong validation. |

## Phase 6: SDK Views

Do not expose raw summary payloads as the normal rule API. Expose typed views:

```rust
impl Effects<'_> {
    pub fn may_write(&self, callable: CallableId, resource: ResourceSelector) -> QueryAnswer;
    pub fn may_call_external(&self, callable: CallableId, kind: ExternalKind) -> QueryAnswer;
    pub fn exits(&self, callable: CallableId) -> ExitSummaryView;
}

impl TaintFlows<'_> {
    pub fn parameter_flows_to_return(&self, callable: CallableId, param: ParamId) -> QueryAnswer;
    pub fn may_reach_sink(&self, source: SourceSelector, sink: SinkSelector) -> FlowQueryResult;
}
```

`QueryAnswer` should carry precision:

```rust
pub enum QueryAnswer {
    Yes(EvidenceId),
    No(EvidenceId),
    Maybe(EvidenceId),
    Unknown(UnknownReason),
}
```

This prevents rules from treating unknown as false.

## Language Provider Order

### Go First

Use:

- `go/packages` for package/module loading;
- `go/analysis` concepts for modular facts;
- `buildssa` for SSA-backed summaries;
- polint-owned normalized output.

Cache by:

- Go version;
- module roots;
- `go.mod`, `go.sum`, `go.work`;
- build tags;
- package patterns;
- include-tests setting;
- provider version.

### TS/JS Second

Use:

- Oxc for parser/scope/semantic/cfg defaults;
- optional TypeScript compiler sidecar for official checker facts:
  predicates, assertions, `never`, narrowing, `.d.ts` summaries.

Do not implement a partial TS checker in Rust for type-sensitive facts.
Normalize official checker output into polint facts.

### Python Third

Use:

- CPython-compatible AST shape and typing semantics;
- `TypeGuard`, `TypeIs`, `NoReturn`/`Never`, decorators, `ParamSpec`,
  async/generator facts;
- third-party checkers as references/oracles, not core dependencies.

### JVM Later

Use:

- JVM/JDK/classfile/javac metadata as official input;
- WALA/Soot/OPAL/Doop as design references;
- polint-owned facts for classpath, methods, bytecode/source mappings,
  synthetic methods, native/reflection summaries.

## Implementation Acceptance Criteria

Do not promote `Effects<'_>` or summary-backed SDK views until these pass:

- direct and recursive function summaries are stable across runs;
- unknown calls are represented as unknown, not false;
- extension-provided summaries are rejected on signature mismatch;
- cache invalidates on source/config/setup/extension/model changes;
- default-vs-agent-extended benchmark delta is reportable;
- diagnostics can explain which summary edge caused a result;
- widening/budget loss is visible in query output.

## First User-Visible Rule Examples

Good first rules:

- "No request handler may execute shell commands."
- "No secrets may flow to logs."
- "No API endpoint may write outside approved storage APIs."
- "No goroutine/task may capture a request-scoped resource after return."
- "This package boundary must not call database APIs directly."

These rules force summaries to exercise calls, external effects, taint,
framework entrypoints, and extension overlays without needing every future
abstract domain at once.

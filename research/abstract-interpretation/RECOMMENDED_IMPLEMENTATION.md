# Recommended Implementation Path

This is the concrete path for implementing abstract interpretation domains in
polint without depending on random external analysis libraries.

Official language tooling may be used as provider input when it is the
compatibility authority: Go toolchain/x/tools, TypeScript compiler behavior,
JVM/JDK metadata, `javac`, CPython metadata, Python packaging metadata, and
language specs. Third-party analyzers such as Infer, CodeQL, Pyright, WALA,
Soot, Semgrep, Goblint, IKOS, APRON, and ELINA should be references and
validation oracles, not runtime core dependencies.

## Phase 0: Ownership Boundaries

Do not let the abstract-interpretation work create a second copy of core engine
concepts.

| Concept | Owning Research Track | Abstract-Interpretation Role |
|---|---|---|
| `PlaceId`, access paths, allocation tokens | `research/type-alias-points-to/` | Consume and refine place/value facts. |
| Semantic operation IR and CFG locations | `research/cfg-control-flow/` plus type/value substrate | Define transfer semantics over shared operation IDs. |
| `SummaryKey`, summary store, SCC summary scheduling | `research/effects-summaries/` | Contribute domain payloads and projection/application logic. |
| Cache/invalidation graph | `research/analysis-kernel/` and future incremental research | Provide domain/reduction/widening/version inputs. |
| Extension registration and process policy | `research/agent-extension-surface/` | Register law-checked domain products and model providers. |

The domain layer should be a consumer and producer inside the shared kernel, not
a parallel mini-kernel.

## Phase 1: Internal Domain Kernel

Add an internal module. Keep it outside the stable SDK at first.

```rust
pub(crate) trait AbstractDomain: Clone + Send + Sync + 'static {
    const ID: DomainId;
    const VERSION: DomainVersion;

    fn bottom() -> Self;
    fn top(reason: TopReason) -> Self;
    fn is_bottom(&self) -> bool;
    fn is_top(&self) -> bool;

    fn leq(&self, other: &Self) -> bool;
    fn join(&self, other: &Self) -> Self;

    fn join_into(&mut self, incoming: &Self) -> Changed {
        let joined = self.join(incoming);
        if joined.equivalent(self) {
            Changed::No
        } else {
            *self = joined;
            Changed::Yes
        }
    }

    fn meet(&self, other: &Self) -> Option<Self> {
        None
    }

    fn widen(&self, next: &Self, site: WidenSite, fuel: WidenFuel) -> Self {
        self.join(next)
    }

    fn narrow(&self, next: &Self, site: WidenSite) -> Self {
        next.clone()
    }

    fn stable_digest(&self, hasher: &mut StableHasher);
}
```

Add a transfer trait that is separate from lattice operations:

```rust
pub(crate) trait TransferDomain: AbstractDomain {
    fn assign(&mut self, place: PlaceId, expr: ExprId, cx: &TransferCx<'_>);
    fn assume(&mut self, predicate: PredicateId, sense: BranchSense, cx: &TransferCx<'_>);
    fn call(&mut self, call: CallSiteId, summary: Option<&SummarySet>, cx: &TransferCx<'_>);
    fn forget(&mut self, target: ForgetTarget, reason: ForgetReason);
    fn project_summary(&self, subject: CallableId, cx: &SummaryCx<'_>) -> DomainSummary;
}
```

The split matters. Many domains share lattice operations but have different
language-specific transfer providers.

Solvers should call `join_into` or an equivalent mutation API, not open-coded
`leq` polarity checks. `leq` still exists for law tests, validation, and summary
conflict checks, but `join_into` is the guardrail that keeps fixpoint scheduling
correct when a domain uses a dual order, a compact bitset representation, or a
canonicalized value form.

## Phase 2: Semantic Operation Layer

Do not point domains at parser ASTs. Lower adapters into a MIR-like semantic
operation layer. The full contract is in `implementation/MIR-CONTRACT.md`. This
layer needs more structure than a flat list of statement kinds:

```rust
pub(crate) enum StatementKind {
    Assign { place: PlaceId, value: ValueExprId },
    Destructure { pattern: PatternId, value: ValueExprId },
    Read { place: PlaceId },
    Write { place: PlaceId, value: ValueExprId },
    Call { call: CallSiteId },
    Await { value: ValueExprId },
    Defer { call: CallSiteId },
    Acquire { resource: ResourceId },
    Release { resource: ResourceId },
}

pub(crate) enum TerminatorKind {
    Goto { target: BasicBlockId },
    Branch { predicate: PredicateId, then_bb: BasicBlockId, else_bb: BasicBlockId },
    Switch { discr: ValueExprId, targets: SwitchTargets },
    Return { value: Option<ValueExprId> },
    Throw { value: Option<ValueExprId> },
    Panic { value: Option<ValueExprId> },
    Unreachable,
}
```

Every operation should carry source span evidence, language id, original syntax
references, and unsupported-semantics markers. The IR contract must also model:

- expression/value facts for constants, literals, calls, allocations, and
  field/property/index reads;
- allocation IDs for object literals, closures, composite literals, class
  instances, arrays/slices/maps, and synthetic framework objects;
- phi/join-point identity so facts projected at joins are explainable;
- edge effects for short-circuiting, call-return, unwind/throw/panic, cleanup,
  `finally`, ordered Go `defer`, async rejection, and callback invocation;
- invalidation points for unknown calls, dynamic writes, alias escape, and
  reflection;
- explicit `Unsupported` facts where language semantics are not lowered yet.

Domains should consume operation IDs and facts, not raw AST pointers.

## Phase 3: Product State

Represent the local abstract state as a product of independently versioned
domain slots. Use the hybrid model defined in
`implementation/EXTENSION-DOMAIN-CONTRACT.md`: fixed core slots for built-in
P0/P1 domains, plus registry-backed extension slots for future law-checked
domain products.

```rust
pub(crate) struct ProductState {
    core: CoreDomains,
    extension_slots: ExtensionDomainSlots,
    meta: StateMeta,
}

pub(crate) struct CoreDomains {
    reachability: ReachabilityDomain,
    nilness: NilnessDomain,
    truthiness: TruthinessDomain,
    constants: ConstantsDomain,
    strings: StringDomain,
    ranges: IntervalDomain,
    initialized: InitDomain,
    shape: ShapeDomain,
    typestate: TypestateDomain,
    predicates: PathPredicateDomain,
}
```

Extension slots declare domain id/version, dependencies, transfer hooks, summary
payload schema, merge policy, validation record, and cache identity.

Reductions should be explicit, fuel-bounded, and scheduled through a versioned
dependency graph:

```rust
fn reduce(state: &mut ProductState, cx: &ReductionCx<'_>) {
    for round in 0..cx.max_reduction_rounds {
        let mut changed = Changed::No;
        for reduction in cx.reduction_graph.stable_order() {
            changed |= reduction.apply_value_only(state, cx, round);
        }
        if changed == Changed::No {
            break;
        }
    }
}
```

Reduction contracts must be monotone/reductive for the affected product order.
Reduction order, dependency graph version, and fuel policy are cache inputs.
Provenance changes are recorded separately so metadata does not force expensive
full-state hashing during every reduction round.

## Phase 4: Solver

Copy the useful shape of rustc MIR dataflow:

- deterministic worklist;
- stable block order;
- early and primary effects;
- edge-specific effects;
- call-return effects distinct from call/unwind effects;
- `ResultsCursor` for specific locations;
- `ResultsVisitor` for full traversal.

```python
def solve(cfg, product):
    entry = {bb: product.bottom() for bb in cfg.blocks}
    entry[cfg.start] = product.initial()
    queue = reverse_postorder(cfg)

    while queue:
        bb = queue.pop()
        state = entry[bb].copy()

        for op in cfg.block_ops(bb):
            transfer(op, state)
            reduce(state)

        for edge in cfg.successors(bb):
            out = apply_edge_effect(edge, state.copy())
            candidate = out

            if edge.dst in cfg.widen_points:
                candidate = entry[edge.dst].widen(candidate, edge.dst)

            if entry[edge.dst].join_into(candidate):
                queue.push(edge.dst)

    return Results(entry)
```

Use weak topological order or SCC order once recursive loops and summary SCCs
become common.

## Phase 5: First Domains

### Reachability

```rust
enum Reachability {
    Unreachable,
    Reachable,
    Ambiguous,
}
```

This domain feeds dead-code, branch feasibility, and result precision.

### Nilness / Nullish

```rust
enum Nilness {
    Bottom,
    Nil,
    NonNil,
    MaybeNil,
    Unknown,
}
```

Use language-specific constructors:

- Go: `nil`.
- TS/JS: `null`, `undefined`, nullish and non-nullish.
- Python: `None`.
- JVM: `null`.

### Truthiness

```rust
enum Truthiness {
    Bottom,
    Truthy,
    Falsy,
    Maybe,
    Unknown,
}
```

The reduction from constants is language-specific because JS and Python
truthiness differ.

### Constants

```rust
enum ConstValueSet {
    Bottom,
    Values(SmallVec<[LiteralId; 8]>),
    Top,
}
```

Widen capped literal sets to the base primitive domain after a threshold or loop.

## Phase 6: P1 Domains

Add in this order:

1. `StringValues`: literal set, template fragments, length interval, prefix/suffix.
2. `Initializedness`: maybe-initialized and maybe-uninitialized bitsets over interned places.
3. `NumericRanges`: intervals over selected scalar places.
4. `Shape`: object/record/property/TypedDict presence and exactness.
5. `Typestate`: finite state machines over abstract resources/objects.

Do not expose these to public SDK until each has:

- fact docs;
- precision limits;
- inline fact fixtures;
- cache digest tests;
- extension merge rules;
- diagnostic examples.

## Phase 7: Summaries

Each domain must project a summary payload into the shared summary kernel from
`research/effects-summaries/`. Use the algebra in
`implementation/SUMMARY-ALGEBRA.md`. A minimal domain payload structure:

```rust
pub(crate) struct DomainSummary {
    pub domain: DomainId,
    pub version: DomainVersion,
    pub requires: Vec<DomainRequirement>,
    pub ensures: Vec<DomainEnsure>,
    pub returns: Vec<DomainReturnFact>,
    pub throws: Vec<DomainThrowFact>,
    pub modifies: Vec<Invalidation>,
    pub invalidates: Vec<Invalidation>,
    pub flows: Vec<TitoFlow>,
    pub guard_refinements: Vec<GuardRefinement>,
    pub typestate_transitions: Vec<TypestateTransition>,
    pub unknowns: Vec<UnknownFact>,
    pub diagnostics: Vec<LatentDiagnostic>,
    pub precision: Precision,
    pub status: SummaryStatus,
    pub provenance: ProvenanceId,
    pub dependencies: Vec<SummaryDependency>,
    pub digest: Hash,
}
```

The full `SummaryKey` belongs to the summary kernel and includes callable
identity, package/language, signature hash, context key, semantic input digest,
domain version, reduction graph version, config/setup digests, extension
digests, and dependent summary digests.

Each domain summary defines partial order, `join_into`, widening for recursive
SCCs, caller-place substitution, call application transfer, unknown/havoc
behavior, conflict policy, and context sensitivity.

Summaries should be the only way interprocedural domain facts cross function
boundaries. Extensions may provide summaries, but the kernel validates and
merges them.

## Phase 8: Agent-Authored Extensions

Expose extensions as registered analysis products:

```rust
pub trait DomainExtension {
    fn manifest(&self) -> ExtensionManifest;
    fn register(&self, registry: &mut ExtensionRegistry);
}

pub trait GuardModel {
    fn refine_guard(&self, guard: GuardCx<'_>, sink: &mut RefinementSink<'_>);
}

pub trait TypestateModel {
    fn machine(&self) -> TypestateMachine;
}

pub trait SummaryModel {
    fn summarize(&self, item: SummaryItem<'_>, sink: &mut SummarySink<'_>);
}
```

Extensions must not mutate stores, caches, parser ASTs, or diagnostics directly.
They emit typed products into kernel-owned sinks.

There are two extension classes:

1. **Model extensions** emit guard refinements, summaries, typestate machines,
   reducers, or selected invariants for built-in domains.
2. **Domain extensions** register new law-checked domain slots with explicit
   dependencies and summary payloads.

Support model extensions first. Domain extensions need registry-backed product
state, domain law tests, summary algebra, cache identity, deterministic
scheduling, and execution isolation before public use.

Validation must include:

- lattice law tests for new domains;
- transfer monotonicity samples;
- deterministic output across worker counts;
- cache-key coverage;
- conflict/merge policy checks;
- suppressive-model review gates.

Validation is not enough for arbitrary Rust execution. Untrusted repo-local
Rust should run out of process with a narrow protocol: the kernel sends
read-only semantic snapshots and receives canonical fact batches. In-process
native Rust is acceptable for built-ins and explicitly trusted workspace
extensions.

## Phase 9: SDK Views

Expose only typed views:

```rust
Nilness<'_>
Truthiness<'_>
Constants<'_>
StringValues<'_>
NumericRanges<'_>
Initializedness<'_>
Shapes<'_>
Typestate<'_>
PathPredicates<'_>
```

Rule authors should not receive mutable states or raw lattice internals. SDK
views should expose query methods:

```rust
nilness.of_place(place)
constants.of_expr(expr)
strings.possible_literals(expr)
ranges.interval(place)
typestate.state(resource)
predicates.guard_evidence(branch)
```

## First Vertical Slice

The first implementation slice should target Go and TS/JS because those adapters
already exist:

1. Add semantic operation facts for assignment, branch, call, return, throw/panic.
2. Add places/access paths for locals, params, receiver, fields/properties, indexes.
3. Implement reachability, nilness/nullish, truthiness, constants.
4. Add inline fixtures with `polint-expect` fact assertions.
5. Add `Nilness<'_>` and `Constants<'_>` internal views, then decide when to expose.
6. Add direct/syntactic call facts and context-insensitive direct summaries.
7. Add minimal cache keys for domain versions, reduction graph, lifecycle setup,
   summary inputs, and extension manifests.
8. Add one subprocess-style extension fixture that models a project-specific
   guard function through a typed sink.

This gives immediate rule value while keeping the architecture open for the
larger engine.

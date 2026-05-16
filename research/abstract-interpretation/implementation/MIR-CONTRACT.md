# Semantic MIR Contract For Abstract Domains

Abstract domains should not run over parser ASTs. They should run over a
language-normalized, MIR-like operation layer that is shared with CFG,
type/value/place, summaries, call graph, and data-flow work.

This file defines the minimum contract needed before P0 domains depend on the
IR.

## Core IDs

```rust
BodyId
BasicBlockId
StatementId
TerminatorId
EdgeId
ValueExprId
PredicateId
PlaceId
AllocId
PatternId
UnsupportedId
```

Every item carries:

- language id;
- package/module/function owner;
- stable source span and original syntax evidence;
- semantic schema version;
- precision/status metadata;
- unsupported-semantics payloads when lowering is incomplete.

Domains consume these IDs plus fact tables. They do not read parser AST nodes
directly.

## Body Shape

```text
Body =
  stable body id
  + ordered basic blocks
  + entry block
  + exit/unwind/synthetic cleanup blocks
  + local/place table
  + expression fact table
  + allocation table
  + unsupported-semantics table
```

A basic block contains ordered side-effect statements and exactly one
terminator.

## Statements

Statements model local effects that do not choose successors.

```rust
pub(crate) enum StatementKind {
    StorageLive { place: PlaceId },
    StorageDead { place: PlaceId },
    Bind { place: PlaceId, value: ValueExprId },
    Assign { place: PlaceId, value: ValueExprId, mode: AssignMode },
    Destructure { pattern: PatternId, value: ValueExprId },
    Read { place: PlaceId },
    Write { place: PlaceId, value: ValueExprId },
    Alloc { place: PlaceId, alloc: AllocId },
    MakeClosure { place: PlaceId, closure: AllocId },
    Capture { closure: AllocId, captured: PlaceId },
    Forget { target: ForgetTarget, reason: ForgetReason },
    Acquire { resource: ResourceId },
    Release { resource: ResourceId },
}
```

Assignment must distinguish:

- declaration binding;
- overwrite;
- partial write;
- simultaneous assignment;
- mutation through alias/projection.

Calls that may return, throw, panic, reject, suspend, or run cleanup should be
terminators, not plain statements.

## Terminators

Terminators own successor edges and edge-specific effects.

```rust
pub(crate) enum TerminatorKind {
    Goto { target: BasicBlockId },
    Branch { predicate: PredicateId, then_bb: BasicBlockId, else_bb: BasicBlockId },
    Switch { discr: ValueExprId, targets: SwitchTargets },
    Call { call: CallSiteId, outcomes: CallOutcomes },
    Return { value: Option<ValueExprId> },
    Throw { value: Option<ValueExprId> },
    Panic { value: Option<ValueExprId> },
    Await { value: ValueExprId, outcomes: AwaitOutcomes },
    Yield { value: Option<ValueExprId>, resume: BasicBlockId },
    RunCleanup { cleanup: CleanupId, next: CleanupNext },
    Unreachable,
}
```

Call terminators must separate callee/argument evaluation from success,
exceptional, panic, async-reject, and unknown-target outcomes. A return summary
must not be applied to an unwind edge.

## Edge Effects

Required edge kinds:

```text
normal
true / false
switch case / default
call-return
unwind / exception
panic
recover
async-fulfill / async-reject
suspend / resume
finally-enter / finally-exit
defer-run
callback-invoke
callback-stored
loop-back
```

Each edge can carry:

- assumptions and predicate refinements;
- return/temp initialization;
- invalidations and havoc actions;
- typestate transitions;
- cleanup effects;
- summary application;
- unsupported facts.

## Expression Facts

Expressions lower into stable facts:

```text
literal
place read
allocation
unary/binary operation
comparison
property key
index key
call target
predicate
type/test predicate
may throw
may allocate
may mutate
may invoke user code
may suspend
```

Expression facts need evaluation-order metadata. This is required for JS/Python
side effects, Go multi-assign, destructuring defaults, short-circuiting, and
cleanup paths.

Predicates should expose affected places and candidate refinements, not just an
opaque predicate id.

## Allocation IDs

`AllocId` must identify allocation site plus kind:

```text
object
array/slice/map
closure/env
class/instance
resource
promise/future
iterator
channel
error/exception
synthetic framework object
unknown
```

Each allocation carries a freshness policy:

- per allocation site;
- per call summary;
- per loop widened allocation;
- escaped allocation;
- unknown allocation.

This policy is a cache and summary input.

## Destructuring

Use explicit `Pattern` and `Destructure` facts for:

- nested fields/properties;
- array/list/tuple slots;
- rest/spread/starred entries;
- defaults;
- renames;
- ignored slots;
- computed keys.

Language differences matter:

- Go multi-assign is simultaneous.
- JS default initializers run only for `undefined`; object/array destructuring
  may trigger getters and iterators.
- Python unpacking can raise length or iterator errors.

Unsupported destructuring should conservatively invalidate affected places and
emit an unsupported fact.

## Short-Circuiting

Lower `&&`, `||`, `??`, optional chaining, ternary, Python `and/or`, and guard
expressions into CFG blocks with value temps and join points.

RHS evaluation must be edge-guarded. Side effects on skipped branches must not
appear in the skipped path.

JS and Python `and/or` return operand values, not booleans. Constants,
truthiness, and nullish domains must see that value flow.

Optional chaining should be modeled as short-circuit expression flow. It should
not be described as a general persistent variable narrowing unless the specific
language semantics and scope justify that refinement.

## Async, Callback, Defer, Finally, Exceptions

The MIR must make these schedulable:

- Go `defer`: arguments evaluate immediately; deferred calls run LIFO on return
  and panic; `recover` only applies on panic edges inside deferred calls.
- JS/TS `await`: normal fulfillment and rejection edges; async function throw
  becomes rejection.
- JS/TS callbacks: distinguish definitely invoked, maybe invoked, stored,
  escaped, framework-scheduled, and unknown.
- Python `finally` and context managers: cleanup runs on return, break,
  continue, and exception; `__exit__` can suppress exceptions.
- JVM/Java: exceptions can arise from more than calls; finally and
  try-with-resources may throw and suppress.

## Unsupported Semantics

Unsupported facts should include:

```text
construct
affected places
affected domains
conservative action
precision/status downgrade
evidence
```

Important dynamic hooks:

- JS getters/setters, proxy, `eval`, `with`;
- Python descriptors, metaclasses, `exec`, `eval`;
- Go reflection, `unsafe`, goroutines, channels;
- JVM reflection, native methods, `invokedynamic`.

Validation fixtures should assert MIR shape, not only final domain facts. That
catches lowering bugs before domains hide them with `top`.

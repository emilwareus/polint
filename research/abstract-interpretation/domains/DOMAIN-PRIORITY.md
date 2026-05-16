# Domain Priority And Precision Plan

## Priority Matrix

| Domain | Priority | Default? | Summary? | Extension Value | Notes |
|---|---:|---:|---:|---:|---|
| Reachability | P0 | yes | yes | medium | Required by every path-sensitive fact. |
| Nilness/nullish | P0 | yes | yes | high | Guards and repo APIs can refine this heavily. |
| Truthiness | P0 | yes | yes | high | Required for TS/JS/Python precision. |
| Constants/literals | P0 | yes | yes | high | Feeds routes, env keys, feature flags, string APIs. |
| String values/templates | P1 | yes | yes | high | Very useful for policies; must cap and widen. |
| Initializedness | P1 | yes | local/summary | medium | Bitset-friendly and high signal. |
| Numeric intervals | P1 | yes | yes | medium | Cheap numeric facts. |
| Shape/property presence | P1 | yes | yes | high | JS/Python/framework-heavy repos need this. |
| Typestate/resource | P1 | capability | yes | very high | Best match for repo-specific extensions. |
| Congruence | P2 | selected | yes | medium | Useful for modulo/parity/index checks. |
| Packed DBM/octagon | P2 | selected | yes | medium | First relational numeric precision tier. |
| Path predicates/trace partitions | P2 | selected | selected | very high | Agent-provided guards drive value. |
| Polyhedra | P3 | no | selected | low/medium | Expert precision mode only. |
| SMT/path focusing | P3 | no | no | medium | Diagnostic refinement, not default analysis. |

## P0 Domains

### Reachability

Use a three or four point domain:

```text
Unreachable < Reachable
Unreachable < Ambiguous
Reachable join Ambiguous = Ambiguous
```

Expose why a path is ambiguous: unsupported predicate, unknown constant,
budgeted partition, missing summary, or setup gap.

### Nilness / Nullish

Language mapping:

| Language | Values |
|---|---|
| Go | nil, non-nil, maybe nil |
| TS/JS | null, undefined, nullish, non-nullish, maybe |
| Python | None, non-None, maybe |
| JVM | null, non-null, maybe |

Start with local guards, direct assignments, constants, and known allocation
constructs. Add summary returns and extension guard models next.

### Truthiness

Truthiness must be language-specific:

- JS: `false`, `0`, `-0`, `0n`, `NaN`, `""`, `null`, `undefined` are falsy.
- Python: `False`, `0`, empty containers/strings, `None`, and user-defined
  `__bool__`/`__len__` semantics complicate precision.
- Go has no generalized truthiness.

Treat user-defined dynamic truthiness as heuristic unless modeled.

### Constants

Use capped literal sets:

```text
Bottom -> {literal... max N} -> Top
```

Recommended cap: 8 by default, per-domain configurable internally. Widen at
loop headers and when string/numeric computations exceed cheap transfer.

## P1 Domains

### StringValues

Payload:

```text
literal set
template with unknown holes
prefix/suffix
length interval
classification tags: path-like, url-like, identifier-like, sql-like
```

Classifications are heuristic unless provided by explicit models.

### Initializedness

Copy rustc's lesson:

- intern places/access paths;
- track `MaybeInitialized`;
- track `MaybeUninitialized`;
- answer definite queries through complements/intersections.

This handles assignment, move-like invalidation, use-before-def, and resource
initialization policies.

### NumericRanges

Start with intervals. Add thresholds from guards:

```text
if i < len(xs): i.upper <= len(xs)-1
if n >= 0: n.lower >= 0
```

Represent overflow semantics per language. Do not use one numeric semantics for
Go, JS, Python, and JVM.

### Shape

Payload:

```text
object id / allocation id
exact or inexact
property -> present / absent / maybe
property -> value summary
dynamic write status
escape status
```

Precision policy:

- object literals and TypedDicts can start exact;
- spreads, dynamic writes, prototype mutation, reflection, and unknown calls
  degrade to inexact or unknown;
- discriminant properties should feed trace partitions.

### Typestate / Resource

Payload:

```text
resource id
state machine id
current possible states
required final states
transitions with evidence
ownership/escape status
```

This should be extension-first. Built-ins can cover common open/close,
lock/unlock, builder, transaction, and start/stop shapes, but repo-specific
state machines are where polint can outperform generic tools.

## P2 Domains

### Congruence

Track facts like:

```text
x = a mod m
```

Useful for parity, modulo indexing, generated ids, and bitmask-like code.

### Packed Octagons

Use small variable packs selected by:

- loop guards;
- array/string index relationships;
- rule-requested variables;
- extension-provided invariants.

Do not run globally. Emit `BudgetExceeded` or `UnknownTop` facts when a pack is
too large.

### PathPredicates

Store selected path facts:

```text
predicate id
branch sense
places affected
source span
partition priority
precision
```

This domain should be the bridge between agent-authored guard models and native
narrowing.

## P3 Domains

Polyhedra and SMT/path focusing should be researched again only after P0-P2 are
implemented and measured. They are not first-year default domains.

## Public SDK Timing

Do not expose every domain immediately. Suggested exposure order:

1. `Nilness<'_>`;
2. `Constants<'_>`;
3. `StringValues<'_>`;
4. `Initializedness<'_>`;
5. `NumericRanges<'_>`;
6. `Shapes<'_>`;
7. `Typestate<'_>`;
8. `PathPredicates<'_>`.

Each view must document exactness, unsupported constructs, and setup
requirements.

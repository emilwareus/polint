# TypeScript / JavaScript Abstract Domains

## State Of The Art Pattern

TypeScript and Pyright-style code-flow engines show that most precision comes
from local flow nodes and narrowing, not from whole-program symbolic execution.
TAJS shows what a full JS abstract interpreter can do, but it is too heavy for
polint's first native domain layer.

Oxc should remain the fast parser/semantic substrate. polint should add its own
domains on top.

## Recommended Product Domain

```text
JsState =
  PrimitiveKind
  x Nullish
  x Truthiness
  x ConstLiteralSet
  x StringDomain
  x ShapeDomain
  x EffectDomain
  x TargetSet
```

Keep this as a reduced product. Do not try to clone TypeScript's full type
checker or TAJS's complete heap/value lattice first.

## Key Narrowing Transfers

Support:

- `x == null`, `x != null`, `x === null`, `x === undefined`;
- `typeof x === "string"` and related primitive checks;
- truthiness and falsiness;
- discriminant property equality;
- `prop in obj`;
- optional chaining short-circuit expression flow;
- literal equality and switch cases;
- array length and string length where literal or interval facts exist.

Pseudo-code:

```python
def narrow_js(state, expr, sense):
    if expr == Eq(x, Null):
        return state.nilness.refine(x, "nullish" if sense else "non_nullish")
    if expr == TypeofEq(x, kind):
        return state.primitive.refine(x, kind, sense)
    if expr == StrictEq(x, literal):
        return state.constants.refine_literal(x, literal, sense)
    if expr == In(prop, obj):
        return state.shape.refine_property(obj, prop, present=sense)
    if expr == Ident(x):
        return state.truthiness.refine(x, truthy=sense)
    return state
```

Optional chaining should not be treated as a general persistent variable
narrowing rule. It should lower to edge-guarded expression flow with a
nullish/undefined result path and a non-nullish receiver path scoped to that
expression unless a stronger TypeScript-compatible refinement is justified.

## Shape Precision

Object shape should track:

- object literal allocation id;
- exact/inexact status;
- property presence;
- property value summaries;
- dynamic write status;
- escape status.

Degrade shape precision on:

- dynamic property writes;
- spread from unknown object;
- prototype mutation;
- unknown calls that may mutate object;
- alias escape beyond budget.

## Strings

String facts are high-value for polint:

- routes;
- SQL fragments;
- env/config keys;
- feature flags;
- file paths;
- generated API names.

Start with capped literal sets, template pieces, prefix/suffix, and length
intervals. Automata/regex domains are future work.

## Effects

Track coarse effects:

- may throw;
- may return Promise;
- async rejection path;
- callback stored vs invoked immediately;
- unknown external call.

This should feed summaries and CFG abrupt edges.

## Precision Labels

Important labels:

- TypeScript annotations: `DeclaredExternal` or `DeclaredType`, not runtime exact.
- Oxc syntactic facts: `ExactLocal` where purely syntactic.
- Dynamic property/reflection facts: `Heuristic` or `UnknownTop`.
- Extension guard facts: `DeclaredExternal` unless validated by fixtures.

## First TS/JS Slice

1. Branch narrowing for nullish/truthiness/constants.
2. Literal string and template facts.
3. Object-literal shape with dynamic write invalidation.
4. Summary return facts for direct functions.
5. Extension guard model for a project-specific `isX` predicate.

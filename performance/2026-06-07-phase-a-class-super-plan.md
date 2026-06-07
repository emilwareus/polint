# Phase A Implementation Plan: class/`super` completion — 2026-06-07

Starting checkpoint (reproduced in this workspace): **821 / 82 / 658, F1 68.94%**,
precision 90.92%.

## Goal

Recover the `super4` / `super5` / `classes.json` FN by making **class expressions**
(named or returned-from-function) first-class flowable constructor values, then
reusing the iteration-40 `this`/`super`/`caller_override` machinery to resolve
their internal call edges.

Per-case baseline (the recall headroom):

| Case | TP | FP | FN |
|---|---:|---:|---:|
| super4 | 2 | 0 | 16 |
| super5 | 2 | 0 | 10 |
| classes.json | 62 | 4 | 15 |

## How Jelly does it (research summary)

- A class (declaration or expression) is **not** a distinct value type — it is the
  **constructor `FunctionToken`**. `astvisitor.ts:554-607` maps `class2constructor`,
  then `ct ∈ ⟦class … {}⟧` (the class-expression node var) and `ct ∈ ⟦C⟧` for named.
  So a `class extends A` returned from a function flows through the return var, and
  `var a = f(); new a()` resolves because `⟦ret_f⟧ ⊆ ⟦new a()⟧`.
- `new X()` (`operations.ts:418-430`): if `X` → FunctionToken `t`, allocate an
  `ObjectToken q`, bind `q ∈ ⟦this_t⟧`, and inherit `t.prototype`. Instance methods
  live on the prototype; static members on the constructor FunctionToken.
- Instance properties are **flow-insensitive**: `this.p = …` in *any* method/ctor/
  field flows to every instance (`addForAllTokensConstraint(thisVar(constr), …)`).
  This is why super5's `c.www()` resolves even though `m()` is never called.
- `super` (`astvisitor.ts:166-196`) resolves through the prototype chain of the
  enclosing class's constructor; inside nested arrows `super` is captured lexically.
- Caller attribution: constructor / field-init / static-block calls are attributed
  to the constructor's `FunctionInfo`, whose **location is the class node span** for
  class expressions. (Confirmed by super5 oracle: constructor-body IIFE edge has the
  class-expression span `0:11:12:18:6` as caller — exactly polint's iter-40
  `caller_override = class node`.)

## polint today (the gap)

- `self.classes: BTreeMap<name, ClassTargets>` is populated only from top-level
  `ClassDeclaration`, `var x = class`, and `A.prototype`-style functions.
- Class **expressions returned from a function** never enter `self.classes`; their
  class-node + method `FunctionFact`s are never emitted (frontend
  `extract_anonymous_callables_from_class` walks bodies for nested arrows only).
- `new a()` only resolves when `a` is a literal class **name** (`callee_identifier`
  → `self.classes.get(name)`), never a variable bound to a returned class.
- `collect_class_body_call_flows` walks only top-level `ClassDeclaration` bodies.

## Slices (each measured independently)

### A1 — frontend: class-expression FunctionFacts (dedup-safe)
`crates/polint/src/ts/adapter.rs`. Make `push_ts_class` **idempotent by class span**
(skip if a FunctionFact already exists at that span), then call it for class
expressions in `extract_anonymous_callables_from_expression`'s `ClassExpression` arm
and for nested `ClassDeclaration`s in `extract_anonymous_callables_from_statement`.
Top-level decls / `var x = class` keep emitting via existing paths; the idempotency
guard prevents duplicate facts (which would be FP graph nodes).
*Expected on its own:* ~0 benchmark movement (facts without flow), enables A2.

### A2 — value-flow: walk class-expression bodies with `this`/`super`
`ts_value_flows.rs`. During `collect_expression_function_flows`, register every class
expression into `self.classes` under a span-derived key and record `(key, &Class)`.
Extend `collect_class_body_call_flows` to also walk those via
`collect_class_method_bodies` (this/super bound, `caller_override = class node`).
*Expected:* the internal `super.m()`/`super.s()`/`this.*` and constructor-body IIFE
edges in super4/super5 (the bulk). Hold precision ≥ 90%.

### A3 — value-flow: flow returned class to `new a()` / `a.s()` / `x.m()`
Add `class_bindings: BTreeMap<var, classKey>` to `FlowEnv`. A `class_key_from_expression`
resolves a class key from a class expression, an identifier already bound, or a call
to a local function whose body returns a class expression. Bind `var a = f()` →
class key (+ seed `env.objects[a] = static targets`); resolve `new a()` through the
binding to instance targets (+ emit the `new a()` → constructor `call2fun` edge);
bind `x = new a()` → instance. Collect `this.x = …` assignments flow-insensitively
from **all** methods (not just the constructor) so super5's `c.www()` resolves.

### A4 — classes.json remainder (15 FN)
Investigate after A1–A3; per roadmap: residual super/prototype/object-return and
call-result flow on inherited members (`k1.a4().a2()`-style, `A.prototype.s2`).

## Guardrails
- Hold precision ≥ 90% (reachability-pruned oracle ⇒ new FP are real).
- Fixture-first: add focused real-kernel regression tests before the benchmark is
  the bar; revert any slice that does not move the benchmark.
- Run the full `cargo test -p polint` lib suite for snapshot regressions (A1 shifts
  function IDs in files containing class expressions).
- Log TP/FP/FN/precision/recall/F1/runtime/hash per slice in
  `performance/2026-06-06-jelly-gap-closure-research.md` (iterations 41+).

# Jelly Gap Closure Research - 2026-06-06

## TLDR

The gap is not a tuning problem. Jelly is a whole-program JS/TS call graph
analyzer built around module execution nodes, function-object points-to flow,
object/prototype constraints, native models, and recovery passes. polint has
many of the backend building blocks, but the TS/JS frontend is not feeding them
the right facts yet.

Close the gap in this order:

1. Add first-class TS/JS module execution owners and module-level MIR bodies.
2. Model every function, arrow, method, constructor, and class as a callable
   object value that can flow through places.
3. Feed the existing points-to solver with object property, prototype, `this`,
   `new`, and class constraints.
4. Add CommonJS/ESM module graph modeling.
5. Add ECMAScript native callback models for arrays, promises, iterators, and
   `Function.prototype.call/apply/bind`.
6. Add Jelly-style recovery passes only after the core model works.

The first two steps should move Jelly recall from near-zero to visibly useful.
Matching Jelly closely requires all six.

## Current Evidence

Measured on 2026-06-06 after the Go RTA benchmark fix:

| Suite | TP | FP | FN | Precision | Recall | F1 |
|---|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% |
| Jelly JS/TS callgraph micro | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% |

Worst Jelly misses:

| Case | Expected | Observed | TP | FN | Dominant missing capability |
|---|---:|---:|---:|---:|---|
| `tests/helloworld/app.json` | 342 | 0 | 0 | 342 | CommonJS module graph and dependency execution |
| `tests/micro/classes.json` | 77 | 8 | 4 | 73 | class/object/prototype/function-object flow |
| `tests/micro/classes2.json` | 76 | 0 | 0 | 76 | class/object/prototype/function-object flow |
| `tests/micro/iterators.json` | 65 | 0 | 0 | 65 | native iterator and callback models |
| `tests/micro/promises.json` | 56 | 0 | 0 | 56 | Promise executor/then/catch/finally models |
| `tests/micro/call-expressions.json` | 45 | 0 | 0 | 45 | module-level calls, IIFEs, function-object calls |
| `tests/micro/fun.json` | 45 | 0 | 0 | 45 | function values and assignment flow |
| `tests/micro/rest.json` | 44 | 0 | 0 | 44 | argument/rest/spread flow |

The smallest smoking gun is `tests/micro/call-expressions.json`: Jelly expects
45 graph edges and polint emits none. That fixture is mostly top-level direct
calls, IIFEs, method calls, optional calls, and constructors. This proves the
first bottleneck is not an exotic JavaScript feature. It is that top-level
program execution is not represented as a call graph owner in polint.

## What Jelly Is Optimized Around

The pinned Jelly benchmark repo is cloned at:

- `research/evaluation-harness/repos/jelly-research`
- Commit `b799ed4f0d68c670fe398830aaa51dd5c628cf74`

Jelly's public README describes its core as flow-insensitive control-flow and
points-to analysis with access paths, plus ECMAScript standard-library models.
It is explicitly designed for Node.js JS/TS call graph construction, library
usage matching, and vulnerability exposure analysis. Source:
https://github.com/cs-au-dk/jelly

The implementation follows that description:

- `src/analysis/analyzer.ts` assigns each module a whole-program location,
  preprocesses the AST, visits it with `Operations`, propagates constraints to a
  fixpoint, applies patching passes, and finalizes call edges.
- `src/output/analysisstatereporter.ts` exports both `FunctionInfo` and
  `ModuleInfo` in the `functions` array, so modules are call graph nodes.
- `src/analysis/operations.ts` registers each call with an enclosing
  `FunctionInfo` or `ModuleInfo`, reads method targets through object-property
  variables, and turns resolved function tokens into `call2fun` and `fun2fun`
  edges.
- `src/natives/ecmascript.ts` and `src/natives/nativehelpers.ts` model large
  parts of ECMAScript: arrays, promises, iterators, `call`, `apply`, `bind`,
  `Object.create`, `Object.defineProperty`, prototypes, and callbacks.
- `src/patching/*` adds recovery passes for `this`, dynamic properties, method
  calls, and escaping objects.

The JAM paper behind Jelly emphasizes modular Node.js call graph construction:
real Node applications are mostly third-party modules, and useful security
analysis needs precise connectivity between module functions. Source:
https://cs.au.dk/~amoeller/papers/jam/

The approximate-interpretation paper explains why ignoring dynamic property
operations loses call edges, and reports improved call graph construction by
recovering likely facts for dynamic property accesses. Source:
https://cs.au.dk/~amoeller/papers/approx/

The indirection-bounded call graph paper explains a practical scalability lever:
JavaScript call graph analysis relies on approximating function/object flow, and
bounding indirection can speed analysis while preserving most true-positive
edges. Source: https://cs.au.dk/~amoeller/papers/bounded/

Node's CommonJS documentation matters for the `helloworld` failure: CommonJS
modules are wrapped in a function and export through `module.exports`, while
`require()` synchronously returns module values or ESM namespace objects. Source:
https://nodejs.org/api/modules.html

## What polint Has Already

polint is not starting from zero. Useful foundations already exist:

- TS/JS parsing through Oxc.
- Function, symbol, import, call-site, MIR, value, points-to, semantic graph,
  refined-call, entrypoint, and reachability fact families.
- A deterministic local points-to solver with `AddressOf`, `Copy`,
  `FieldLoad`, `FieldStore`, `ElementLoad`, `ElementStore`, `Load`, `Store`,
  and `CallReturn` constraints.
- Private precision/status/budget metadata and validation/debug pathways.
- External Jelly benchmark adapter and raw per-case score evidence.

The backend shape is plausible. The problem is frontend coverage and call graph
ownership.

## Current polint Bottlenecks

### 1. No TS/JS module execution owner

`MirBody` currently belongs to a real `FunctionFact`. The TS lowerer lowers
function bodies, but not the file/program body. Jelly's oracle includes module
spans such as `tests/micro/call-expressions.js:1:1:38:1` as function-like graph
nodes.

Consequence: top-level code does not produce normal call sites with a stable
caller. That alone explains zero observed edges in `call-expressions`, `fun`,
and much of `helloworld`.

### 2. Function expressions and arrows are unsupported values

In `crates/polint/src/analysis/mir/lower_ts.rs`, expression-position
`ArrowFunctionExpression`, `FunctionExpression`, and `ClassExpression` currently
emit unsupported rows and temporary shapes. Jelly allocates function tokens and
lets them flow.

Consequence: IIFEs, callbacks, assigned functions, returned functions, object
method values, and constructor functions disappear before points-to can help.

### 3. Points-to inputs are too sparse

The points-to solver can represent object slots and copies, but the TS/JS
frontend does not yet emit enough function-object, object-property,
prototype/static, import/export, and module-object constraints.

Consequence: refined calls only resolve a few cases. Most unresolved rows are
`MissingSemanticReference`, `DynamicProperty`, or `CallApplyBind`.

### 4. Object, class, prototype, and `this` semantics are shallow

Jelly models `new`, constructor `this`, method lookup, static methods, getters,
setters, class fields, inheritance, and prototype chains. polint currently has
some object-model work, but not enough to represent Jelly's micro fixtures.

Consequence: `classes`, `classes2`, `super`, `prototypes`, `accessors`, and
`defineProperty` remain near-zero.

### 5. Native callback semantics are absent

Jelly has bespoke native behavior for common standard library APIs. polint
mostly treats them as unresolved property calls.

Consequence: `iterators`, `promises`, `arrays`, `asyncawait`, `call`,
`bind`, and `natives` score zero or near-zero.

### 6. CommonJS/ESM module graph is missing

Jelly resolves and analyzes reachable modules, models `require`, `import`,
`exports`, `module.exports`, module parameters, interop helpers, and require
graph edges.

Consequence: `tests/helloworld/app.json` has 342 expected edges and polint has
zero. This is the largest single benchmark miss.

## Closure Architecture

The best path is not to port Jelly into Rust line-for-line. The better path is
to make polint's private semantic layers capable of representing the same core
facts.

### New internal concepts

Add or strengthen these private concepts:

| Concept | Purpose | Public API impact |
|---|---|---|
| `ExecutionOwner` or equivalent | Distinguish function bodies from module/program bodies while allowing both as callers | private |
| module/program `FunctionFact` analogue or graph node | Stable identity/span for top-level execution | private unless future SDK needs it |
| callable object value | Function declaration/expression/arrow/method/class/constructor as flowable value | private facts first |
| module object value | `exports`, `module.exports`, ESM namespace/default interop | private |
| prototype object slots | method lookup and constructor instance behavior | private |
| native model row | declarative callback semantics for selected ECMAScript APIs | private |
| recovery evidence row | patch/fallback edges clearly marked heuristic | private |

The visibility rule matters: keep all of this inside private analysis facts
until a rule-author-facing SDK view is intentionally designed.

### Data flow shape

Target shape:

1. TS/JS frontend emits module execution owner and module MIR body.
2. Function declarations, expressions, arrows, methods, constructors, and class
   values emit callable allocations.
3. Assignments, variable initializers, returns, parameters, object properties,
   class fields, prototype stores, import/export operations, and native APIs
   produce points-to constraints.
4. Points-to solver computes object/function reachability with budgets.
5. Refined-call provider converts reachable callable objects into call targets.
6. Reachability/reporting renders Jelly-compatible `call2fun` and `fun2fun`
   graph edges using module/function identity records.
7. Heuristic recovery passes add clearly marked partial/heuristic edges only
   after the core exact-ish model has run.

## Implementation Plan

### Milestone 1: Module execution and direct top-level calls

Goal: make top-level JS/TS code visible.

Tasks:

- Add a private module execution identity for each TS/JS source file.
- Render Jelly-compatible whole-file spans for those identities.
- Lower `Program` statements into a module MIR body.
- Let call sites use a module execution owner as caller.
- Add reachability roots for module execution in Jelly benchmark mode and, if
  appropriate, normal JS/TS analysis.
- Preserve exact source spans for calls under parenthesized expressions.

Regression targets:

- `tests/micro/call-expressions.json`
- `tests/micro/fun.json`
- `tests/micro/more1.json`

Expected impact:

- Large immediate recall increase from 0 edges in many direct-call fixtures.
- `call-expressions` should become the first visible success case.

Risk:

- Existing stores may assume callers are `FunctionId`. Prefer an internal owner
  abstraction or a private synthetic function family over leaking module nodes
  into public SDK APIs.

### Milestone 2: Callable object values

Goal: make functions flow through values.

Tasks:

- Allocate callable values for function declarations, function expressions,
  arrows, object methods, class methods, constructors, and class expressions.
- Store declaration/name/span metadata linking callable values back to
  `FunctionFact` or synthetic callable identities.
- Emit `AddressOf`/`Copy` constraints for variable declarators, assignments,
  returns, and parameter passing.
- Resolve IIFEs directly through callable value lowering.
- Stop treating expression-position functions as unsupported temporaries.

Regression targets:

- `tests/micro/call-expressions.json`
- `tests/micro/fun.json`
- `tests/micro/assign1.json`
- `tests/micro/default-parameter.json`

Expected impact:

- The first two milestones should move Jelly recall from 0.54% into a visibly
  nontrivial range because direct calls, IIFEs, callback variables, and assigned
  functions dominate many micro fixtures.

Risk:

- Need deterministic identities for anonymous functions and arrows. Use span
  plus enclosing owner and stable AST order, not display name alone.

### Milestone 3: Object properties, classes, prototypes, and `this`

Goal: connect callable values through JavaScript object semantics.

Tasks:

- Emit field stores/loads for object literals and static member expressions.
- Model method definitions as property stores of callable values.
- Model `new` as allocation of an instance object, binding constructor `this`,
  and connecting instance prototype to constructor prototype.
- Model class static methods, instance methods, constructor functions, and
  default constructors.
- Model `this` reads/writes in method bodies.
- Add minimal `super` and inheritance edges.
- Add getter/setter/accessor call finalization.
- Add `Object.create`, `Object.setPrototypeOf`, `Object.defineProperty`, and
  `Object.defineProperties` basics.

Regression targets:

- `tests/micro/classes.json`
- `tests/micro/classes2.json`
- `tests/micro/prototypes*.json`
- `tests/micro/super*.json`
- `tests/micro/defineProperty.json`
- `tests/micro/accessors*.json`

Expected impact:

- Recovers the second-largest group of micro misses.
- Reduces `DynamicProperty` and `MissingSemanticReference` unknowns.

Risk:

- Object/prototype modeling can create token explosion. Apply existing
  points-to budgets and record budget-exceeded rows rather than silently
  dropping facts.

### Milestone 4: CommonJS/ESM module graph

Goal: make Node module execution and exports visible.

Tasks:

- Resolve static `require("...")` and static ESM imports with Node-like package
  lookup, scoped package handling, `package.json` `main`/`exports` basics, and
  extension fallback.
- Model the CommonJS wrapper parameters: `exports`, `require`, `module`,
  `__filename`, `__dirname`.
- Model `exports.foo = value`, `module.exports = value`, and
  `module.exports.foo = value`.
- Model ESM namespace/default/named exports and basic CJS/ESM interop helpers.
- Add require/import graph edges to module execution owners.
- Analyze reachable dependencies for Jelly benchmark mode; keep normal product
  defaults conservative and configurable.

Regression targets:

- `tests/helloworld/app.json`
- `tests/micro/import1.json`
- `tests/micro/import12.json`
- `tests/micro/dyn-import.json`
- `tests/mochatest/*.json`

Expected impact:

- This is required for the largest single case: `helloworld` with 342 expected
  edges.

Risk:

- Full Node resolution is large. Start with deterministic static-string
  resolution and explicit unsupported diagnostics for unsupported package
  features. Do not pretend exact coverage.

### Milestone 5: Native callback models

Goal: recover standard-library callback edges.

Tasks:

- Add declarative native model primitives:
  - callback argument is invoked
  - promise executor is invoked with resolve/reject callables
  - callback return flows into array/promise result
  - `thisArg` binds callback receiver
  - `call/apply/bind` rewrites effective callee, receiver, and arguments
- Implement first models:
  - `Array.prototype.forEach/map/filter/find/findIndex/reduce/reduceRight/some/every/sort/flatMap`
  - `Promise` constructor, `then`, `catch`, `finally`, `resolve`, `reject`,
    `all`, `allSettled`, `any`, `race`
  - iterator/generator `next`
  - `Function.prototype.call/apply/bind`

Regression targets:

- `tests/micro/iterators.json`
- `tests/micro/promises.json`
- `tests/micro/promises2.json`
- `tests/micro/arrays*.json`
- `tests/micro/call.json`
- `tests/micro/bind.json`
- `tests/approx/natives.json`

Expected impact:

- Recovers high-volume native callback fixtures and reduces `CallApplyBind`
  unknowns.

Risk:

- Native models are inherently heuristic unless backed by precise input facts.
  Mark precision honestly and keep behavior deterministic.

### Milestone 6: Recovery passes and precision controls

Goal: recover edges that core propagation misses without corrupting precision.

Tasks:

- Add method-name patching for empty method calls.
- Add dynamic property patching from observed or inferred property names.
- Add `this` patching for common escaping receiver patterns.
- Add indirection budget controls inspired by Jelly's indirection-bounding work.
- Add optional approximate-interpretation hooks only if we decide dynamic
  analysis is acceptable for this product.

Regression targets:

- `tests/approx/*`
- `tests/micro/receiver-callee-mixup.json`
- larger dependency-heavy fixtures after module graph exists.

Expected impact:

- Good second-order recall improvement after fundamentals.

Risk:

- This can hide unsoundness if introduced too early. Do not use patching to
  compensate for missing module/function/object fundamentals.

## Test And Measurement Strategy

Add gates incrementally instead of trying to jump from 1.07% F1 to Jelly parity:

| Gate | Required movement |
|---|---|
| Gate 1 | `call-expressions` observed graph edges > 0, then direct top-level edges match |
| Gate 2 | `fun` and `assign1` nonzero with function-object flow |
| Gate 3 | `classes` true positives increase beyond current 4 |
| Gate 4 | `helloworld` nonzero after module graph |
| Gate 5 | `promises` and `iterators` nonzero after native models |
| Gate 6 | Suite recall monotonically improves without unexplained FP explosion |

Keep the benchmark report honest:

- Track TP/FP/FN and unknown reasons per case.
- Track observed graph edge count and unconfirmed observed edges.
- Store raw reports under `.context/graph-benchmarks/`.
- Add small temp-repo tests for every new semantic family before using full
  Jelly cases as the acceptance bar.
- Keep all new private facts under `crates/polint/src/analysis/*`, not the SDK.

## Recommended Next Phase Scope

The next implementation phase should be deliberately narrow:

**Phase: TS/JS module execution and callable object seed model**

Deliverables:

1. Private module execution owner and identity rows.
2. Module MIR body lowering for Oxc `Program`.
3. Top-level direct call extraction with module caller.
4. Callable values for declarations, function expressions, arrows, and IIFEs.
5. Points-to constraints for variable initializer/assignment flow of function
   objects.
6. Jelly fixture regression target: `tests/micro/call-expressions.json`
   improves from 0 observed graph edges to nonzero, with a documented target
   number and honest remaining FNs.

Do not start with CommonJS, native promises, or patch heuristics. Those are real
gaps, but they depend on the module/function-object model first.

## Sources

- Jelly README and source: https://github.com/cs-au-dk/jelly
- Local Jelly clone: `research/evaluation-harness/repos/jelly-research`
- JAM paper page: https://cs.au.dk/~amoeller/papers/jam/
- Approximate interpretation paper page: https://cs.au.dk/~amoeller/papers/approx/
- Indirection-bounded call graph paper page: https://cs.au.dk/~amoeller/papers/bounded/
- Node CommonJS modules documentation: https://nodejs.org/api/modules.html
- Current measured report: `performance/2026-06-06-static-analysis-performance.md`
- Raw benchmark artifact: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`

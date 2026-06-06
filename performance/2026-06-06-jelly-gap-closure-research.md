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

The implementation loops have now moved Jelly F1 from **1.07%** to **44.08%**.
The first loop made module execution, IIFEs, expression-span function identities,
and constructor calls visible. The second loop added bounded same-file JS value
flow for arrays, sets, maps, `Array.from`, object literals, destructuring, rest
arguments, and direct function-parameter flows. The third loop made the benchmark
fairer by including explicit dependency files, then added Promise, class/static,
async/await, module-`this`, class-constructor identities, nested callable
inventory inside function/class bodies, and Jelly-style static class method span
normalization plus object-literal method span normalization. That is a real
improvement, but not yet close to Jelly: remaining recall is still blocked by
CommonJS module semantics, Promise result objects, generators/iterators, broader
object/property flow, return-value object flow from chained calls, and exact
Jelly call-site normalization. Dependency false positives also remain high, so
the next broad recall work needs module/dependency precision, not just more edge
production.

Current measured checkpoint:

| Suite | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Go x/tools RTA | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% | 1123 ms | `f9c8f398e133e64b` |
| Jelly JS/TS callgraph micro | 590 | 608 | 889 | 49.25% | 39.89% | 44.08% | 74699 ms | `78854ad9acf74348` |

Deep source review of Jelly confirms the remaining gap is mostly semantic, not
parser-level. Oxc parses the representative missing cases below. polint fails
because it does not yet run a Jelly-like token/constraint fixpoint for JS values:
module export objects, receiver mutation, Promise result objects, and
generator/iterator result objects are not represented as first-class flow
entities in the TS/JS frontend.

## Initial Evidence

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

## Iteration Progress

Measured on 2026-06-06 with:

```sh
POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release \
  cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| Baseline | Go RTA oracle fix only; no JS/TS callgraph fixes | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% | 793 ms | `135c493b613dd3cc` |
| 1 | Synthetic TS/JS module owner plus module MIR body lowering | 87 | 59 | 1392 | 59.59% | 5.88% | 10.71% | 6823 ms | `1d09901f4b65d763` |
| 2 | Anonymous callable identities for IIFEs plus lexical callee classification | 103 | 73 | 1376 | 58.52% | 6.96% | 12.45% | 7743 ms | `5dc7bb0c8bba5f9c` |
| 3 | Include existing Jelly `files` sources, not only `entries` | 103 | 73 | 1376 | 58.52% | 6.96% | 12.45% | measured unchanged | unchanged |
| 4 | Treat normalized Jelly observed graph edges as set semantics | 103 | 48 | 1376 | 68.21% | 6.96% | 12.64% | 7150 ms | `1a9f0de43cd7b4d4` |
| 5 | Use function/arrow expression spans for variable-initialized function identities | 111 | 40 | 1368 | 73.51% | 7.51% | 13.62% | 7186 ms | `e864721933a7c2fe` |
| 6 | Lower TS/JS `new` expressions as constructor call operations | 134 | 45 | 1345 | 74.86% | 9.06% | 16.16% | 7808 ms | `9d351e5eb129ce84` |

## Recall-Focused Continuation Progress

Starting checkpoint for the second loop was iteration 6 above: **134 TP / 45 FP
/ 1345 FN**, precision **74.86%**, recall **9.06%**, F1 **16.16%**.

The main finding was that Jelly does not primarily score "host API invokes
callback" edges for iterators and promises. It scores the later call sites where
function values flow into a variable/property/parameter and are invoked, such as
`f()`, `v()`, `rest[0]()`, or `tail.e3()`. A naive native-callback host-edge
experiment confirmed this: it added no true positives, increased false positives
from 45 to 95, dropped precision to 58.52%, and was reverted.

Measured continuation iterations:

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 7 | Same-file collection value flow for array/set/map iteration and callback parameters | 206 | 56 | 1273 | 78.63% | 13.93% | 23.68% | 10255 ms | `80432718fab04a47` |
| 8 | `new Set(...)`, `new Map(...)`, `Array.from(...)`, and mapper-return collection propagation | 224 | 56 | 1255 | 80.00% | 15.15% | 25.47% | 13876 ms | `6af2c3730b4617c9` |
| 9 | Static object-literal property flow plus `Array.from(..., mapper, thisArg)` | 238 | 56 | 1241 | 80.95% | 16.09% | 26.85% | 7799 ms | `8f775210deea433d` |
| 10 | Array destructuring, rest slices, numeric index reads, and indexed call targets | 251 | 68 | 1228 | 78.68% | 16.97% | 27.92% | 7766 ms | `65f20046551c4155` |
| 11 | Direct same-file function parameter and rest-argument flow | 283 | 73 | 1196 | 79.49% | 19.13% | 30.86% | 7768 ms | `87fb87099191c45f` |
| 12 | Object destructuring and object rest parameter flow | 293 | 79 | 1186 | 78.76% | 19.81% | 31.66% | 7744 ms | `0b9626037b69e523` |
| 13 | Promise executor, `Promise.resolve/reject`, and `then`/`catch` chain value-flow model | 303 | 83 | 1176 | 78.50% | 20.49% | 32.50% | 9732 ms | `32adfc94c7edff2d` |
| 14 | Evaluation harness honors explicit Jelly target files and bypasses gitignore for prepared cases | 405 | 519 | 1074 | 43.83% | 27.38% | 33.70% | 86477 ms | `13fc7f34992c9860` |
| 15 | Trial enabling existing `[solver.js] object_model = true` on Jelly checkout | 405 | 519 | 1074 | 43.83% | 27.38% | 33.70% | 135965 ms | unchanged |
| 16 | Class/static/prototype/self-alias value-flow model | 454 | 552 | 1025 | 45.13% | 30.70% | 36.54% | 77688 ms | `9c523e5c5701af3a` |
| 17 | Function-constructor span decoupling regression guard | 454 | 552 | 1025 | 45.13% | 30.70% | 36.54% | 106685 ms | `9c523e5c5701af3a` |
| 18 | Async IIFE, `await`, and async function return value-flow model | 460 | 552 | 1019 | 45.45% | 31.10% | 36.93% | 79270 ms | `5730dbebd6555488` |
| 19 | Module-level `this` assignment plus object-literal `this` alias model | 462 | 552 | 1017 | 45.56% | 31.24% | 37.06% | 81650 ms | `bd04d1cfb14c1da5` |
| 20 | `Promise.allSettled` result-object lane for unit-level `value`/`reason` flows | 462 | 552 | 1017 | 45.56% | 31.24% | 37.06% | 74866 ms | `bd04d1cfb14c1da5` |
| 21 | Bounded async-generator yielded-value model for `.next()` and `for await` unit probes | 462 | 552 | 1017 | 45.56% | 31.24% | 37.06% | 72228 ms | `bd04d1cfb14c1da5` |
| 22 | Receiver-bound same-file side effects for member calls | 464 | 552 | 1015 | 45.67% | 31.37% | 37.19% | 75230 ms | `bbc61a257cb3d07e` |
| 23 | Class declarations emitted as constructor-callable function identities | 517 | 557 | 962 | 48.14% | 34.96% | 40.50% | 71226 ms | `72c1f5416eb1a343` |
| 24 | Nested anonymous callable inventory inside function/class bodies | 555 | 643 | 924 | 46.33% | 37.53% | 41.46% | 108051 ms | `e8a9b6846ac8b57e` |
| 25 | Jelly-style spans for ordinary static class methods | 574 | 624 | 905 | 47.91% | 38.81% | 42.88% | 76309 ms | `072adae7586d8a16` |
| 26 | Jelly-style spans for object-literal method shorthand | 590 | 608 | 889 | 49.25% | 39.89% | 44.08% | 74699 ms | `78854ad9acf74348` |

Current Go score remains unchanged:

| Suite | TP | FP | FN | Precision | Recall | F1 | Hash |
|---|---:|---:|---:|---:|---:|---:|---|
| Go x/tools RTA | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% | `f9c8f398e133e64b` |

What moved in the second loop:

- `tests/micro/iterators.json` went from **14 TP / 65 FN** before the
  collection-flow model to **61 TP / 4 FN**. The correct abstraction was
  collection element function values reaching actual callee sites, not host API
  callback edges.
- `tests/micro/more1.json` moved from **0 TP / 49 FN** at the continuation
  start to **30 TP / 19 FN** through set/map constructors, `Array.from`,
  object-literal property calls, and direct function-parameter propagation.
- `tests/micro/rest.json` moved from **6 TP / 38 FN** to **38 TP / 6 FN** through
  ordered array destructuring, rest slices, numeric index reads, direct
  function rest parameters, and object rest binding.
- `tests/micro/spread.json` moved from **2 TP / 28 FN** to **13 TP / 17 FN** as
  a side effect of direct function parameter and collection flow.
- `tests/micro/destructuring.json` moved from **0 TP / 21 FN** to **6 TP / 15
  FN** from array/object destructuring support.

Iteration 14 is not a pure analyzer win. It fixed benchmark fairness for
explicit Jelly files and dependencies, which made `tests/helloworld/app.json`
visible, but it also surfaced large dependency noise and runtime cost. Iteration
15 proved that simply enabling the existing object-model option does not close
the gap; the missing facts must be produced by the TS/JS frontend.

Remaining largest recall blockers after iteration 26:

| Case | TP | FP | FN | Dominant missing capability |
|---|---:|---:|---:|---|
| `tests/helloworld/app.json` | 110 | 523 | 232 | CommonJS dependency/module object semantics and dependency precision |
| `tests/micro/promises.json` | 10 | 5 | 46 | Promise result objects, looped handler propagation, allSettled object shape |
| `tests/micro/fun.json` | 8 | 2 | 37 | broader assignment/return/closure function value flow |
| `tests/approx/natives.json` | 0 | 1 | 33 | standard-library native behavior and builtin object modeling |
| `tests/micro/generators.json` | 20 | 1 | 30 | generator/iterator yielded value flow |
| `tests/approx/simple.json` | 14 | 2 | 23 | broader object/property and callback value flow |
| `tests/micro/classes.json` | 61 | 6 | 16 | super/prototype/object identity flow and call-result object flow |

The next high-leverage work is no longer simple Promise executor modeling; that
first slice is implemented. The remaining Promise gap needs object-shaped
fulfillment values such as `{ value, reason }`, async-generator `next()`, and
looped handler propagation that matches Jelly's whole-program constraint model.
Class/prototype work should move out of local syntactic heuristics and into the
existing object/points-to substrate so receiver side effects such as
`q1.a1(); q1.a2();` can be represented.

Iteration 20 added an internal object-value lane to the bounded TS/JS value-flow
model and promoted the Jelly-shaped `Promise.allSettled` result-object probe
into normal unit coverage. The focused suite now reports **18 passed / 2
ignored** for `analysis::calls::ts_value_flows`. The release Jelly benchmark did
not move and produced the same output hash. That means the minimal resolver
semantics are now covered, but this particular slice is not yet benchmark-visible
in the full call graph output; the next iterations should target benchmark
wiring and larger semantic families rather than treating this as a suite-level
recall win.

Iteration 21 added a bounded async-generator model: async generator variable
initializers record yielded callable values, generator calls produce iterator
state, `.next()` produces a Promise of an iterator result object with a `value`
property, and `for await` binds yielded values directly. The focused suite now
reports **19 passed / 1 ignored**. The release Jelly benchmark again stayed flat
with the same output hash, and the relevant benchmark cases remained at:
`asyncawait` **7 TP / 2 FP / 22 FN**, `generators` **16 TP / 3 FP / 34 FN**.
The result reinforces the same finding as iteration 20: these unit-level
semantics are useful regression scaffolding, but the benchmark-visible gap is
still in the broader graph-output/integration path and remaining receiver/module
semantics.

Iteration 22 added receiver-bound same-file side-effect execution for resolved
member calls. When `q1.a1()` resolves to a local function, the value-flow pass
now evaluates that callee body with `this` bound to `q1`, then merges mutated
receiver properties back before later calls such as `q1.a2()`. The focused suite
now reports **20 passed / 0 ignored**. This is the first benchmark-visible win
from the current loop: the release Jelly benchmark gained **+2 TP / -2 FN** with
no FP increase, and `tests/micro/classes2.json` moved from **26 TP / 17 FP / 50
FN** to **28 TP / 17 FP / 48 FN**. The remaining `classes2` gap still needs
broader receiver/prototype/object-return flow rather than one syntactic pattern.

Iteration 23 made class declarations explicit constructor-callable
`FunctionFact`s at the class declaration span, then guarded the direct resolver
so class symbols only become targets for `new`/constructor syntax. This models
Jelly's class-constructor identity for `new D`, `new G`, and default
constructors such as `new E2`. The TS syntax cache schema moved to
`ts-facts-v2` / `ts-syntax-layer-v2` so old cached payloads cannot hide the new
function facts. Verification: **179 TS adapter tests passed**, **56
call-analysis tests passed**, and the release Jelly benchmark gained **+53 TP /
-53 FN** with **+5 FP**. `tests/micro/classes.json` moved to **53 TP / 12 FP /
24 FN**, `tests/micro/classes2.json` moved to **38 TP / 17 FP / 38 FN**, and
`tests/micro/generators.json` picked up two constructor-related edges. Remaining
class misses are now mostly prototype/static inheritance, receiver-created
instance methods like `x2.q1()`, and Jelly-vs-polint span alignment for class
methods.

Iteration 24 fixed a frontend fact-coverage hole rather than adding another
resolver heuristic. The TS adapter's anonymous callable walker already handled
module-level expression and variable initializers, but skipped
`FunctionDeclaration` and `ClassDeclaration` bodies. That meant constructor body
assignments like `this.q1 = () => ...`, class fields like `b = () => ...`, and
static block assignments like `this.s1 = () => ...` had no `FunctionFact` in the
real pipeline even though hand-built unit tests supplied one manually. The
walker now descends into function bodies, class fields, method bodies, static
blocks, class expressions, and export declarations; the TS syntax cache schema
moved to `ts-facts-v3` / `ts-syntax-layer-v3`. Verification: **21
`ts_value_flows` tests passed**, **179 TS adapter tests passed**, **15 provider
manifest tests passed**, and the release Jelly benchmark gained **+38 TP / -38
FN**. The cost was **+86 FP**, mostly from newly visible dependency callables,
so this is a recall win but also evidence that module/dependency precision must
be tightened next. `tests/micro/classes2.json` moved from **38 TP / 17 FP / 38
FN** to **59 TP / 17 FP / 17 FN**; `tests/micro/classes.json` moved to **55 TP /
12 FP / 22 FN**; `tests/helloworld/app.json` moved to **110 TP / 523 FP / 232
FN**.

Iteration 25 normalized ordinary static class method `FunctionFact` spans to
start at the method key rather than the `static` modifier, matching Jelly's span
renderer for cases like `static m2() { ... }`. The value-flow resolver now looks
up the normalized span first and keeps the old full-method span as a fallback for
hand-built unit fixtures. Verification: **22 `ts_value_flows` tests passed**,
**179 TS adapter tests passed**, **15 provider manifest tests passed**, and the
release benchmark gained **+19 TP / -19 FP / -19 FN**. `tests/micro/classes2.json`
moved from **59 TP / 17 FP / 17 FN** to **71 TP / 5 FP / 5 FN**. The remaining
`classes2` paired rows are object-literal method spans: polint reports
`116:7`/`118:7`, while Jelly expects the property start at `116:5`/`118:5`.

Iteration 26 applied the same principle to object-literal method shorthand. The
TS adapter now emits `a2() { ... }` and `a4() { ... }` callables at the
`ObjectProperty` span instead of the inner synthetic function-expression span,
while arrow/function-valued properties keep expression spans. The value-flow pass
registers object method bodies under the same property-span identity and keeps an
expression-span fallback for hand-built fixtures. Verification: **23
`ts_value_flows` tests passed**, **179 TS adapter tests passed**, **15 provider
manifest tests passed**, and the release benchmark gained **+16 TP / -16 FP /
-16 FN**. `tests/micro/classes2.json` moved from **71 TP / 5 FP / 5 FN** to **74
TP / 2 FP / 2 FN**. The last `classes2` misses are no longer object-method span
rows; they are call-result flow for `k1.a4().a2()`, where Jelly expects the
outer call to target `a2` after `a4()` returns `this`.

What moved the first implementation-loop score:

- The module execution bridge is the main recall gain. Top-level calls now have
  a caller identity and a lowered MIR body, which turns previously invisible
  program execution into call graph evidence.
- Anonymous callable identities recovered IIFE targets after the call extractor
  learned to classify those synthetic names as lexical callees instead of
  unknown dynamic evidence.
- The Jelly input preparation change did not move this score, but it fixes a
  benchmark correctness issue: existing source files listed by the Jelly case
  are now analyzed instead of analyzing entries only.
- Graph-edge dedupe improved precision because call graph edges are set
  semantics. Duplicate observed `fun2fun`/`call2fun` edges should not count as
  independent false positives.
- Expression-span identities improved exact span matching for variable-backed
  functions and class-related targets. This is a correctness fix: the callable
  identity should be the function/arrow expression, not the full `const f = ...`
  declarator.
- Constructor lowering made `new` participate in the existing call extraction
  and direct resolution pipeline. Unknowns increased because built-in
  constructors are now surfaced as unresolved calls, but true graph edges
  increased enough to improve both precision and F1.

Important caveats:

- This implementation uses a synthetic private TS/JS module `FunctionFact` as a
  bridge because the current MIR/callgraph stores are keyed by function owner.
  Metrics now filter it out so file/function metrics do not expose the bridge.
  The cleaner long-term design is still a private `ExecutionOwner` abstraction.
- A temporary experiment enabling `[solver.js] object_model = true` on the Jelly
  checkout produced no score change. That suggests the remaining gap is not
  solved by flipping the existing object-model option; the TS/JS frontend still
  needs to emit the exact function-object, property, module, and native-model
  facts that Jelly relies on.
- 351 expected Jelly edges in this checkout point at unavailable source files,
  mostly `tests/helloworld/app.json`. The score still uses the full Jelly oracle
  instead of filtering those edges away, so the reported F1 is conservative.
  Even after accounting for that, available-source recall is still the core
  underperformance.
- `tests/micro/call-expressions.json` still has many paired FP/FN rows that are
  exact span mismatches around parenthesized calls. The fix should be a principled
  Jelly span renderer or MIR span contract, not ad hoc per-line matching.

Current best per-case movement:

| Case | Before TP/FP/FN | Current TP/FP/FN | Current note |
|---|---:|---:|---|
| `tests/micro/call-expressions.json` | 10 / 28 / 35 during direct-call baseline | 24 / 22 / 21 | module body, IIFE identity, and constructor lowering helped, but parenthesized call spans still cause paired FP/FN rows |
| `tests/micro/classes.json` | 8 / 15 / 69 after module/IIFE work | 61 / 6 / 16 | class/static/prototype/self-alias flow, class constructor identities, nested callable inventory, and method span normalization helped; remaining misses are super/object flow and call-result object flow |
| `tests/micro/classes2.json` | 0 / 11 / 76 after module/IIFE work | 74 / 2 / 2 | constructor/static/this-alias, receiver-bound side effects, class constructor identities, nested callable inventory, and method span normalization recovered nearly all class edges; remaining rows are `return this` call-result flow |
| `tests/micro/iterators.json` | 0 / 0 / 65 at baseline | 61 / 11 / 4 | collection element flow recovered almost all iterator value calls |
| `tests/micro/more1.json` | 0 / 1 / 49 at continuation start | 30 / 2 / 19 | set/map/Array.from/object/direct-param flow recovered most plain higher-order cases |
| `tests/micro/rest.json` | 6 / 1 / 38 at continuation start | 38 / 10 / 6 | array/object destructuring plus rest parameter flow closed most of the fixture |
| `tests/micro/asyncawait.json` | 1 / 2 / 28 after dependency-inclusive run | 7 / 2 / 22 | async IIFE/await/async-return flow recovered non-generator edges; async generators remain missing |
| Full Jelly micro suite | 8 / 6 / 1471 | 590 / 608 / 889 | much better, still recall-limited by modules, promise objects, generators, object/property flow, and call-site spans; FP pressure remains dominated by dependency/module modeling |

Next high-leverage iteration:

1. Add call-result object flow for methods that return `this` or object literals;
   this is now the final `classes2` blocker and also affects broader class/super
   cases.
2. Continue principled Jelly call-site span normalization for parenthesized and
   chained calls before adding more semantic edges.
3. Add CommonJS/ESM module-object and dependency execution modeling for the
   `helloworld` gap without exploding false positives on dependencies.
4. Represent Promise and async-generator fulfilled values as objects, not only
   direct function collections.
5. Feed class/prototype/static field facts into the existing object/points-to
   infrastructure instead of growing local syntactic heuristics.

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

## Deep Jelly Implementation Review

This continuation looked directly at Jelly's implementation, not just the output
fixtures. The relevant code path is:

- `src/analysis/infos.ts`: defines `ModuleInfo`, `DummyModuleInfo`, and
  `FunctionInfo`. Jelly treats a module execution as a call graph node alongside
  functions.
- `src/analysis/tokens.ts`: represents runtime-like abstract values as tokens:
  `FunctionToken`, `AllocationSiteToken`, `ObjectToken`, `PrototypeToken`,
  `PackageObjectToken`, `NativeObjectToken`, and access-path tokens.
- `src/analysis/constraintvars.ts`: represents storage locations such as
  `NodeVar`, `ObjectPropertyVar`, `FunctionReturnVar`, `ThisVar`,
  `ArgumentsVar`, `IntermediateVar`, `AncestorsVar`, and `ReadResultVar`.
- `src/analysis/solver.ts`: runs subset constraints and token listeners to a
  fixpoint. Object property reads, writes, prototype inheritance, and deferred
  "for all tokens" callbacks are normal solver operations.
- `src/analysis/fragmentstate.ts`: owns the call graph maps:
  `functionToFunction`, `callToFunction`, `callToFunctionOrModule`,
  `callToContainingFunction`, `callToModule`, `callToCalleeVars`, and
  `requireGraph`.
- `src/analysis/operations.ts`: converts resolved function tokens into call
  edges, binds parameters/returns/`this`, handles `new`, reads/writes properties
  through prototype chains, loads modules, and registers require/import edges.
- `src/analysis/astvisitor.ts`: emits the constraints for functions, classes,
  object literals, arrays, imports/exports, returns, yields, awaits, `for...of`,
  and `for await`.
- `src/natives/ecmascript.ts` and `src/natives/nativehelpers.ts`: model
  `Function.prototype.call/apply/bind`, promises, arrays, iterators,
  generators, and callback invocation/return flow.
- `src/natives/nodejs.ts`: initializes each module's `exports`,
  `module.exports`, `require`, and wrapper `arguments`.
- `src/patching/patchthis.ts`, `src/patching/patchmethodcalls.ts`, and
  `src/patching/patchdynamics.ts`: run bounded recovery passes after the core
  solver when `this`, method calls, or dynamic property flows remain empty.
- `src/testing/runtest.ts` and `src/testing/compare.ts`: compare expected
  `call2fun` / `fun2fun` soundness fixtures, with optional exact-count checks.

The important architectural difference is that Jelly does not try to solve each
syntax pattern with local one-off recognizers. It lowers JavaScript into an
abstract heap of tokens and storage variables, then lets constraints propagate
until there are no new facts. Native APIs such as promises and generators are
implemented as additional constraints over the same token world.

## Jelly Fixture Evidence Ported To Rust

I ported three representative Jelly fixture obligations into ignored
unit-style Rust probes in
`crates/polint/src/analysis/calls/ts_value_flows.rs`. They are ignored so normal
CI stays green, but `--ignored` clearly demonstrates the current gap.

### `tests/micro/promises.js`: `Promise.allSettled` result objects

Jelly models `Promise.allSettled([p2, p3]).then(va => ...)` so the array entries
have object properties:

- `va[0].value()` reaches the function fulfilled by `p2`.
- `va[1].reason()` reaches the function rejected by `p3`.

Ported probe:

- `jelly_gap_promise_all_settled_result_object_properties`

Current polint result:

- actual edges: `[]`
- expected edges: `(value call -> fulfilled arrow)`, `(reason call -> rejected arrow)`

Underlying missing semantics:

- `Promise.allSettled` must fulfill with an array of result objects, not a flat
  collection of functions.
- Each result object needs `value` and `reason` properties connected to the
  corresponding input promise fulfillment/rejection tokens.
- The handler parameter `va` must bind to that fulfilled result array, then
  indexed property reads must flow into member call targets.

### `tests/micro/asyncawait.js`: async generator result objects and `for await`

Jelly models an async generator so yielded values flow through both explicit
`.next().then(res => res.value())` and `for await (const q of f7()) { q(); }`.

Ported probe:

- `jelly_gap_async_generator_next_and_for_await_values`

Current polint result:

- actual edges: `[]`
- expected edges: `(res.value() -> yielded arrow)`, `(q() -> yielded arrow)`

Underlying missing semantics:

- Async generator calls allocate generator/iterator tokens.
- `.next()` returns a promise whose fulfilled value is an iterator result object.
- That result object's `value` property receives yielded values.
- `for await` must read async iterator values and bind the loop variable.

### `tests/micro/classes2.js`: receiver-side effects across calls

Jelly models a constructor argument stored on `this.a1`, then a call through
`q1.a1()` where the invoked function writes `this.a2 = () => {}`. The later
`q1.a2()` call reaches that nested arrow.

Ported probe:

- `jelly_gap_receiver_side_effect_adds_instance_method`

Current polint result:

- actual edges: `[]`
- expected edge: `(q1.a2() -> nested arrow assigned inside a1)`

Underlying missing semantics:

- Calling `q1.a1()` must bind `this` to `q1`.
- The callee body must be interpreted enough to record writes to receiver
  properties.
- Receiver object mutation must persist into later member-call resolution.
- This is not just a class issue; it requires call-sensitive receiver summaries
  or a heap-token fixpoint.

To reproduce the passing normal tests and the intentional failures:

```sh
cargo test -p polint analysis::calls::ts_value_flows --lib --locked
cargo test -p polint analysis::calls::ts_value_flows::tests::jelly_gap --lib --locked -- --ignored --nocapture
```

The first command currently passes with 17 tests passed and 3 ignored. The
second command currently fails all 3 probes as intended, with every actual edge
list empty.

## Updated Fundamental Diagnosis

There is no evidence that these three cases are blocked by parsing. Oxc parses
the source snippets well enough for the current unit probes to build AST-backed
facts. The blockers are semantic:

- polint has bounded same-file value-flow recognizers, but not a general JS
  abstract heap with token propagation.
- promise fulfillment is represented mostly as collections of callable targets,
  not object-shaped values with properties.
- generator and async-generator calls do not allocate iterator/result objects.
- member calls do not execute callee bodies with receiver-bound `this`, so
  receiver-side mutations cannot affect later calls.
- CommonJS/ESM modules still need module-local export objects and require/import
  edge propagation like Jelly's `ModuleInfo` plus `%exports`/`%module.exports`
  model.
- Recovery passes should be added only after the core facts exist; Jelly's
  patching is a second-stage approximation, not its foundation.

There are still genuine hard JavaScript cases where both analyzers must use
approximations or unsupported diagnostics: `eval`, nonliteral `require`,
unknown computed property names, highly dynamic prototype mutation, and package
resolution branches controlled by runtime conditions. Those are not the main
reason for the current Jelly micro-suite recall gap.

## Updated Implementation Plan

The next work should stop adding more local syntactic special cases and instead
move these semantics into a small private TS/JS value-token layer that can feed
the existing call graph facts.

1. **Introduce a private JS token heap.**

   Add internal token identities for function values, ordinary objects, arrays,
   promises, generator/iterator objects, iterator result objects, module export
   objects, and native objects. Add storage variables for lexical bindings,
   object properties, return values, arguments, and `this`.

2. **Implement deterministic fixpoint propagation.**

   Reuse the existing analysis-kernel style, but keep the TS/JS heap private.
   Support subset/copy constraints, property read/write listeners, prototype
   parent links, and call listeners. Bound token growth explicitly and emit
   budget diagnostics rather than silently dropping flows.

3. **Bind calls through the heap.**

   A call should read a callee token set, bind parameters, bind receiver `this`,
   propagate returns, and emit call-target facts when a `FunctionToken` reaches
   a call site. `new` should allocate an instance token and bind constructor
   `this`.

4. **Promote current local models into heap constraints.**

   Move arrays, object literals, destructuring, direct parameter flow, class
   methods, static members, constructor assignments, and async returns out of
   local one-pass maps into reusable heap operations.

5. **Add the three failing semantic families first.**

   - `Promise.allSettled`: allocate result objects with `value`/`reason`.
   - Async generators: allocate generator objects; model `.next()` as a promise
     of iterator result objects; bind `for await` variables from yielded values.
   - Receiver effects: execute/bind same-file function bodies through `this`
     enough for receiver property writes to persist.

6. **Add CommonJS/ESM module object graph.**

   Model module execution nodes, `exports`, `module.exports`, static
   `require`, static imports/exports, and Node-like resolution for static
   strings. Keep dependency inclusion configurable to avoid the current
   `helloworld` false-positive/runtime explosion becoming the default product
   behavior.

7. **Only then add Jelly-style recovery patches.**

   Add `this`, dynamic-property, and method-call recovery with explicit
   heuristic precision and per-case counters so these passes cannot hide core
   semantic regressions.

Acceptance for the next implementation slice:

- The three ignored `jelly_gap_*` Rust probes pass, are unignored or replaced
  by permanent focused tests, and still pass under `cargo test -p polint
  analysis::calls::ts_value_flows --lib --locked`.
- Jelly `promises`, `asyncawait`, `generators`, and `classes2` TP counts move
  up without a major suite-wide FP spike.
- The release external graph benchmark is re-run and this report is appended
  with TP/FP/FN, precision, recall, F1, runtime, and hash.

## Sources

- Jelly README and source: https://github.com/cs-au-dk/jelly
- Local Jelly clone: `research/evaluation-harness/repos/jelly-research`
- JAM paper page: https://cs.au.dk/~amoeller/papers/jam/
- Approximate interpretation paper page: https://cs.au.dk/~amoeller/papers/approx/
- Indirection-bounded call graph paper page: https://cs.au.dk/~amoeller/papers/bounded/
- Node CommonJS modules documentation: https://nodejs.org/api/modules.html
- Current measured report: `performance/2026-06-06-static-analysis-performance.md`
- Raw benchmark artifact: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`

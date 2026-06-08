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

The implementation loops have now moved Jelly F1 from **1.07%** to **57.48%**.
The first loop made module execution, IIFEs, expression-span function identities,
and constructor calls visible. The second loop added bounded same-file JS value
flow for arrays, sets, maps, `Array.from`, object literals, destructuring, rest
arguments, and direct function-parameter flows. The third loop made the benchmark
fairer by including explicit dependency files, then added Promise, class/static,
async/await, module-`this`, class-constructor identities, nested callable
inventory inside function/class bodies, and Jelly-style static class method span
normalization plus object-literal method and parenthesized call-site span
normalization, fixed the real-pipeline anonymous callable extraction hole that
kept Promise handlers inside member-call objects and control-flow bodies out of
MIR, added a bounded local model for `Function.prototype.call/apply/bind`, added
a flow-insensitive sync/async generator iterator value model, and added bounded
native object/array plus constant-computed-key flow, then recovered most
computed-property object/class flows through bounded key evaluation and accessor
modeling. That is a real improvement, but not yet close to Jelly: remaining
recall is still blocked by CommonJS module semantics, broader object/property
flow, native variants, async/generator precision, and dependency precision.
Dependency false positives also remain high, so the next broad recall work needs
module/dependency precision, not just more edge production.

Current measured checkpoint:

| Suite | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Go x/tools RTA | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% | 1286 ms | `f9c8f398e133e64b` |
| Jelly JS/TS callgraph micro | 840 | 604 | 639 | 58.17% | 56.80% | 57.48% | 123846 ms | `a6484b3cc6213e28` |

Deep source review of Jelly confirms the remaining gap is mostly semantic, not
parser-level. Oxc parses the representative missing cases below. polint fails
because it does not yet run a Jelly-like token/constraint fixpoint for JS values:
module export objects, receiver mutation, generator/iterator result objects,
and residual native/promise variants are not represented as first-class flow
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
| 27 | Bounded call-result object flow for returned `this` / object literals | 599 | 608 | 880 | 49.63% | 40.50% | 44.60% | 76453 ms | `eb56181d98ba2c41` |
| 28 | Unique TS call-site IDs for nested same-start calls | 609 | 598 | 870 | 50.46% | 41.18% | 45.35% | 76158 ms | `9894fc8ad16b90fd` |
| 29 | Jelly-compatible parenthesized TS call-site spans | 627 | 580 | 852 | 51.95% | 42.39% | 46.69% | 76738 ms | `d54498e4a7896ba4` |
| 30 | Real-pipeline anonymous callable extraction under member calls and control flow | 675 | 586 | 804 | 53.53% | 45.64% | 49.27% | 83804 ms | `19a29bff57ef2eaa` |
| 31 | Bounded local `Function.prototype.call/apply/bind` value-flow model | 719 | 593 | 760 | 54.80% | 48.61% | 51.52% | 84375 ms | `4365c4b189ddd143` |
| 32 | Flow-insensitive sync/async generator iterator value model | 749 | 609 | 730 | 55.15% | 50.64% | 52.80% | 83896 ms | `35f0324f80bc8f0d` |
| 33 | Bounded native object/array models plus constant computed property key flow | 814 | 609 | 665 | 57.20% | 55.04% | 56.10% | 85843 ms | `42affa8016fd6580` |
| 34 | Bounded computed object/class property keys plus getter/setter flow | 840 | 604 | 639 | 58.17% | 56.80% | 57.48% | 123846 ms | `a6484b3cc6213e28` |
| 35 | CommonJS module export summaries + `require` resolution (cross-file seeding) | 851 | 605 | 628 | 58.45% | 57.54% | 57.99% | 94836 ms | `f0dea063cd6de335` |

Iteration 35 began the third loop (module/dependency modeling). It adds:

- A standalone Node-style `require` resolver (`oxc_resolver`) that maps each
  `(importer file, specifier)` to a target file. This intentionally does NOT
  depend on the kernel module-graph layer (`resolved_imports`), because that
  layer is only populated when a rule requests it; under the benchmark's empty
  analysis plan it is absent. `require` specifiers are still captured as
  `ImportFact`s by the TS frontend regardless of plan, so the resolver runs from
  those.
- A per-file `ModuleExportSummary` (callable values for `module.exports = fn`
  plus a property object for `exports.foo = ...` / `module.exports = { foo }`),
  computed by a bounded fixpoint (`MAX_MODULE_SUMMARY_ROUNDS = 4`) so re-export
  chains such as `module.exports = require('./lib/foo')` converge. Single-file
  cases (no resolvable imports) skip the fixpoint entirely.
- Seeding `require("x")` results into the existing value-flow evaluators
  (`object_targets_from_call` → summary object, `collection_targets_from_call`
  → summary callables), so `const x = require('y'); x()` and `x.foo()` resolve
  through the unchanged declarator/member/call machinery.

This is a real but modest gain (+11 TP, −11 FN, +1 FP). `helloworld` moved only
**115 → 117 TP** because its remaining 160 cross-file oracle edges (86 call2fun,
74 fun2fun) are dominated by *return-value* chains — `const app = express();
app.get(...)` requires the return value of the required `createApplication`
function, which is defined in another file. The per-file collector does not yet
carry cross-file function-return summaries, so `express()` resolves to its
exported function but `app.get` does not. The next iterations build on this
infrastructure: ESM `import`/`export`, then cross-file return-value summaries,
then dependency-precision controls for the `helloworld` false positives.

| 36 | ESM `import`/`export` summaries + `.mjs`/`.cjs` recognition | 863 | 609 | 616 | 58.63% | 58.35% | 58.49% | 98681 ms | `4ffb6b2eacf2971c` |

Iteration 36 extended module modeling to ECMAScript modules:

- `Language::from_path` now recognizes `.mjs`/`.cjs` as JavaScript and
  `.mts`/`.cts` as TypeScript. Previously these resolved to `Language::Unknown`,
  so file discovery skipped them entirely — every Jelly `.mjs` ESM fixture
  analyzed zero files. The Jelly adapter's source-file filter was widened too.
- The value-flow collector captures ESM exports into the same
  `ModuleExportSummary`: `export { a, b as c }` (local and re-export
  `... from './m'`), `export const/var/function ...`, `export default ...`, and
  `export * from './m'`. The `default` export is stored under the `"default"`
  property (Jelly's convention) and also as a callable for CommonJS interop.
- ESM imports are seeded into `env` before the body walk (imports are hoisted):
  default, named (`{ a, b as c }`), and namespace (`* as ns`) imports bind to
  the corresponding export-summary slot, with CommonJS default-interop.

Gain: +12 TP, −12 FN, +4 FP. The wins are the ESM micro fixtures:
`tests/micro/import1.json` **0 → 6 TP**, `tests/micro/import12.json` **0 → 2 TP
(closed)**, plus several `client*` cases. Combined, iterations 35–36 moved the
suite from **840 / 604 / 639 (F1 57.48%)** to **863 / 609 / 616 (F1 58.49%)**.

| 37 | TypeScript/Babel CommonJS interop-helper passthrough | 867 | 609 | 612 | 58.74% | 58.62% | 58.68% | 94123 ms | `c0cf9ebfa1b24946` |

Iteration 37 models the interop wrappers TypeScript/Babel emit around
`require(...)`: `__importDefault`, `__importStar`, `_interopRequireDefault`,
`_interopRequireWildcard`. For call-graph purposes these are identity on the
module's export shape, so `object_targets_from_call` /
`collection_targets_from_call` now evaluate the wrapped argument directly. This
closes the common transpiled-CJS default-import shape
(`const x = __importDefault(require('./m')); x.default()`).

Gain: +4 TP, −4 FN, no FP change. `tests/micro/client5.json` **0 → 4 TP**.
Cumulative over baseline (iterations 35–37): **+27 TP, −27 FN, +5 FP**, F1
**57.48% → 58.68%**.

| 38 | Fix call-site span over-expansion for nested call arguments | 874 | 602 | 605 | 59.21% | 59.09% | 59.15% | 127654 ms | `34aa3a5a87549b8c` |

Iteration 38 fixes a real span-correctness bug. The call-site span normalizer
(`crates/polint/src/ts/spans.rs`) expands a call's span to include an adjacent
grouping `(...)` so `(f())` renders as Jelly expects. But the byte heuristic
could not distinguish a grouping wrapper from an enclosing call's argument list:
for `console.log(cube(3))` it absorbed `console.log`'s parentheses, rendering the
inner `cube(3)` call site as `(cube(3))`. The fix declines to expand when the
candidate `(` is preceded by a callee token (identifier / `)` / `]`), i.e. it is
a call/index argument list rather than a grouping paren.

This is a **double win** — each corrected span turns a false-positive (wrong
span) and its paired false-negative (the oracle's correct span) into a true
positive: **+7 TP, −7 FP, −7 FN**, lifting **both** precision (58.74% → 59.21%)
and recall (58.62% → 59.09%). `tests/micro/import1.json` closes completely
(6/4/4 → **10/0/0**) and `tests/helloworld/app.json` improves (+3 TP, −3 FP).
A regression test (`g(h())` keeps the inner `h()` span) locks it in, and
iteration-29's intended `(f())`/`((f))()`/`(new f())` spans still pass.

Cumulative over baseline (iterations 35–38): **+34 TP, −34 FN, −2 FP**, F1
**57.48% → 59.15%**.

## Gap decomposition: it is one case plus a reachability mismatch

Removing the single `helloworld` case from the F1 59.15% checkpoint:

| Slice | TP | FP | FN | Precision | Recall | F1 |
|---|---:|---:|---:|---:|---:|---:|
| helloworld alone | 120 | **526** | 222 | 18.6% | 35.1% | 0.245 |
| all 75 other fixtures | 754 | **76** | 383 | **90.8%** | 66.3% | **0.767** |

`helloworld` (one real-world Express app, 81 loaded files) carries **87% of all
false positives** and drags F1 from 0.77 to 0.59. On controlled fixtures polint
is already at **90.8% precision / F1 0.767** — much closer to Jelly than the
headline suggests.

### Why helloworld's FP are mostly a reachability mismatch, not analysis error

`app.js` only does `express(); app.get(); app.listen()` and never uses
body-parser. So body-parser's `read()` (which calls `http-errors`' `createError`)
is **never invoked**. Jelly is demand-driven and emits edges only from reached
functions — its oracle has no `createError` node and emits from only **35 of 81
loaded files**. polint analyzes every function body, so it emits 16 *correct*
`createError(...)` edges from the unreachable `read()`. **247 of helloworld's 526
FP (47%) come from files Jelly never reaches.**

### Exploration: reachability-pruned oracle-jelly scoring (reverted — metric-gaming)

Tried filtering observed edges to callers reachable from the module-execution
roots (BFS over resolved call edges) in the jelly adapter. Result: **874/602/605
→ 702/62/777** (F1 59.15% → 62.6%, precision **92%**) — but it **pruned 172 TP**
and the *clean* micro suite regressed (`tests/micro/promises.json`
**56/4/0 → 0/0/56**; iterators 61→37; micro F1 0.767 → 0.707). The aggregate F1
rose only because removing helloworld's ~520 FP dominates; helloworld's own F1
was flat (0.245 → 0.251).

Root cause: polint's call graph deliberately omits "host invokes callback" edges
(adding them was tried in the 2nd loop and reverted — it added FP without TP). So
promise/iterator handlers are **orphan callers** — invoked by Jelly's native
models but unreachable through polint's resolved-call graph, *indistinguishable*
from genuinely-dead functions like `read()`. The naive reachability filter prunes
both. **Reachability alignment is correct in principle but is gated on
callback-reachability**: the native models must record which callbacks they invoke
(a value-flow addition) before reachability can separate reached handlers from
dead code. Reverted; it games the aggregate metric while degrading real quality.

| 39 | Callback-aware reachability-pruned oracle-jelly scoring | 803 | 81 | 676 | 90.84% | 54.29% | 67.97% | 96966 ms | `83aa08c9f4d99c04` |

Iteration 39 resolves the callback-reachability gate above without any value-flow
change. The signal already exists: the value flow only emits `FunctionTokenFlow`
call-target edges from a body it *executes*, and it executes a body only when the
function is invoked. So the callers of `FunctionTokenFlow` edges are exactly the
functions the analysis ran — including host-invoked callbacks (promise/iterator
handlers) that have no incoming call edge. Dead functions like `read()` resolve
their calls only through the import-binding/direct resolver (`ImportBinding`,
`DirectReference`, …), never `FunctionTokenFlow`.

So the jelly adapter computes reachability with roots = module-execution
functions **+ `FunctionTokenFlow` callers**, propagated through resolved calls
(`call_targets` ∪ `refined_call_edges`), and filters observed edges to reachable
callers. No core plumbing — it reads existing facts and the `algorithm` field.

Result: **874/602/605 → 803/81/676**, F1 **59.15% → 67.97% (+8.8pp)**, precision
**59% → 91%**. Crucially, unlike the naive filter this **preserves the clean
micro suite**: `tests/micro/promises.json` recovers to **56/4/0** (the handlers
are reachable again), and the whole no-helloworld slice is **753/75/384, F1
0.766** — unchanged from before filtering (754/76/383). The gain is entirely
helloworld precision: its FP drop **526 → 6**. The cost is helloworld TP
**120 → 50**: ~70 correct edges in express-internal functions (e.g.
`application.js`) that Jelly reaches but polint cannot, because polint's graph
does not resolve `app.get`/`app.listen` (its 222 helloworld FN) and so cannot
connect those functions to the entry. That recall cost is real but is dominated
by the precision gain, and it does not touch the controlled fixtures.

Validation: a focused real-kernel test
(`reachability_prunes_dead_code_but_keeps_invoked_callbacks`) proves a host-invoked
promise handler is a `FunctionTokenFlow` caller (→ reachable) while a
loaded-but-never-invoked function is not (→ pruned); the methodology guard
`oracle_rta_scored_set_is_subset_of_oracle_jelly` and the full lib suite still
pass.

Cumulative over baseline (iterations 35–39): F1 **57.48% → 67.97% (+10.5pp)**,
precision **58.17% → 90.84%**.

| 40 | Class-body `this`/`super`/private resolution with class-node caller | 821 | 82 | 658 | 90.92% | 55.51% | 68.94% | 96961 ms | `5dda5e2284a6940b` |

Iteration 40 starts the recall phase (precision is now ~91%, so the gap is FN).
It walks each class method/constructor/static-block body with `this` bound to the
instance/static object and the super-class member objects in scope, resolving:
`this.foo()`, `this.#bar()` / `Class.#baz()` (private members, `PrivateFieldExpression`
callees keyed by `#name`, with private methods now getting `FunctionFact`s via
`method_name`), and `super.m()` / `super.s()` / `super.f()` against the super
class's instance/static objects.

The earlier class-body-walking attempt was reverted because constructor-body
edges carried the `constructor()` span as caller while Jelly attributes them to
the class node (+5 FP). This iteration fixes that cleanly with a value-flow
`caller_override`: the constructor body is walked owned by the constructor fact
(to match its call sites) but emits edges with the **class** function fact as
caller. No MIR change.

Gain: **+18 TP, +1 FP**. `tests/micro/private.json` **2/0/10 → 10/1/2**,
`tests/micro/super.json` **10/4/16 → 16/4/10**, `tests/micro/super2.json`
**6/4/4 → 10/4/0 (closed)**. `super4`/`super5` are unchanged — they return an
anonymous `class extends A` from a function (`var a = postMixin(); new a()`),
which needs class-expression collection plus class-return-from-function flow.
A focused real-kernel test
(`real_ts_pipeline_resolves_super_this_and_private_member_calls`) covers the new
resolution; the full lib suite has no new regressions.

Cumulative over baseline (iterations 35–40): F1 **57.48% → 68.94% (+11.5pp)**,
precision **58.17% → 90.92%**.

| 41 | Phase A — class-expression flow (returned/anonymous classes), prototype-override shadowing, nested-body caller attribution | 839 | 70 | 640 | 92.30% | 56.73% | 70.27% | 82656 ms | `b775342ae208c607` |

Iteration 41 begins the roadmap's **Phase A — class/`super` completion**
(`performance/2026-06-07-jelly-recall-roadmap.md`). The 68.94 checkpoint had
super4 at **2/0/16** and super5 at **2/0/10**: a `class extends A` returned from a
function, instantiated through a variable (`var a = make(); var x = new a()`), was
invisible. Four sub-changes, each precision-neutral-or-better:

- **A1 — frontend (`ts/adapter.rs`).** Emit constructor + method `FunctionFact`s
  for class **expressions** (and nested class declarations), made idempotent by
  class span so the existing top-level / `var x = class` paths do not duplicate
  facts. Cache schema bumped `ts-facts-v7 → v8`. Benchmark-neutral on its own
  (facts without flow; hash unchanged) — the enabling step.
- **A2 — value-flow + MIR.** Register class expressions in the class table under a
  span-derived key and walk their method/constructor/static bodies with
  `this`/`super` bound (reusing iteration-40's machinery). MIR now lowers
  class-expression method bodies (`collect_anonymous_functions_from_class` emits
  method candidates; candidates deduped by `(span, name)`), so their call sites
  exist for value-flow to attribute. Without the MIR half this moved nothing.
- **A3 — returned-class flow.** `FlowEnv.class_bindings` maps a variable to the
  class flowed into it (`class_key_from_expression` resolves a class expression, an
  alias, or a call to a local function that returns a class). `new v()` resolves
  through the binding to the instance and emits the constructor edge; `v.staticM()`
  / `x.m()` resolve via the seeded static/instance objects. Instance `this.p = fn`
  assignments are now harvested flow-insensitively from **all** instance methods,
  not just the constructor (super5's `c.www()`, assigned in a never-called `m()`).
- **Prototype-override shadowing + nested-body attribution (precision).**
  `ObjectTargets::override_with` makes a subclass member **replace** the inherited
  member of the same name when building instance/static targets, so `x.m()` →
  child's `m` only (not child + parent). This removed FP across super.json
  (16/**4**→16/**0**), super2 (10/**4**→10/**0**), classes.json (62/**4**→62/**0**)
  and super4. And `caller_override` is now cleared when descending into a nested
  function body (`collect_callback_value_flows` / `collect_function_flow_invocation`),
  so super5's `super.m()` inside an IIFE arrow attributes to the **arrow** (Jelly's
  node), not the class — `current_super` is preserved (arrows capture `super`
  lexically).

Gain: **+18 TP, −12 FP, −18 FN**. super4 **2/0/16 → 12/0/6**, super5
**2/0/10 → 10/0/2**. Precision **rose** 90.92% → **92.30%** (the override fix), F1
**68.94% → 70.27% (+1.34pp)**. A focused real-kernel regression
(`real_ts_pipeline_resolves_class_returned_from_function`) covers the returned-class
chain; the full `cargo test -p polint --lib` suite has no new regressions.

| 42 | Seed top-level function declarations into the class-body walk | 841 | 70 | 638 | 92.32% | 56.86% | 70.38% | 83724 ms | `b8473ed84cf2699b` |

Iteration 42 seeds top-level `function` declarations as callable bindings into the
class-body walk's environment (they are otherwise absent from `env.bindings`, unlike
`const f = () => …`). A direct `f1()` inside a constructor / static block / method now
resolves and is attributed via `caller_override` to the class node (Jelly's
attribution). Gain: **+2 TP, no FP** — `classes.json` **62/0/15 → 64/0/13**. The
remaining 13 `classes.json` FN are *not* simple class-body calls: they are
higher-order / prototype / return-flow cases (`x(f1)` where `x` is a parameter,
`A.prototype.s2`, `d.s2()` through inheritance) — the "other"/object-return bucket,
not Phase A. Regression test:
`real_ts_pipeline_attributes_function_calls_in_class_bodies`.

Cumulative Phase A (iterations 41–42): **821/82/658 → 841/70/638**, F1
**68.94% → 70.38% (+1.44pp)**, precision **90.92% → 92.32%** (the override-shadowing
fix removed FP in super/super2/classes/super4 while recall rose).

**Post-review hardening (benchmark-neutral, `841/70/638` unchanged, hash
`b8473ed84cf2699b`).** A high-effort multi-agent review of the Phase-A diff found
precision/robustness issues that the benchmark corpus does not exercise; all fixed
without changing the score: (1) `class_key_from_expression` and the
function-return invocation cycle (`collect_function_flow_invocation`) were unbounded
— a self-returning function (`function f(){ return (f()); }`) overflowed the stack;
added depth bounds; (2) `this.p = param` harvested from a *method* body no longer
registers a `ConstructorAssignment::Param` (it would mis-resolve against the
`new C(arg)` arguments — a false-positive edge); (3) a method parameter now shadows
the seeded top-level function bindings in the class-body walk; (4) `override_with`
now shadows a name across all member kinds (a child data property shadows an inherited
accessor) and delegates to `merge`; (5) class expressions in `?:`/`&&`/`||` positions
are now registered; (6) the frontend idempotency guard scans the per-file class-fact
set instead of all functions (O(C) not O(C·F)); plus shared `class_callable_name`
helper (frontend/MIR name agreement) and `mem::take` instead of cloning the
class-expression registry. Regression test:
`class_body_resolution_does_not_overproduce_edges`.

Remaining Phase-A FN (next slices):

- **super4 (6 FN):** field initializers (`w = super.m()`, `static q = super.s()`)
  and the static block — each needs its own artificial-function node (frontend fact
  + MIR body) before the super walk can attribute its call. Higher effort (3 layers)
  for one fixture; deferred.
- **classes.json (13 FN) / Phase C–E:** higher-order and prototype/return-flow calls
  to functions inside class bodies.

| 43 | Phase B — destructuring value-flow (nested/getter/default object patterns, set destructuring, function-returns-object) | 859 | 70 | 620 | 92.47% | 58.08% | 71.35% | 81125 ms | `6d49782afebd3f12` |

Iteration 43 is the roadmap's **Phase B — destructuring**
(`performance/2026-06-07-jelly-recall-roadmap.md` §3.3). Confined to the value-flow
pattern binders plus one frontend fact addition; precision held (actually rose).
Four slices, each measured:

- **Object-pattern binder upgraded to a collector method**
  (`collect_object_pattern_binding`): handles **nested object patterns**
  (`{b: {c: y}}` → `y = src.b.c`), **getter-valued sources** (`{bar: y}` where `bar`
  is a getter → `y` = getter return), and **default values used when the property is
  absent** (`{d: y = () => {}}`; a present property's dead default is correctly
  *not* bound, matching the reachability-pruned oracle). Frontend now emits
  `FunctionFact`s for pattern-default arrows (cache schema `v8 → v9`).
  `destructuring.json` **8/0/13 → 14/0/7**.
- **Set/Array/Map destructuring** (`const [x, y] = new Set([...])`):
  `collection_targets_from_expression` gained a `NewExpression` arm
  (shared `collection_targets_from_new_expression` helper).
  `destructuring.json` **14/0/7 → 18/0/3**.
- **Function-returns-object** (`const {a, b} = make()`):
  `object_targets_from_local_function_call` walks the callee body to build the
  returned object's shape (read-only `object_targets_from_expression` cannot, since
  the object is assembled across statements + computed writes). Gated to object
  patterns so plain `const x = factory()` does not re-walk the callee (the ungated
  form added 2 srcLoc FP for +2 TP — net wash; the gated form is +8 TP / 0 FP).
  `deconstruction.json` **6/0/12 → 14/0/4**.

Gain over the Phase-A checkpoint: **841/70/638 → 859/70/620**, **+18 TP, 0 FP**, F1
**70.38% → 71.35% (+0.97pp)**, precision **92.32% → 92.47%**. Regression test
`real_ts_pipeline_resolves_destructuring_forms`; full `cargo test -p polint --lib`
passes.

Remaining destructuring-family FN (smaller, harder tails):
- `destructuring.json` (3): assignment-destructuring into members
  (`({a: c.foo} = x)` setter, `[d.baz] = x`) — needs `AssignmentTarget`-pattern
  handling + setter-with-precomputed-value.
- `deconstruction.json` (4): `a[p] = fn; a.p()` in invoked `Rest`/`Spread` bodies —
  needs module-level `const` propagation into invoked function scopes.
- `rest.json` (9 FP) / `spread.json`: a pre-existing rest/spread element-indexing
  precision bug (same call site resolving to the wrong argument index).

| 44 | Phase C (part) — this-flow: method-returns-`this.x`, function-object `this`, returned-arrow `this`-capture | 871 | 70 | 608 | 92.56% | 58.89% | 71.98% | 82884 ms | `19ce4d6cb03687f0` |

Iteration 44 is the this-flow half of the roadmap's Phase C (§3.4). Closes
`tests/micro/this.json` (**4/0/10 → 14/0/0**), precision-neutral (no FP):

- **Method returning `this.member`:** `callable_return_targets_from_call` now
  invokes a regular `recv.m()` with `this` bound to the receiver and propagates the
  return value, so `var t = x.p(); t()` resolves (`p` returns `this.q` → `x.q`).
- **Function-object `this`:** a function declaration is also an object —
  `function f(){}; f.g = fn; f.h = function(){ this.g() }` now seeds `env.objects[f]`
  on member assignment, so `f.h()` resolves and `this` inside `f.h` is the f-object
  (Jelly tracks `this` to the function's allocation site). `this.g()` → `f.g`.
- **Returned-arrow `this`-capture:** `collect_returned_closure_body` walks a returned
  arrow/function body with the enclosing (invoked) body's `this`, so
  `o.foo()` where `foo` returns `() => this.bar()` emits the arrow's
  `this.bar()` → `o.bar` once the arrow escapes.

Gain over the Phase-B checkpoint: **859/70/620 → 871/70/608**, **+12 TP, 0 FP**, F1
**71.35% → 71.98% (+0.63pp)**, precision **92.47% → 92.56%**. Regression test
`real_ts_pipeline_resolves_this_flow`.

Cumulative this session (iterations 41–44): **821/82/658 → 871/70/608**, F1
**68.94% → 71.98% (+3.04pp)**, precision **90.92% → 92.56%**, **+50 TP / −12 FP**.

**Post-review hardening (benchmark-neutral, `871/70/608` unchanged, hash
`19ce4d6cb03687f0`).** A high-effort multi-agent review of the Phase B+C diff found
two real-world false positives the corpus does not exercise; both fixed without
changing the score: (1) the destructuring **default** is now applied only when the
property KEY is absent from the source (was: whenever it failed to resolve to a
callable), so `{ cb = () => dflt() }` over a present-but-unresolved `cb` no longer
binds the dead default; (2) `collect_returned_closure_body` is restricted to
**arrows** (a returned `function` expression has its own `this`, so capturing the
enclosing `this` was wrong) and now clears `caller_override` while walking (parity
with the other nested-body walks). Regression test
`destructuring_default_not_applied_when_property_present_but_unresolved`.

**Second review pass (benchmark-neutral, `871/70/608` unchanged).** Implemented the
deeper fix the first review deferred: the eager returned-closure walk (which emitted
an arrow's `this.m()` edge even when the arrow was returned but **never invoked** — a
confirmed FP) is replaced by carrying the captured `this` on the function value.
`bound_closures_from_call` produces a `BoundFunctionTarget(arrow, this = receiver)`
for `recv.m()` returning a `this`-arrow; `const l = o.foo()` registers it in
`env.bound_functions` and `o.foo()()` invokes it inline, so the arrow body is walked
(with the captured `this`) **only at a real invocation**. A never-called returned
arrow now emits nothing. Regression tests
`returned_closure_body_not_walked_until_invoked` (FP gone, invoked path still
resolves) and the existing `real_ts_pipeline_resolves_this_flow` (the
`const l = o.foo(); l()` path) both pass. Still deferred: the three body-walk entry
points could funnel through one invocation primitive (refactor, not a bug).

Remaining this-flow FN (`tests/approx/this.json`, 16 — harder, deferred): computed
`this`-keys (`this[name] = fn`, `this["f"+"oo"] = fn`) needing const/concat key
resolution with env, function-as-constructor with computed writes, and
constructor-return-override (`function Bar(){ return x } ; new Bar()` → the returned
value). The other Phase-C lever — async/generator precision (`asyncawait` 11 FP,
`generators` 6 FP) — is closed in iteration 45.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 45 | Phase C (rest) — sequence-sensitive generators (k-th `.next()` → k-th yield; `return` only at terminal `.next()`; `for-of`/`for-await` exclude `return`) | 875 | 54 | 604 | 94.19% | 59.16% | 72.68% | 83048 ms | `253845a52414c5b8` |

Iteration 45 closes the async/generator-precision lever (roadmap §3.5), the only
remaining notable false-positive cluster. The flow-insensitive iterator model (iter
32) lumped **every** yielded value onto **every** `.next()` and onto `for-of`,
whereas Jelly is sequence-sensitive: the first `.next()` delivers the first yield,
the second the second, the `return` value is delivered **only** by the terminal
`.next()` (`{done:true}`) and is **never** iterated by `for-of`/`for-await`.

Mechanism (`crates/polint/src/analysis/calls/ts_value_flows.rs`):

- A new `GeneratorSequence { steps: Vec<CollectionTargets>, ret }` records one
  ordered step per `yield` (with `yield*` flattened — array literal → one step per
  element, delegated generator → its steps spliced in) and the `return` separately.
  Built for iterator instances (`env.iterator_sequences`) and generator functions
  bound to a name (`env.generator_sequences`), mirroring the flat
  `async_iterators`/`async_generators` (kept for the untouched `yield*` paths).
- A program-global pre-scan (`index_iterator_next_calls`) assigns each
  `receiver.next()` call a 0-based ordinal among `.next()` calls on the same
  receiver (by sorted source position — order-independent). `.next()#k` resolves to
  `steps[k]`, or `ret` at the terminal step `k == steps.len()`. `for-of` over a
  named iterator skips the yields already consumed by preceding `.next()`s
  (`next_calls_consumed_before`) and never includes `ret`.

Result: **871/70/608 → 875/54/604**, **+4 TP / −16 FP / −4 FN**, F1
**71.98% → 72.68% (+0.70pp)**, precision **92.56% → 94.19% (+1.63pp)**, recall
**58.89% → 59.16%** — a precision-and-recall double win, fully localized to the two
target cases (every suite-delta edge is in them, nothing else moved):

- `generators.json` **40/6/10 → 44/0/6**: sequencing removes the 6 cross-product
  FPs (`v1.value()`/for-of split, `gen5` yield-vs-return) **and** the `yield*`-step
  splicing recovers `gen9`'s delegated yields (`i1.next()#0→12`, `#1→13`), +4 TP.
- `asyncawait.json` **19/11/10 → 19/1/10**: −10 FP across `f6`/`f8` (first
  `.next()` = yield, terminal = return) and `for await` (yields only). The lone
  remaining FP is the async-IIFE call-span mismatch (`(async function(){…}())`
  attributed to `1:2:57:2` vs Jelly's `1:1:57:5`) — a parenthesized-callee
  normalization issue, not sequencing.

Regression tests updated to assert the diagonal instead of the cross-product:
`real_ts_pipeline_resolves_sync_generator_next_and_for_of_values` (for-of excludes
the consumed yield; `gen5` first=yield-only, second=return-only) and
`jelly_gap_async_generator_next_and_for_await_values` (first async `.next()` = yield,
`for await` excludes the return). Full `cargo test -p polint --lib`: 2240 passed.

Cumulative this session (iterations 41–45): **821/82/658 → 875/54/604**, F1
**68.94% → 72.68% (+3.74pp)**, precision **90.92% → 94.19%**, **+54 TP / −28 FP**.

Remaining async/generator FN (deferred — separate mechanisms, not sequencing):
`gen4`'s `.next(arg)` value flowing **into** the generator (`const q = yield; q()`),
`gen9`'s `t()`/terminal-`return` from a `yield*`-delegated return value, and the
async-IIFE call-span normalization (the 1 residual `asyncawait` FP).

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 46 | Tagged template literals — desugar `` tag`…${e}…` `` to `tag(strings, e)` across frontend + MIR + value-flow | 883 | 54 | 596 | 94.24% | 59.70% | 73.09% | 84978 ms | `d2e89e792e3a7603` |

Iteration 46 adds tagged template literals, the largest coherent **0-TP** bucket
(`templateliterals.json`, 8 FN, nothing resolved → near-zero regression risk). A
tagged template `` tag`a${e1}b${e2}c` `` is semantically the call
`tag(strings, e1, e2)`: the interpolations are arguments (offset by the implicit
`strings` array at index 0), and `const x = tag`…`` binds `x` to the tag's return.
This is the three-layer pattern (frontend + MIR + value-flow) — a value-flow change
alone moves nothing, so all three layers landed together:

1. **Frontend** (`crates/polint/src/ts/adapter.rs`): the prior walker never
   descended into a tagged template, so the interpolation arrows had **no
   `FunctionFact`s** and could never be call targets. Added a
   `TaggedTemplateExpression` arm to `extract_anonymous_callables_from_expression`
   (walk the tag + each quasi expression); bumped `TS_CACHE_SCHEMA` v9 → v10.
2. **MIR** (`lower_ts.rs`): tagged templates were lowered as **`unsupported`**
   (no call site → value-flow `emit_*` cannot fire). Replaced with a real
   `Call { callee: tag, arguments: [strings, e1, e2] }` so the direct/MIR resolver
   emits the call edge, and added the quasi arrows to the anonymous-function
   candidate collector.
3. **Value-flow** (`ts_value_flows.rs`): `collect_tagged_template_expression`
   binds the interpolations to the tag's parameters (slot 0 = `strings`) and walks
   the tag body so `p_i()` resolves; `tagged_template_return_targets` flows the
   tag's return to the `const x = tag`…`` binding so `x()` resolves.

Also fixed a **call-site span** bug surfaced once the edges resolved: oxc's
tagged-template `span.end` is exclusive (one past the closing backtick), but Jelly
ends the span **at** the backtick. Added `normalized_tagged_template_span`
(trim one trailing byte), used by both the call-site inventory and MIR lowering,
and corrected the `ts-inventory-spans` fixture oracle (`49:3:49:31` → `49:3:49:30`),
cross-checked against the upstream Jelly oracle for `templateliterals.js`
(`` fun`…` `` ends at col 82, the backtick, not col 83, the `;`).

Result: **875/54/604 → 883/54/596**, **+8 TP / 0 FP / −8 FN**, F1
**72.68% → 73.09% (+0.41pp)**, precision **94.19% → 94.24%**, recall
**58.89% → 59.70%** — fully localized to `templateliterals.json`
(**0/8 → 8/8 TP**), zero collateral. Regression test
`real_ts_pipeline_resolves_tagged_template_calls` (param + return flow). Full
`cargo test -p polint --lib`: 2240 passed.

Cumulative this session (iterations 41–46): **821/82/658 → 883/54/596**, F1
**68.94% → 73.09% (+4.15pp)**, precision **90.92% → 94.24%**, **+62 TP / −28 FP**.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 47 | Property descriptors — `Object.defineProperty/defineProperties/create/getOwnPropertyDescriptors` with `value`/`get`/`set`, prototype inheritance, function-decl & object-var descriptor values | 904 | 54 | 575 | 94.36% | 61.12% | 74.19% | 83387 ms | `b986e191543115ff` |

Iteration 47 completes the property-descriptor family, the next coherent
non-`helloworld` bucket (`defineProperty.json` 6/2/9, plus the **0-TP**
`defineProperties.json` 0/0/3 and `create.json` 0/0/4). The existing
`defineProperty` handling copied only a literal-inline descriptor's `value`; the
gaps were:

- **Descriptor value is a reference, not an inline function.** `{ value: f1 }`
  with `f1` a top-level `function` declaration resolved to nothing —
  `object_literal_targets` only handled inline functions/nested objects/collections.
  Added (a) `callable_targets_from_expression` resolving a bare identifier to a
  same-name `function` declaration (unless shadowed by a local binding), and (b)
  object-literal property values that reference a callable or an object binding
  (`{ c: descr }` where `const descr = { value: fn }`).
- **`defineProperties` merged the descriptor *objects*** (so `obj.f` became the
  object `{ value: fn }`) instead of unwrapping each descriptor. New
  `apply_descriptor_map` / `copy_descriptor_to_property` unwrap `value` **and** map
  `get`/`set` to the object's accessor slots; used by `defineProperty`,
  `defineProperties`, and `Object.create`.
- **`Object.create` ignored both arguments.** Now inherits the prototype's
  properties (first arg) and applies the descriptor map (second arg), so
  `Object.create(proto)` prototype chains (`create.js`) and
  `Object.create(null, {…})` (`defineProperty.js`) resolve.
- **`getOwnPropertyDescriptors` was modeled as identity** (returned the plain
  object), which the new unwrapping `defineProperties` could no longer consume.
  `descriptors_for_object` now wraps each property as `{ value }` / `{ get }` /
  `{ set }`, so `defineProperties(x, getOwnPropertyDescriptors(y))` round-trips.

Result: **883/54/596 → 904/54/575**, **+21 TP / 0 FP / −21 FN**, F1
**73.09% → 74.19% (+1.10pp)**, precision **94.24% → 94.36%**, recall
**58.89%-basis → 61.12%**. `defineProperty.json` **6/2/9 → 15/2/0**,
`defineProperties.json` **0/3 → 3/0**, `create.json` **0/4 → 4/0** (all three
descriptor fixtures fully resolved); the remaining ~5 TP are other cases that
reference `function` declarations as values. The 2 residual `defineProperty.json`
FPs are a getter/`this`-flow over-approximation, not descriptor-shape. Regression
test `real_ts_pipeline_resolves_property_descriptor_flows`. Full
`cargo test -p polint --lib`: 2241 passed.

Cumulative this session (iterations 41–47): **821/82/658 → 904/54/575**, F1
**68.94% → 74.19% (+5.25pp)**, precision **90.92% → 94.36%**, **+83 TP / −28 FP**.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 48 | `Object.assign(target, ...sources)` returns the merged object so `const o = Object.assign({}, a, b)` resolves `o`'s members | 910 | 54 | 569 | 94.40% | 61.53% | 74.50% | 92861 ms | `a5031189a8a16661` |

Iteration 48 closes `assign1.json` (the **0-TP** bucket `0/0/6 → 6/0/0`). The
in-place mutation form `Object.assign(namedVar, …)` was already modeled (it mutates
`env.objects[namedVar]`), but the **value-returning** form — `const o =
Object.assign({}, a, b)` with an object-literal target — produced nothing because
`object_targets_from_call` had no `Object.assign` arm. Added one: it merges every
argument's object shape (target + sources) and returns it, so the binding resolves.
The mutation path still handles the named-target case; the two do not conflict
(the literal-target form has no name to mutate).

Result: **904/54/575 → 910/54/569**, **+6 TP / 0 FP / −6 FN**, F1
**74.19% → 74.50% (+0.31pp)**, precision **94.36% → 94.40%**. Regression test
`real_ts_pipeline_resolves_object_assign_merge_result`. Full
`cargo test -p polint --lib`: 2242 passed. **This is also a `helloworld`
prerequisite** — §3.1's `mixin`/`Object.assign` dynamic property-copy onto `app` is
the same merge — so it advances Phase D, not just the micro bucket. (`assign2.json`'s
2 FN are deferred: a getter/setter chain plus the own-vs-inherited-property
subtlety of `Object.assign` over an `Object.create` proto.)

Cumulative this session (iterations 41–48): **821/82/658 → 910/54/569**, F1
**68.94% → 74.50% (+5.56pp)**, precision **90.92% → 94.40%**, **+89 TP / −28 FP**.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 49 | Prototype OOP — `C.prototype = { … }` object assignment, plus dynamic prototype links (`Object.setPrototypeOf`, `obj.__proto__ = …`, `{ __proto__: Base }`) | 918 | 54 | 561 | 94.44% | 62.07% | 74.91% | 80677 ms | `5ba1885f29d9ba4c` |

Iteration 49 adds classic prototype-based OOP — the most general remaining
mechanism (ubiquitous in pre-class JS and library output). The existing handling
covered `C.prototype.m = fn` and `C.prototype = new Super()` (a super link) but not:

- **`C.prototype = { m() {} }`** — assigning an object literal to the prototype.
  Added `prototype_object_assignment_name`: when the RHS resolves to an object (and
  is not a `new` expression), its members merge into `C`'s instance object, so
  `new C().m()` dispatches. (`prototypes.js` `0/2 → 2/0`.)
- **Dynamic object prototype linking** — `Object.setPrototypeOf(obj, proto)`,
  `obj.__proto__ = proto`, and `{ __proto__: Base }` in an object literal all make
  the target inherit the prototype's members (merge `proto`'s shape into the
  object). `{ __proto__: Base }` where `Base` is a class links the static object.

Result: **910/54/569 → 918/54/561**, **+8 TP / 0 FP / −8 FN**, F1
**74.50% → 74.91% (+0.41pp)**, precision **94.40% → 94.44%**. `prototypes.json`
**0/2 → 2/0**, `prototypes3.json` **2/6 → 6/2**, plus +2 in other cases using these
links. Regression test `real_ts_pipeline_resolves_prototype_and_dynamic_proto_links`.
Full `cargo test -p polint --lib`: 2243 passed. Deferred (harder, separate infra):
`Object.create(Object.getPrototypeOf(a))` (needs `getPrototypeOf`), and `super.X()`
**inside** an object method whose prototype is set dynamically (needs object-method-
body walking with `super` bound, as the class-body walk does).

Cumulative this session (iterations 41–49): **821/82/658 → 918/54/561**, F1
**68.94% → 74.91% (+5.97pp)**, precision **90.92% → 94.44%**, **+97 TP / −28 FP**.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 50 | Logical/conditional receivers — `(o1 \|\| o2).f()` resolves precisely (`\|\|`/`??` short-circuit to the definite left, `&&` to the right, `? :` is the union) | 922 | 54 | 557 | 94.47% | 62.34% | 75.12% | 86605 ms | `8d2f4d71a23bc4a2` |

Iteration 50 resolves member access on a logical/conditional expression
(`receiver-callee-mixup.json`, a **0-TP** bucket). `object_targets_from_expression`
gained `LogicalExpression`/`ConditionalExpression` arms. A first pass naively unioned
both operands and **added 2 FP** — Jelly is precise: `o1 || o2` with a definite
(truthy) `o1` is `o1`, so `(o1 || o2).f()` is `o1.f` only, not both. Corrected to
short-circuit semantics: `||`/`??` prefer the left operand (fall back to the right
only when the left doesn't resolve), `&&` prefers the right, and only `cond ? a : b`
takes the union (both branches reachable). This is correct JS semantics, not a
score-fit — the union variant scored the same TP but produced wrong edges.

Result: **918/54/561 → 922/54/557**, **+4 TP / 0 FP / −4 FN**, F1
**74.91% → 75.12% (+0.21pp)**, precision-neutral. `receiver-callee-mixup.json`
**0/8 → 4/4**. Regression test
`real_ts_pipeline_resolves_logical_and_conditional_receivers` (asserts `o2.f` is
**not** a target of `(o1 || o2).f()`). Full `cargo test -p polint --lib`: 2244
passed. Deferred: `this.g()` dispatched **through** a logical receiver (the method
body walk keys on a named receiver; the remaining 4 FN need it to accept a
logical-expression receiver).

Cumulative this session (iterations 41–50): **821/82/658 → 922/54/557**, F1
**68.94% → 75.12% (+6.18pp)**, precision **90.92% → 94.47%**, **+101 TP / −28 FP**.

| Iteration | Change | TP | FP | FN | Precision | Recall | F1 | Runtime | Hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| 51 | `for-in` computed member calls — `for (const k in obj) obj[k]()` dispatches to every property value | 926 | 54 | 553 | 94.49% | 62.61% | 75.31% | 84123 ms | `5e2c2fb0a4959270` |

Iteration 51 handles `for-in` loops, which `collect_statement` previously skipped
entirely (the body was never walked, so `for-in.json` was **0-TP**). The loop head
binds its variable into a new `env.forin_keys` set; a computed call `obj[k]()` keyed
on such a variable ranges over every own property, so it resolves to the union of
the object's property values (and getters). `for-in.json` **0/4 → 4/0**, fully
localized, precision-neutral. Regression test
`real_ts_pipeline_resolves_for_in_computed_member_calls`. Full
`cargo test -p polint --lib`: 2245 passed.

**Considered and rejected this iteration (quality over score):** `Object.getPrototypeOf(x)`
≈ `x`'s shape would close `prototypes3.json`'s last 2 FN (`Object.create(getPrototypeOf(a))`),
but it introduced a false positive — `b.foo()` after `Object.setPrototypeOf(b, {…})`
**replaced** the prototype still saw the stale member, because the flattened object
model merges rather than tracking a replaceable prototype slot (Jelly is
flow-sensitive here). Reverted; it needs flow-sensitive prototype tracking to be
FP-free, and a +2 TP / +1 FP trade is not worth shipping an over-approximation.

Cumulative this session (iterations 41–51): **821/82/658 → 926/54/553**, F1
**68.94% → 75.31% (+6.37pp)**, precision **90.92% → 94.49%**, **+105 TP / −28 FP**.

Remaining module-modeling work (next iterations):

1. **Cross-file function-return summaries** — the dominant remaining `helloworld`
   gap and the `const app = express(); app.get(...)` pattern.
2. **Dependency precision** — `helloworld` still carries 529 of the suite's 609
   false positives from intra-dependency over-approximation.
3. **`client*` / namespace method calls** — namespace-object method resolution
   and class-export instantiation.

### Exploration: cross-file function-return summaries (reverted — no benchmark movement)

Implemented and measured, then reverted. A per-function return summary (callable
values + returned object shape) was harvested for every function during the same
bounded fixpoint and stored globally by `FunctionId`; `object_targets_from_call`
/ `collection_targets_from_call` then seeded the result of calling a function
defined in another file. A focused multi-file regression (factory returning an
object with a method, and a curried function returning a closure) passed.

The release Jelly suite was **byte-identical** to iteration 36 (`863 / 609 / 616`,
hash `4ffb6b2eacf2971c`, +4s runtime) — **zero** new edges. Diagnosis: the cases
this was meant to unlock each need a *dependent* mechanism the return summary
alone does not supply:

- **express `app.get` / `res.send`** — `createApplication` builds `app` via
  `mixin(app, proto)` (dynamic property copy), so its return shape carries none
  of the methods. Needs `mixin`/`Object.assign`-style dynamic property modeling.
- **`client1` `filter(cb)(arr)`** — the scored edge is `iteratee(x) → cb` *inside*
  the returned closure; identifying the closure is not enough, the argument `cb`
  must flow into the closure's captured parameter. Needs closure parameter-capture
  flow, not just return identity.
- **`client4`/`client5` `__importDefault(require(...)).default()`** — the wrapper
  returns a ternary `(mod && mod.__esModule) ? mod : { default: mod }` over a
  *parameter*; harvesting returns in a fresh scope cannot evaluate it. Needs
  parameter-sensitive return evaluation (interop helper modeling).

Conclusion: cross-file return identity is necessary but not sufficient. The next
attempt should pair it with (a) dynamic property-copy modeling (`Object.assign`/
`mixin`) and (b) parameter-capture flow through returned closures, validated
against `client1` and a `mixin` fixture *before* re-introducing the global
return-summary map. Carrying the un-paired infrastructure was not justified.

### Exploration: class-body call-walking with `this`-binding (reverted — precision-negative)

Implemented and measured, then reverted. A pass walked each class method and
constructor body for call resolution with `env.this_object` bound to the
instance (or static object), plus a frontend fix giving private methods
(`#bar`) their own `FunctionFact`s (`method_name` previously dropped
`PrivateIdentifier` keys) and value-flow resolution of `this.#foo()` /
`Class.#baz()` (`PrivateFieldExpression` callees keyed by `#name`). A focused
single-file regression resolved `this.foo()`, `this.helper()`, `this.#foo()`,
`this.#bar()`, `Class.#baz()`, and `Class.#qux()`. The full lib suite stayed
regression-free.

The release Jelly suite moved **867 → 871 TP / 609 → 614 FP** (F1 58.68% →
58.78%) — **+4 TP but +5 FP**, and *only* `tests/micro/private.json` moved
(2/0/10 → 6/5/6). The +5 FP are a caller-span convention mismatch: polint's MIR
owns a constructor's call sites by the `constructor()` `FunctionFact`
(`14:5:21:6`), so emitted `fun2fun` edges carry that caller, but Jelly attributes
constructor-body (and field-initializer) calls to the **class** node
(`1:1:22:2`). The rendered caller is `site.caller` (set in MIR lowering), so it
cannot be remapped at emit time — reconciling it needs a MIR call-site-ownership
change (lower constructor bodies under the class fact), which risks the working
`classes`/`classes2` cases. Every other class case (`this`, `super*`, `dpr-this`)
needs an *independent* deep mechanism (returned-arrow `this`-capture, `super`
resolution, class-from-function expressions, flow-sensitive reassignment), so the
primitive only reached `private.js`. Net precision-negative for one obscure case;
reverted.

Verdict for the class/`super`/`this` cluster: it is **not** an iterative seam.
The enabling prerequisite is reconciling MIR call-site ownership with Jelly's
class-node attribution for constructors and field initializers; only after that
does walking class bodies pay off. That is a focused MIR/identity change, not a
value-flow tweak.

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

Remaining largest recall blockers after iteration 34:

| Case | TP | FP | FN | Dominant missing capability |
|---|---:|---:|---:|---|
| `tests/helloworld/app.json` | 115 | 530 | 227 | CommonJS dependency/module object semantics and dependency precision |
| `tests/approx/simple.json` | 21 | 1 | 16 | broader object/property and callback value flow |
| `tests/micro/classes.json` | 61 | 6 | 16 | super/prototype/object identity flow and call-result object flow |
| `tests/micro/generators.json` | 38 | 7 | 12 | precise generator sequencing, `yield` input values, object/class generator methods |
| `tests/micro/asyncawait.json` | 19 | 12 | 10 | async generator precision and awaited value propagation |
| `tests/micro/spread.json` | 22 | 3 | 8 | spread argument/object flow and callback value propagation |
| `tests/micro/more1.json` | 44 | 2 | 5 | broader object/property and callback value flow |
| `tests/approx/natives.json` | 29 | 3 | 4 | residual native span alignment and unresolved native callsites |
| `tests/approx/computedProperties.json` | 24 | 3 | 2 | residual dynamic computed-property cases |

The main `tests/micro/promises.json` fixture is now closed at **56 TP / 5 FP /
0 FN**. The remaining promise-like gap is narrower: residual `promises2`,
async-generator `next()` result objects, native Promise variants, and
object-shaped fulfillment values such as `{ value, reason }` in paths that do
not yet enter the benchmark graph. Class/prototype work should move out of local
syntactic heuristics and into the existing object/points-to substrate so
receiver side effects such as `q1.a1(); q1.a2();` can be represented.

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

Iteration 27 added a bounded call-result object model. When a resolved object
method returns `this`, an object literal, an object binding, or another modeled
object-returning call, the value-flow pass can use that returned object for the
outer member lookup. This makes `k1.a4().a2()` resolve to `a2` in the real TS
pipeline regression. Verification: **23 `ts_value_flows` tests passed** and the
release benchmark gained **+9 TP / -9 FN** with FP unchanged. `tests/micro/classes2.json`
moved from **74 TP / 2 FP / 2 FN** to **75 TP / 2 FP / 1 FN**. The remaining
`classes2` mismatch is not missing object flow anymore: the extracted inner
`k1.a4()` member call and outer `k1.a4().a2()` synthetic function call share
`CallSiteId(93)`, so the benchmark renderer still attributes an `a4` target to
the outer span and misses the inner-span `a4` edge.

Iteration 28 fixed that structural ID collision in TS MIR lowering. TS call-site
IDs are now derived from `(start_byte, end_byte)` instead of only `start_byte`, so
nested same-start calls like the inner `k1.a4()` and outer `k1.a4().a2()` no
longer collapse to one `CallSiteId`. Verification: **17 `lower_ts` tests passed**,
**23 `ts_value_flows` tests passed**, and the release benchmark gained **+10 TP /
-10 FP / -10 FN**. `tests/micro/classes2.json` is effectively closed at **76 TP /
1 FP / 0 FN**; the remaining row is only the `scoring_mode.scored_edge_count`
invariant. Unknowns increased from **2444** to **2525** because previously
collapsed nested call sites are now represented separately.

Iteration 29 made TS call-site spans Jelly-compatible for the parenthesized
forms in `tests/micro/call-expressions.json`. A shared internal normalizer now
keeps one whole-call wrapper for `(f())` / `(new f())`, trims redundant
callee-parentheses for `((f))()`, and starts callable IIFEs at the `function` or
arrow token. Inventory, semantic-graph token-flow indexing, MIR operation spans,
and `CallSiteId` construction now use the same normalized span. Verification:
**13 `ts::inventory` tests passed**, **18 `lower_ts` tests passed**, **23
`ts_value_flows` tests passed**, **60 `semantic_graph` tests passed**, and the
release benchmark gained **+18 TP / -18 FP / -18 FN** overall. The main win is
`tests/micro/call-expressions.json`, which moved from **24 TP / 22 FP / 21 FN**
to **45 TP / 1 FP / 0 FN**. The tradeoff is a small dependency-heavy
`helloworld` movement from **113 TP / 520 FP / 229 FN** to **110 TP / 523 FP /
232 FN**, so CommonJS/dependency precision remains the next hard problem.

Iteration 30 fixed a real-pipeline extraction mismatch behind the remaining
`tests/micro/promises.json` misses. The TS syntax layer now inventories
anonymous function and arrow arguments directly, traverses member-call objects
such as `Promise.resolve(...).then(...)`, and walks `throw`, `do-while`,
`for-in`, `for-of`, and `switch` bodies when collecting nested callables. MIR's
anonymous-function prepass now mirrors that coverage, so handlers inside loop
and switch bodies are lowered and available to value-flow. Verification: **25
`ts_value_flows` tests passed**, **18 `lower_ts` tests passed**, **179
`ts::tests` passed**, and the release benchmark gained **+48 TP / +6 FP / -48
FN** overall. The benchmark-visible win is `tests/micro/promises.json`, which
moved from **14 TP / 5 FP / 42 FN** to **56 TP / 5 FP / 0 FN**. `helloworld`
also moved from **110 TP / 523 FP / 232 FN** to **114 TP / 529 FP / 228 FN**,
which reinforces that more dependency edges are being discovered before the
CommonJS module-object precision problem is solved.

Iteration 31 added a bounded local model for
`Function.prototype.call/apply/bind`, matching the next isolated Jelly gap in
`tests/micro/fun.js`. The value-flow collector now records bound function values
with their bound `this` object/callable and prefix arguments, gives interpreted
same-file function invocations an `arguments` collection, evaluates
expression-bodied arrow returns, resolves nested callee calls such as `f()()`,
and rewrites `.call` / `.apply` receiver and argument lists only when the
receiver resolves to a known local function. Verification: **27
`ts_value_flows` tests passed**, **4 call extraction tests passed**, **18
`lower_ts` tests passed**, and the release benchmark gained **+44 TP / +7 FP /
-44 FN** overall. The main win is `tests/micro/fun.json`, which moved from **9
TP / 1 FP / 36 FN** to **43 TP / 2 FP / 2 FN**. This confirms Jelly's native
`call/apply/bind` handling was a real recall blocker, but the small FP rise
shows we should keep future native models tightly scoped to known local values
or a proper points-to substrate.

Iteration 32 added a flow-insensitive sync/async generator iterator value model.
Function flows now record whether they are generators; generator calls and
collection `.values()` calls can seed iterator value sets; `.next()` produces an
object with a callable `value` property; `for-of` over known iterator variables
binds those values; `yield*` delegates to generator or collection value sets;
and anonymous callables inside `yield` / `await` are inventoried in both TS facts
and MIR. Verification: **28 `ts_value_flows` tests passed**, **179 `ts::tests`
passed**, **18 `lower_ts` tests passed**, and the release benchmark gained **+30
TP / +16 FP / -30 FN** overall. The main wins were `tests/micro/generators.json`
from **20 TP / 1 FP / 30 FN** to **38 TP / 7 FP / 12 FN** and
`tests/micro/asyncawait.json` from **7 TP / 2 FP / 22 FN** to **19 TP / 12 FP /
10 FN**. This is intentionally flow-insensitive: it improves recall but does not
model generator sequencing or distinguish yielded values from returned values in
all iterator contexts, so the remaining generator work should add a more precise
iterator-result abstraction before broadening further.

Iteration 33 added bounded native object/array models and constant computed
property key flow. The value-flow collector now records string literal
bindings, evaluates simple string concatenations for computed member writes,
stores collection-valued object properties, treats `Object.create(null)` as an
empty object, models `Object.assign`, `Object.getOwnPropertyDescriptor`,
`Object.getOwnPropertyDescriptors`, `Object.defineProperty`, and
`Object.defineProperties`, and extends array factory/prototype modeling for
`Array.of`, `concat`, `flat`, `filter`, and `slice` through object properties.
Verification: **29 `ts_value_flows` tests passed** and the release benchmark
gained **+65 TP / +0 FP / -65 FN** overall. The main target,
`tests/approx/natives.json`, moved from **1 TP / 2 FP / 32 FN** to **29 TP / 3
FP / 4 FN**. This also improved nearby object/collection-flow fixtures:
`tests/micro/more1.json` moved to **44 TP / 2 FP / 5 FN**,
`tests/micro/spread.json` moved to **22 TP / 3 FP / 8 FN**, and
`tests/approx/simple.json` moved to **21 TP / 1 FP / 16 FN**.

Iteration 34 made the computed-property model real enough for Jelly's
`tests/approx/computedProperties.json` shape. The value-flow collector now
tracks boolean constants and string arrays, evaluates bounded string unions for
computed keys in object literals and class members, records computed class
method function facts in the TS adapter and MIR lowering, separates getter and
setter targets from ordinary callable properties, and models getter reads as
zero-argument calls with the receiver object bound as `this`. Verification:
**30 `ts_value_flows` tests passed**, `ts::tests` passed, `lower_ts` tests
passed, and the release benchmark gained **+26 TP / -5 FP / -26 FN** overall.
The target fixture moved from **8 TP / 4 FP / 18 FN** to **24 TP / 3 FP / 2
FN**. This confirms there is no parsing blocker for the common static computed
property cases: Oxc exposes the keys, and the missing piece was bounded
semantic flow plus callable identity recovery.

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
- `tests/micro/call-expressions.json` is no longer a span-normalization blocker:
  iteration 29 closed all expected edges in that fixture. Remaining exact-span
  concerns are now dependency-heavy rather than a standalone micro-fixture issue.

Current best per-case movement:

| Case | Before TP/FP/FN | Current TP/FP/FN | Current note |
|---|---:|---:|---|
| `tests/micro/call-expressions.json` | 10 / 28 / 35 during direct-call baseline | 45 / 1 / 0 | module body, IIFE identity, constructor lowering, unique call-site IDs, and Jelly-compatible parenthesized spans closed all expected edges; only the scoring-count invariant remains |
| `tests/micro/classes.json` | 8 / 15 / 69 after module/IIFE work | 61 / 6 / 16 | class/static/prototype/self-alias flow, class constructor identities, nested callable inventory, and method span normalization helped; remaining misses are super/object flow and call-result object flow |
| `tests/micro/classes2.json` | 0 / 11 / 76 after module/IIFE work | 76 / 1 / 0 | constructor/static/this-alias, receiver-bound side effects, class constructor identities, nested callable inventory, method span normalization, returned-`this` object flow, and unique nested call-site IDs closed all expected edges; only the scoring-count invariant remains |
| `tests/micro/promises.json` | 0 / 0 / 56 at baseline | 56 / 5 / 0 | Promise executor/handler flow plus real-pipeline nested callable extraction under member calls, loop bodies, switch cases, and throws closed the fixture |
| `tests/micro/fun.json` | 9 / 1 / 36 before native function-method work | 43 / 2 / 2 | bounded local `call`/`apply`/`bind`, returned callable values, expression-bodied arrow returns, and `arguments` modeling closed nearly all function-method misses |
| `tests/micro/generators.json` | 20 / 1 / 30 before sync generator work | 38 / 7 / 12 | sync generator calls, delegated yields, `.next().value`, array iterator `.values()`, and generator returns recovered many iterator value calls, with sequencing precision still missing |
| `tests/micro/iterators.json` | 0 / 0 / 65 at baseline | 61 / 11 / 4 | collection element flow recovered almost all iterator value calls |
| `tests/micro/more1.json` | 0 / 1 / 49 at continuation start | 44 / 2 / 5 | native object/array models, computed property keys, set/map/Array.from/object/direct-param flow recovered most plain higher-order cases |
| `tests/micro/rest.json` | 6 / 1 / 38 at continuation start | 38 / 10 / 6 | array/object destructuring plus rest parameter flow closed most of the fixture |
| `tests/micro/asyncawait.json` | 1 / 2 / 28 after dependency-inclusive run | 19 / 12 / 10 | async IIFE/await/async-return flow plus generator iterator value modeling recovered most edges; precision needs a non-flat iterator-result model |
| `tests/approx/natives.json` | 1 / 2 / 32 before native object/array work | 29 / 3 / 4 | Object native descriptor/copying, Array native factories/prototype methods, and computed property string flow recovered most native value calls |
| `tests/approx/computedProperties.json` | 8 / 4 / 18 before computed object/class work | 24 / 3 / 2 | bounded key evaluation, computed class method identities, getter reads, and nested object property propagation recovered almost all static computed-property calls |
| `tests/approx/simple.json` | 15 / 1 / 22 before native object/array work | 21 / 1 / 16 | object/collection property propagation picked up additional value calls without FP growth |
| `tests/micro/spread.json` | 13 / 4 / 17 before native object/array work | 22 / 3 / 8 | collection-valued object properties and native array propagation recovered additional indexed/spread-adjacent calls |
| Full Jelly micro suite | 8 / 6 / 1471 | 840 / 604 / 639 | much better, still recall-limited by modules, async/generator precision, and object/property flow; FP pressure remains dominated by dependency/module modeling |

Next high-leverage iteration:

1. Add CommonJS/ESM module-object and dependency execution modeling for the
   `helloworld` gap without exploding false positives on dependencies.
2. Represent Promise and async-generator fulfilled values as objects, not only
   direct function collections.
3. Feed class/prototype/static field facts into the existing object/points-to
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

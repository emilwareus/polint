# Jelly FN decomposition — 2026-06-15 (baseline 1144/47/335, F1 85.69%)

Method: `cases[].matches[]` with `item_kind=="graph_edge"`, `outcome=="false_negative"`,
from `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`. 335 FN =
**189 call2fun** (real call sites) **+ 146 fun2fun** (aggregate mirrors, auto-convert with
their call-site sibling → ~1.7× TP per call-site closed).

**Oracle is the DYNAMIC runtime ground truth** (actual executed edges), NOT Jelly's static
output. So FN = a real runtime edge we miss; FP = an edge we emit that didn't execute.
Jelly's own static model over-approximates against this same oracle (it has FPs too).

## Top-level split

| Slice | call2fun | fun2fun | total |
|---|---:|---:|---:|
| **helloworld** (express dep tree) | 75 | 57 | **132** |
| non-helloworld tail | 114 | 89 | **203** |

## Tail call2fun by mechanism bucket (114 total)

| Bucket | ~c2f | Fixtures | Tractability |
|---|---:|---|---|
| **arrays / collections** | ~32 | spread 8, arrays5 4, more1 3, arrays2 3, arrays3 3, arrays4 2, arrays 2, arrays4 2, natives 2, classes(static) | **HIGH-YIELD, self-contained in heap.** Index cells (specific∪unknown), push/pop, forEach/map/reduce callbacks, spread, for-of, Array.from. This is THE structural lever. |
| **super / static class fields** | ~11 | super 5, super4 3, super3 2, super5 1 | Tier-4; `super.m()`, static field init, super-call. No `Super`/`StaticBlock` model. |
| **promises / async / generators** | ~11 | promises2 5, generators 3, asyncawait 2, promiseall 1 | Tier-4; no producer→consumer heap model. |
| **cross-module class/export** | ~7 | client2/4/5/9, import12, dyn-import 2 | In-analyzer cross-file class export / ESM default. |
| **IIFE self-span** | ~6 | throw, super5, asyncawait, promises2, fun, arrays3 | Call-site span includes parens, target span doesn't — documented +8-FP shared-span trap. |
| **object-literal method aliasing** | ~6 | obj2 4 (`Object(x)` identity native), assign2, destructuring | obj2: model `Object(arg)` native (identity for objects, fresh otherwise) → +4. |
| **this-binding / receiver** | ~5 | receiver-callee-mixup 2, client-this, more1, dpr-this | Object-literal-method `this`, callback `this`-flow. |
| **misc niche** | ~8 | private (#foo), accessors6, arguments 3, default-parameter 2, mix (Map.get), mocha/pirates 7 | Each 1-3, bespoke. |

## helloworld (132) — express dependency internals

Pruning-gated express entry chain; multi-prerequisite, benchmark-only feedback (~4min/iter).
Documented in `jelly-express-object-model-chain`. Not this session's target.

## Decision

Attack **arrays/collections** (largest tractable tail bucket, ~32 c2f + fun2fun mirrors).
Jelly's exact model reverse-engineered from the cloned tool source
(`research/evaluation-harness/repos/jelly/src/analysis/operations.ts`,
`src/natives/ecmascript.ts`, `nativehelpers.ts`):

- Per array token `t`: specific-index cells `t."i"`; `t.%ARRAY_UNKNOWN`; `t.%ARRAY_ALL`
  (= ⋃ index cells + UNKNOWN, maintained by listener).
- Literal `[e0,e1]` (≤10 elts): `ei ⊆ t."i"`; `...spread` → `iter(src) ⊆ t.%UNKNOWN`, flips
  rest to UNKNOWN. >10 elts → all UNKNOWN.
- Read `arr[i]` (const): `t."i" ∪ t.%UNKNOWN`; `arr[dyn]`: `t.%ALL`.
- Write `arr[i]=v` (const): `v ⊆ t."i"`; `arr[dyn]=v`: `v ⊆ t.%UNKNOWN`.
- push/unshift/fill → arg ⊆ `t.%UNKNOWN`; pop/shift/at → returns `t.%ALL`.
- concat/slice/splice/copyWithin/flat → new R; `t.%ALL ⊆ R.%UNKNOWN` (+ args).
- forEach/map/filter/find/some/every/reduce/flatMap: callback param0 = `t.%ALL`, param2 = `t`,
  thisArg bound; map/flatMap collect cb-return into result.%UNKNOWN.
- values()/entries()/for-of/Array.from: flow `t.%ALL` to iterator value / loop var / result.

The heap is **additive** on top of the recognizer (which already smears arrays → owns the
~29 array FP). An index-SENSITIVE heap model adds the precise edges the recognizer misses
(recall) while its over-approx edges largely overlap recognizer FP already counted. The prior
−11 TP regression was specific-index-ONLY *in the recognizer* (removed the union); the index-
insensitive blur *in the heap* was +1/+10 FP. The correct model is specific∪unknown in the heap.

## RESULT (implemented + validated, 2026-06-15): 1144/47/335 → 1152/47/327

**+8 TP, 0 new FP, FN −8. Precision 96.05%→96.08% (held), recall 77.35%→77.89%, F1
85.69%→86.03% (+0.34pp).** Landed in `js_points_to/{solver,harvest}.rs`.

**THE decisive lesson — the oracle is DYNAMIC, so reads that pick ONE index must stay
precise; only genuine ITERATORS (for-of, forEach/map callbacks) touch every element and
stay precise against the dynamic oracle.** Three increments measured:
- Full Jelly model (specific∪unknown reads + dynamic→ALL + concat/slice + pop→ALL + fill):
  1148/**75**/331 — +28 FP. The read-side unions/dynamic reads/pop/concat over-approximate
  one runtime index → REVERTED those pieces.
- Precision-safe core (literal index cells; const-index read = specific ONLY, no union;
  push/unshift→%UNKNOWN; forEach/map/filter/find/some/every/reduce/flatMap callbacks bind
  param0=%ALL; for-of binds loop var=%ALL; map/flatMap collect cb-return): 1152/53/327.
  Then dropped `fill` (range no-op pollutes %ALL, pure FP): 1152/53/327→ still 6 over.
- **for-of block-scope fix** (the loop variable is block-scoped; the heap was function-scoped
  so a reused `const f` in a sibling `for…of` inherited the prior loop's element binding →
  6 FP in iterators.js set/Set loops). Push a fresh scope per `for…of` + bind the loop var
  freshly: 1152/**47**/327. ← landed.

**What did NOT convert (left as FN, all hard/non-precision-safe):** `arr[const]∪%UNKNOWN`
recall (arrays2 `array[2]→push`, more1 `a14[i]` flatMap-result index) needs the smeary union;
arrays4/arrays5 forEach-with-param callbacks are **reachability-pruned** (the callback body's
edge resolves but heap edges are non-roots, and the native forEach→cb edge is deliberately
suppressed to avoid an FP not in the dynamic oracle → no root seeds the body); spread-into-args
& object-spread index copy (spread.js q[0]/q2) are smeary/unmodeled. Set/Map for-of already TP
via the recognizer.

## FN PRUNING DIAGNOSTIC (2026-06-15) — `POLINT_JELLY_NO_PRUNE=1`

Added an env-gated bypass of the reachability filter (`normalize_kernel_output`,
`eval/external/jelly_callgraph.rs`). A no-prune run flips every *computed-but-pruned* FN to TP,
so diffing the FN sets partitions the misses. At 1152/47/327:
- **no-prune: 1246/1125/233** → **94 FN are computed-but-pruned** (un-prune ceiling),
  **233 are unresolved** (resolution ceiling). The +1078 FP is why pruning exists.

**Computed-but-pruned (94 = 51 c2f + 43 mirrors):** helloworld 78, arrays5 7, promises2 4,
arrays4 4, super5 1. helloworld's 78 unlock via **express-chain RESOLUTION** of a few entry
links (`app.init`→`defaultConfiguration`, res/req prototype dispatch); once a link resolves,
reachability propagates and un-prunes the subtree. The 16 tail are genuinely-invoked native/
host callbacks needing **root-seeding** (risky — the iter-53 +48-FP class).

**Unresolved (231 = 132 c2f + 99 mirrors):** helloworld 32 (depth) + ~100 fragmented tail
(spread 8, classes 6, super 5, srcLoc 5, obj2 4, rest 4, more1 4, super4 3, generators 3,
promises2 3, dynamic 3, … each a bespoke per-mechanism model). No single big clean bucket.

**Conclusion:** the F1 ceiling lives in helloworld (110 FN: 78 pruning-gated behind a few
express links + 32 depth) — a dedicated multi-iteration project with benchmark-only feedback.
The tail is fragmented per-mechanism.

## RESULT 2 (2026-06-15): `Object()` identity native → 1157/47/322

`Object(x)` returns `x` for an object argument; modeled in the heap as result ⊇ x's tokens (so
`o2 = Object(o1); o2.g = g; o1.g()` resolves) + a fresh wrapper token (primitive args / writes
on the result). **+5 TP, 0 new FP, FN 327→322. F1 86.03%→86.25% (+0.22pp)**, precision 96.10%.
Gate test `points_to_heap_resolves_object_coercion_identity`.

## STEP 1 DONE — reachability-frontier diagnostic → ranked bridges

Built `/tmp/frontier.py` over the pruned + no-prune JSONs: oracle `fun2fun` = real call graph,
no-prune observed = resolved graph, pruned observed callers = reachable set. A **bridge** =
oracle edge `R(reachable) → U(unreachable)` not yet resolved; ranked by the resolved-closure it
un-prunes. Result (5 bridges, all express):

| impact | closure | R (reachable) | U (pruned) |
|---:|---:|---|---|
| 5 | 15 | `createApplication` (express.js:37) | `app.init` (application.js:57) |
| 3 | 6 | `app[method]` (application.js:473) | `Route.prototype[method]` (route.js:193) |
| 3 | 7 | `app.lazyrouter` (application.js:137) | `proto.use` (router/index.js:434) |
| 0 | 1 | `app[method]` | `proto.route` (router/index.js:497) |
| 0 | 3 | `app.lazyrouter` | `proto` (router/index.js:43) |

## RESULT 3 (2026-06-15): mixin keystone → 1177/49/302

Bridge #1: `mixin(app, proto)` (merge-descriptors) merges `proto`'s methods onto `app`, so
`app.init()` resolves and reachability propagates into the 15-function `defaultConfiguration`
subtree. Modeled in the heap as **prototype inheritance** (`Constraint::Inherit`, reusing
`link_prototype`): `Object.assign(t, …s)` / `mixin(t, s)` (a `require('merge-descriptors')`
binding) link `t` to inherit each source. The chained-export aliasing
(`var app = exports = module.exports = {}`) already flows in the heap, so `proto.init` was
reachable once the inherit linked it.

**1157/47/322 → 1177/49/302: +20 TP, +2 FP, FN −20. F1 86.25%→87.02% (+0.77pp)** — the
session's biggest single move. Precision 96.10%→96.00% (held at the floor). The +2 FP is
collateral: un-pruning `defaultConfiguration` exposed a pre-existing recognizer wildcard smear
(`this.on('mount', …)` → `app[method]`), not a new heap edge. Gate test
`points_to_heap_resolves_mixin_merged_method`. Bridges #2/#3 (Route/router method dispatch)
remain — candidate next increments.

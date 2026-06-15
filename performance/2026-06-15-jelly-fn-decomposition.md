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

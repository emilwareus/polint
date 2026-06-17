# Jelly callgraph: false-negative categorization, best wins, and solutions

*2026-06-17. Branch `emilwareus/nuuk` (PR #76). State: **1219 TP / 49 FP / 260 FN**,
precision 96.14%, recall 82.42%, F1 88.75%. Source of truth: the release graph
benchmark (see [`jelly-benchmark-setup`] memory).*

This report inventories **every remaining false negative**, partitions them by the
two fundamental failure modes, groups them into ten mechanism categories, and for
each gives a concrete capture strategy with expected impact and precision risk.
It closes with a ranked plan.

---

## 1. Where we are

This branch closed the class/super/object-method cluster (+32 TP, 0 net FP,
F1 87.44→88.75%). Fully resolved: `super.js`, `super3.js`, `super4.js`,
`receiver-callee-mixup.js`. The work is **recall-bound** (260 FN ≫ 49 FP) and
precision sits on the 96% floor — every future change must net-increase TP
without dropping precision below ~96% (FP ≤ ~50).

The 49 FP decompose as: **27 array-`%ALL`-smear** (documented trap, off-limits),
**11 helloworld** (express dead-branch edges), **11 scattered single FPs**.

## 2. The fundamental split (no-prune diagnostic)

Running the benchmark with `POLINT_JELLY_NO_PRUNE=1` (bypasses the reachability
filter) gives **1295 / 1137 / 184**. Diffing the FN sets against the pruned run
(1219/49/260) partitions the 260 FN:

| | count | meaning | lever |
|---|------:|---------|-------|
| **computed-but-pruned** | **76** | we resolve the edge; reachability prunes it | un-pruning / reachability |
| **genuinely unresolved** | **182** | never produced even with pruning off | resolution mechanism |
| (2 unparsed spans) | 2 | edge-case keys | — |

**Key implication:** 76 FN are *already resolved* and need only a reachability
unlock; 60 of those 76 are express. The other 182 need new resolution logic.

## 3. The ten categories

Exact per-category tally (pruned/unres split in parentheses):

| # | Category | FN | pruned | unres | Representative call sites |
|---|----------|---:|-------:|------:|---------------------------|
| **A** | express + npm deps (helloworld) | 108 | 60 | 48 | `new Router({…})`, `flatten(slice.call(arguments,…))`, `res.send(…)`, `getter()` |
| **I** | cross-module | 31 | 0 | 31 | `arit.sum(1,2)`, `lib5b.default()`, `f(a)` (lib callback), dynamic `import()`, pirates `addHook` |
| **C** | array-index / positional | 23 | 11 | 12 | `a14[2]()`, `array[2]()`, `arr6[0]()`, `x.pop()()` |
| **B** | dynamic / computed member | 20 | 0 | 20 | `x5[v]()`, `a[b]()`, `o._foo.bar()`, `b.foo()` |
| **J** | class / ctor / param tail | 20 | 1 | 19 | `new C5(f1)`→`x()`, `new F1`, default params, `{a}=…`, `this.#foo()` |
| **H** | paren / computed-key / call-result | 19 | 0 | 19 | `( creator())`, `(Foo()[0][k]())`, `new B()()`, `y["My"+"Fn"]()` |
| **D** | spread / rest | 14 | 0 | 14 | `{...q}`→`q2.p1()`, `[...xs]`→`q[0]()`, `...args` |
| **F** | async / promise | 11 | 4 | 7 | `resolveWithFun(resolve)`, `f1()`→arrow via promise, async IIFE |
| **E** | generators / iterators | 6 | 0 | 6 | `i1.next().value()`, `q()`, `t()` |
| **G** | arguments object | 6 | 0 | 6 | `arguments[i]()`, `arguments.callee()`, arrow capturing `arguments` |

---

## 4. Per-category evaluation and capture strategy

### A — express + npm deps (108: 60 pruned / 48 unresolved). The ceiling.
Two distinct sub-problems:

- **60 pruned** — factory/curried-internal calls (`getter()`, `compileTrust(…)`,
  http-errors constructor chain, `flattenForever(…)`) that *resolve* but sit in
  the pruned express-init subtree. **Capture:** the express object-model keystone
  (member-method `this`-binding) un-prunes the subtree, lighting up all 60 at once.
  **Blocked:** it adds 8 dynamically-dead FP (`gettype()` in a `throw` branch ×2;
  `flattenWithDepth` recursion dead under `depth==null` ×6) → precision 95.5%.
  **Solution = dead-branch path-sensitivity** (below), not more resolution.
- **48 unresolved** — `Router` instantiation/dispatch, `res`/`req` prototype
  methods resolved cross-file, and `flatten(slice.call(arguments, offset))` (needs
  `Function.prototype.call/apply` arg-array spreading + the `arguments` object).
  **Capture:** deeper express prototype/object modeling. Multi-iteration.

### B — dynamic / computed member (20, all unresolved).
`x5[v]()` (computed key from a variable), `a[b]()`, `o._foo.bar()` (chained
member), `b.foo()` (prototype method). These are property→function resolutions
where the key is computed or the property was assigned dynamically.
**Capture:** field-sensitivity in the Andersen points-to heap
(`analysis/calls/js_points_to/`) for computed keys and dynamic property writes.
The heap already models some of this; this extends it. **Precision:** moderate —
must keep computed-key reads precise (don't union all properties on an unknown key).

### C — array-index / positional (23: 11 pruned / 12 unresolved). Mostly leave.
`a[i]()`, `arr[0]()`, `x.pop()()`. This is the documented **`%ALL` smear trap**:
precise per-index resolution trades TP for FP against the 27 array FP, because
Jelly itself models `pop()`→`%ALL`. **Capture:** only worthwhile if the array
model is reverse-engineered to match Jelly's exact `∪`/unknown-index routing — a
high-risk precision project. The 11 pruned are reachability, not the array model.
**Recommendation: defer; local optimum.**

### D — spread / rest (14, all unresolved).
Object-spread property copy (`{...q}`→`q2.p1()`), array-spread element flow
(`[...xs]`→`q[0]()`), rest params (`...args`). **Object-spread is the clean half**
(copy own-properties of the spread source into the target object). The
array-spread half overlaps the array trap (and the 2 spread FP from
argument-position misalignment). **Capture object-spread; defer array-spread.**

### E — generators / iterators (6, all unresolved).
`i1.next().value()` / `q()` / `t()` — yielded/returned values reached through
`.next().value()` sequencing. **Capture:** extend the existing sequence-sensitive
generator model (see [`jelly-generator-sequencing`] memory). Self-contained,
0-FP-risk. ~6 TP.

### F — async / promise (11: 4 pruned / 7 unresolved).
Promise-executor callbacks (`resolveWithFun(resolve)`, pruned), `then`/`await`
result flow (`f1()`/`f2()`→arrow), async IIFE. **Capture:** promise/async
value-flow — flow the executor's `resolve` argument and `await` results.
**Precision:** one known async-self-loop FP to guard. Medium.

### G — arguments object (6, all unresolved).
`arguments[i]()`→i-th actual arg, `arguments.callee()`→enclosing function, and an
arrow capturing the outer `arguments`. **Capture:** model the `arguments` object
as a positional tuple of the call's actual arguments. Self-contained, niche. ~6 TP.

### H — paren / computed-key / call-result (19, all unresolved).
- **Call-result / new-result invocation:** `new B()()`, `(Foo()[0][k]())`,
  `f()()` — invoke the value a call/new returns. Partially modeled; generalize.
- **Constant-string computed keys:** `y["My"+"Fn"]()` — resolve a computed key
  that is a constant string expression to the literal property.
- **Paren unwrapping:** `( creator())`, `(x[q] ())` — already unwrapped for field
  values (`field_value_expression`); generalize to call positions.
**Capture:** several clean small wins here. ~8–12 TP, low risk.

### I — cross-module (31, all unresolved).
`arit.sum(1,2)`, `lib5b.default()` (ESM default), `f(a)` (library callback param
`this`/arg flow), dynamic `import()` (`dyn-import.mjs`), and the pirates
require-hook (monkey-patch — very hard). The benchmark runs with
`AnalysisPlan::empty()` (no module graph), so cross-module resolution leans on the
`oxc_resolver` path + per-file export summaries. **Capture:** stronger cross-file
export-summary seeding + callback-parameter flow + dynamic-import resolution.
Medium-hard; pirates is a special case to skip.

### J — class / ctor / param tail (20: 1 pruned / 19 unresolved).
The bespoke remainders after this branch's cluster:
- `new C5(f1)`→ctor-param→`super(param)` chain (classes.js `x()→f1`) — needs
  **`new`-argument → constructor-parameter flow**, which then chains through the
  `super(arg)` infra already built. **Guard required** (the `new f` attempt
  over-resolved npm deps, +4 FP, reverted).
- `new F1` (new on a function-valued var) — same guard concern.
- parameter defaults (`default-parameter.js`), destructured function bindings
  (`destructuring.js`), private-field arrows (`private.js` `this.#foo()`),
  accessors, super5 IIFE-in-ctor.
**Capture:** per-construct value-flow, ~2–4 TP each. Mostly clean but fragmented.

---

## 5. The strategic shape

- **~42% of all FN are express (A).** It is the only path past ~90% F1, but it is
  **precision-gated**. Dilution cannot unlock it: absorbing its +8 FP at the floor
  needs ~+150 clean TP first, but only ~100 clean FN remain. **Therefore the
  express unlock is a precision problem — kill the dead branches — not a
  resolution problem.**
- The **resolution tail (B/E/G/H/J ≈ 71 FN)** is the steady, low-risk, precision-
  safe grind to ~90%.
- **C (array) and the array half of D** are a precision local optimum — defer.

## 6. Best wins now (ranked)

**Tier 1 — clean, precision-safe, do first (~30–40 TP → F1 ≈ 89.5–90%):**
1. **H: call-result/new-result invocation + constant computed-string keys**
   (~8–12 TP). Generalizes machinery already present; low risk.
2. **J: parameter defaults, destructuring bindings, private-field arrows,
   accessors** (~8–12 TP). Each small and self-contained.
3. **E: generator `.next().value()` sequencing** (~6 TP). Extends existing model.
4. **G: `arguments` positional model** (~6 TP). Self-contained.
5. **B: computed-key / dynamic-property field-sensitivity in the points-to heap**
   (~10–15 TP). Higher leverage; needs care to stay precise on unknown keys.
6. **J: `new`→ctor-param flow** (classes.js `x()→f1`, ~+2–4) — **only with a tight
   guard** (the `new f` revert).

**Tier 2 — precision engineering (the enabler):**
7. **Dead-branch path-sensitivity** — suppress edges in `throw`-only branches and
   the `depth==null` flatten branch. Kills the 8 express dead FP. This is the gate.
8. **(stretch) array `%ALL` model** — match Jelly's index routing; frees ~27 FP →
   precision ~98%, which *also* creates headroom for express.

**Tier 3 — the flagship (gated on Tier 2):**
9. **express object-model keystone** — un-prunes the 60 pruned + resolves much of
   the 48 unresolved. **+50–70 TP → F1 ≈ 91–92%.** Only affordable once Tier 2
   removes the dead FP.

## 7. Potential solutions — concrete

- **Path-sensitivity (Tier 2, the highest-value unlock).** Tag call sites that
  appear *only* inside a `throw` argument during MIR lowering
  (`analysis/mir/lower_ts.rs`), and skip seeding/propagating reachability through
  them in `jelly_reachable_caller_spans` (`eval/external/jelly_callgraph.rs`).
  Reachability is OR'd across all sites, so a function reachable via a non-throw
  path stays reachable — this only removes the *dead* edges. The `depth==null`
  branch needs a lightweight value-condition model (harder); even the throw-only
  half kills 2 FP and the `throw.js` bridge cleanly.
- **Generator/arguments/call-result (Tier 1).** All three are value-flow additions
  in `analysis/calls/ts_value_flows.rs` following the established three-layer
  pattern (frontend fact + MIR call site + value-flow resolution). `arguments` and
  call-result are span-keyed emits like the work already shipped.
- **Computed-property field-sensitivity (B).** Extend `js_points_to/{harvest,
  solver}.rs` to track `obj[constKey]` and dynamic `obj[k]=fn` writes as precise
  field cells; keep unknown keys from unioning all properties (precision guard).
- **`new`→ctor-param flow (J).** Aggregate all `new ClassName(args)` argument
  targets per class and bind them to the constructor parameters during the class
  walk, reusing `bind_call_arguments_to_flow` and the `super(arg)` chain already
  built. **Guard:** only bind when the callee is a known local class/function
  (the `new f`-on-arbitrary-value attempt over-resolved `new Buffer`-style npm
  calls, +4 FP — reverted).

## 8. Discipline (unchanged)
Bench-gate every increment; add a regression test per construct; revert anything
net-negative or byte-identical; keep FP ≤ ~50. Pure value-flow edits are
cache-gated — bump the discriminator in `analysis/calls/cache_key.rs` (digest fn
+ its mirror test) or the benchmark serves stale.

[`jelly-benchmark-setup`]: ../../.claude memory
[`jelly-generator-sequencing`]: ../../.claude memory

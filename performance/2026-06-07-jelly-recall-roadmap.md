# Research Report: Moving the Needle on Jelly Recall — 2026-06-07

## Executive summary

The Jelly JS/TS call-graph benchmark has moved from **F1 57.48%** (PR #66 baseline)
to **F1 68.94%** over iterations 35–40, and the problem has fundamentally changed
shape. **Precision is solved (90.9%); the entire remaining gap is recall.**

Current release checkpoint:

| Suite | TP | FP | FN | Precision | Recall | **F1** |
|---|---:|---:|---:|---:|---:|---:|
| Jelly JS/TS micro | 821 | 82 | 658 | 90.9% | 55.5% | **68.94%** |

Two facts drive everything below:

1. **It is one app plus a long tail.** `helloworld` (a real Express app, 1 of 76
   cases) is **292 of the 658 FN (44%)**. The other 75 fixtures sit at
   **F1 0.78 / recall 0.68 / precision 0.91** — already close to Jelly.
2. **The recall gap is a value-flow-coverage gap, not a precision/soundness gap.**
   With precision at 91% there is real headroom: each true positive recovered
   lifts F1 directly, and a few false positives per feature are affordable.

This report decomposes the 658 FN, gives the mechanism / expected impact / effort
for each, and lays out a prioritized roadmap. The short version of the
recommendation: harvest the tractable clusters (class/super, destructuring,
this-flow) for ~+120 TP / **F1 → ~73%** first; treat `helloworld` as a dedicated
multi-mechanism push; and recognize that the long tail ultimately argues for a
small private JS value-token heap rather than more one-off recognizers.

---

## 1. Method and what "recall" means here

- **Suite:** `research/evaluation-harness/suites/jelly-callgraph-micro.toml` over the
  pinned Jelly checkout (`b799ed4f`), release tier. Run via
  `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release
  -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture`.
  Artifacts in `.context/graph-benchmarks/`.
- **Scoring (oracle-jelly, reachability-pruned).** As of iteration 39 the adapter
  filters observed edges to those whose caller is reachable from the
  module-execution roots plus value-flow-invoked callbacks (see iteration 39 in
  the gap-closure log). This matches Jelly's demand-driven oracle and is why
  precision jumped to 91%. **Consequence for recall: an edge polint resolves in a
  function it cannot connect to the entry is pruned, counted as FN.** This is the
  reachability/recall coupling that makes `helloworld` "double-leverage" (§3.1).

---

## 2. The recall gap, decomposed

Excluding `helloworld`, the suite is `771/76/366` → **F1 0.777, recall 0.678**.
The 658 FN by theme:

| Theme | FN | Cases | Dominant missing mechanism |
|---|---:|---:|---|
| **helloworld** (framework object model) | 292 | 1 | cross-file returns + `mixin`/`Object.assign` property-copy + closure capture + `methods.forEach` |
| **"other"** (fragmented value flow) | 173 | 35 | curried/factory returns, parenthesized/sequence callees, object-property propagation |
| **class / `super`** | 74 | 12 | class-expression-from-function (`super4`/`super5`), super/object/call-result flow (`classes`) |
| **destructuring** | 47 | 5 | default values, computed keys, nested patterns, getters-in-patterns |
| **this-flow** | 38 | 4 | object-method `this`, returned-arrow `this`-capture |
| **async / generator** | 34 | 5 | generator sequencing, awaited-value object precision |

Top individual FN cases: `helloworld` (292), `approx/this` (16), `super4` (16),
`classes` (15), `srcLoc` (14), `destructuring` (13), `deconstruction` (12),
`mochatest/test-with-hook` (11), `asyncawait` (10), `generators` (10), `super` (10),
`super5` (10), `this` (10), `approx/simple` (9).

---

## 3. Theme-by-theme: mechanism, impact, effort

### 3.1 helloworld / framework object model — 292 FN (the long pole)

**The chain that must resolve:** `const app = express(); app.get('/', cb);
app.listen(PORT)`.

- `express` is `module.exports = createApplication` (resolved today via the export
  summary — §iter 35).
- `createApplication()` builds `app` as `var app = function(){…}` then
  `mixin(app, proto)` (a `merge-descriptors` call that copies every property of
  `proto`, which is `require('./application')`, onto `app`). The methods `app.get`,
  `app.post`, … are then attached on `proto` via `methods.forEach(m => { app[m] = … })`.
- So `app.get` resolves only if polint models: **(a)** `express()`'s return value
  (cross-file function-return summary), **(b)** `mixin`/`Object.assign` as a
  dynamic property copy from one object onto another, **(c)** the `methods.forEach`
  dynamic-key assignment loop, and **(d)** the request/response objects similarly.

**Double-leverage.** Because scoring is reachability-pruned, today polint *already
resolves* ~70 helloworld edges in `application.js`/`request.js`/`response.js` but
they are **pruned as unreachable** (polint cannot connect those functions to the
entry through the unresolved `app.get`/`app.listen`). Resolving the chain above
therefore pays twice: it adds the new edges **and** un-prunes the ~70 existing
correct edges. The realistic ceiling here is large (helloworld has 160 cross-file
oracle edges) but each mechanism is non-trivial and they interlock.

- **Prerequisites:** cross-file function-return summaries (explored and reverted at
  iteration 37 because, alone, they moved nothing — they need the property-copy and
  closure pieces to land together); `Object.assign`/`mixin` modeling;
  computed/`forEach` dynamic property assignment.
- **Expected impact:** highest ceiling (could be +100–200 TP if fully landed), but
  highest effort and risk. **Best done as a dedicated push after the cheaper
  clusters, with a `mixin` fixture and `client1` closure fixture as the gates.**

### 3.2 class / `super` — 74 FN (highest-tractability coherent bucket)

Iteration 40 already built the enabling machinery: class-body walking with `this`
bound to the instance/static object, `super.m()/.s()/.f()` resolution against the
super class's objects, private members, and a value-flow `caller_override` so
constructor-body edges attribute to the class node (matching Jelly) at +1 FP.

What remains:

- **`super4`/`super5` (26 FN):** the class is an **anonymous `class extends A`
  returned from a function** — `function postMixin(){ return class extends A {…} }`,
  then `var a = postMixin(); var x = new a(); x.m()`. Needs: (1) collect class
  **expressions** (not only declarations) into the class table, (2) flow the
  returned class through `postMixin()` and `new a()` to an instance, (3) the
  existing super walk then resolves the internal `super.m()` edges.
- **`classes` (15 FN):** residual super/prototype/object-return and call-result
  flow (`k1.a4().a2()`-style on inherited members).
- **Effort:** moderate; reuses iteration-40 infrastructure. **Expected ~+30–40 TP →
  F1 ~70.5–71%, precision-neutral.** This is the recommended next step.

### 3.3 destructuring — 47 FN (most self-contained)

`destructuring.json` (13 FN) and `deconstruction.json` (12 FN) plus residue in
`spread`/`rest`. The missing forms, all in `bind_object_pattern`/`bind_collection_pattern`:

- **Default values:** `var {a: y1 = () => {}} = x` — Jelly binds `y1` to *both*
  `x.a` and the default function.
- **Computed keys:** `{["a"]: y2} = x` → `x.a`.
- **Nested object patterns:** `{b: {c: y4}} = x` → `x.b.c`.
- **Getter-valued sources:** `{bar: y5} = x` where `x.bar` is a getter returning a
  function.
- **Assignment-destructuring into members:** `({a: c.foo} = x)` (setter target).

**Effort:** low–moderate, contained to the pattern-binding helpers. **Expected
~+25–35 TP → F1 ~70–70.5%.** Good clean slice; lowest risk.

### 3.4 this-flow — 38 FN

`approx/this` (16), `this` (10), `dpr-this`. Two mechanisms:

- **Function-object `this`:** `f.g = function(){}; f.h = function(){ this.g() };
  f.h()` — `this` in `f.h` is `f`, so `this.g()` → `f.g`. Needs `this`-binding for
  function-with-properties receivers (the class-body walk handles class `this`, not
  this).
- **Returned-arrow `this`-capture:** `const o = { foo(){ return () => this.bar() },
  bar(){…} }; o.foo()()` — the arrow captures `this = o` lexically. Needs the value
  flow to carry the captured receiver into the returned arrow's body.

**Effort:** moderate (arrow `this`-capture is the harder half). **Expected ~+20–30
TP.**

### 3.5 async / generator — 34 FN

`asyncawait` (10 FN + 11 FP — the only notable FP bucket left), `generators` (10 FN).
The flow-insensitive generator/iterator model (iter 32) recovers most edges but not
**generator sequencing** (distinguishing successive `yield` values) or
**awaited-value object shapes** (`await gen.next()` → `{value}` object). Improving
this also trims the `asyncawait` FPs. **Effort:** moderate; **expected ~+15–20 TP
and −5–8 FP** (a rare precision-and-recall double win).

### 3.6 "other" — 173 FN (fragmented; mine for systematic sub-patterns)

35 cases, ~5 FN each. Not one fix, but recurring sub-patterns worth a sweep:

- **Curried / factory returns:** `lib.filter(cb)(arr)` (`client1`), `wrapped()()`
  (`srcLoc`) — needs cross-file/curried function-return flow (overlaps §3.1's
  return-summary prerequisite).
- **Parenthesized / sequence callees:** `(0, lib.foo)()` (`client3`),
  `((Foo()[0][k]()))` (`srcLoc`) — a callee-normalization extension (the iter-38
  span work is adjacent).
- **Object-property propagation depth:** `simple` (9 FN), `more1` (5),
  `natives` (4), `obj`/`obj2` — incremental object/value-flow depth.
- **`__importStar` namespace methods, `defineProperty`/`defineProperties` accessor
  flow, `for-in`/template-literal tags.**

**Effort:** diffuse; each sub-pattern is small. A focused sweep could harvest
~30–50 TP but with diminishing per-pattern returns. **Lower priority than the
coherent clusters.**

---

## 4. The architectural lever behind the long tail

Five iterations of this work (including two reverts — cross-file return summaries
at iter 37, class-body walking at the first attempt) point to one structural
finding: **polint resolves JS with bounded, per-syntax-form local recognizers,
while Jelly runs a single whole-program abstract-heap token-propagation fixpoint.**
Each recall mechanism above is, in Jelly, a few constraints over the *same* token
world (function tokens, object tokens, property variables, `this`/return/argument
variables). In polint each is a separate recognizer, which is why:

- Return summaries alone did nothing (they need the property-copy and closure
  constraints to be in the same propagation).
- Class-body walking needed a bespoke `caller_override` to match Jelly's node
  attribution.
- The "other" bucket is irreducibly fragmented under the recognizer model.

The high-leverage architectural move — explicitly recommended in
`2026-06-06-jelly-gap-closure-research.md` and reinforced here — is a **small
private JS value-token heap** (`crates/polint/src/analysis/...`, not the SDK):
function/object/array/promise/iterator/module tokens; storage variables for
lexical bindings, object properties, returns, `arguments`, `this`; subset/copy +
property read/write + call listeners propagated to a bounded fixpoint. Most of
§3.1, §3.4, and §3.6 collapse into "feed the heap the right constraints" once it
exists. This is a real project (multi-iteration), but it is the only path that
addresses the long tail and `helloworld` together rather than one fixture at a
time.

---

## 5. Prioritized roadmap

| Phase | Work | Δ TP (est.) | F1 (est.) | Effort / risk |
|---|---|---:|---:|---|
| **A** | Class/super completion (`super4/5` class-from-function + `classes` remainder) | +30–40 | ~70.5–71% | moderate / low (infra exists) |
| **B** | Destructuring (defaults, computed, nested, getters) | +25–35 | ~72% | low / low |
| **C** | this-flow (function-`this`, returned-arrow capture) + async/generator precision | +35–45 | ~73–73.5% | moderate |
| **D** | helloworld framework object model (returns + `mixin`/`Object.assign` + `forEach` + closure capture) — recovers pruned TP too | +100–200 | step change | high / high |
| **E** | Private JS value-token heap; migrate A–D's local models onto it; mine the "other" tail | large | toward Jelly | large / architectural |

Phases A–C are recommended as the immediate sequence (each a clean, measured,
precision-neutral iteration). D and E are the strategic pushes.

### Recommended next step
**Phase A — class/super completion**, because the iteration-40 machinery (class-body
walk, super resolution, `caller_override`) is in place; `super4`/`super5` are the
direct class-expression-from-function extension; and it is the largest coherent
non-helloworld bucket at high precision.

---

## 6. Guardrails for recall work

- **Hold precision ≥ 90%.** Headroom exists, but the reachability-pruned oracle
  means new FPs are real. Track per-case TP/FP/FN every iteration
  (`.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`).
- **Respect reachability coupling.** Edges added in functions unreachable from the
  entry will be pruned; prefer mechanisms that also connect callers (which is why
  §3.1 is double-leverage and why pure leaf-resolution can score zero).
- **Fixture-first.** Each mechanism gets a focused real-kernel regression test
  *before* the full Jelly case is the acceptance bar (the pattern used in
  iterations 35–40), and revert cleanly if a slice does not move the benchmark
  (as at iteration 37) rather than carrying un-paired infrastructure.
- **Keep the iteration log honest** in
  `2026-06-06-jelly-gap-closure-research.md`: TP/FP/FN, precision, recall, F1,
  runtime, hash per slice, including reverts.

---

## Sources

- Iteration log and per-slice evidence:
  `performance/2026-06-06-jelly-gap-closure-research.md` (iterations 35–40).
- Architectural analysis and the token-heap recommendation:
  same file, "Updated Implementation Plan" and §"What Jelly Is Optimized Around".
- Current per-case data: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`.
- Jelly source and papers: https://github.com/cs-au-dk/jelly,
  https://cs.au.dk/~amoeller/papers/jam/, https://cs.au.dk/~amoeller/papers/approx/.

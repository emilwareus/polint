# Research Report: The Remaining Jelly Recall Bucket — 2026-06-08

## Executive summary

After iterations 41–51 (this PR), the release Jelly JS/TS call-graph micro suite
sits at **926 TP / 54 FP / 553 FN — F1 75.31%, precision 94.49%, recall 62.61%**.
Precision is solved; the entire remaining gap is recall (553 FN). That gap splits
cleanly in two:

| Slice | TP | FP | FN | What it is |
|---|---:|---:|---:|---|
| **`helloworld`** (one real Express app) | 50 | 6 | **292 (53%)** | A framework object model **plus** the internal call graphs of ~20 npm dependencies |
| **everything else** (75 micro fixtures) | 876 | 48 | **261 (47%)** | The fragmented long tail of individual JS constructs |

The headline finding: **`helloworld` is not one mechanism, and not even mostly the
Express object model.** Only ~60–80 of its 292 FN are the famous
`app.get`/`app.listen` chain. **~210 are the ordinary internal call graphs of the
dependency tree** (`depd`, `debug`, `ipaddr.js`, `http-errors`, …) — the same
general JS the micro fixtures exercise, now at real-library scale. This reframes the
roadmap: closing `helloworld` is mostly *the long tail applied to real code*, which
is exactly the case for the architectural rework over more one-off recognizers.

---

## 1. Method

- Suite: `research/evaluation-harness/suites/jelly-callgraph-micro.toml`, pinned
  Jelly checkout `b799ed4f`, release tier (run command in
  the benchmark setup notes / `2026-06-06-jelly-gap-closure-research.md`).
- Per-case TP/FP/FN from `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`
  (`cases[].matches[]` where `item_kind == "graph_edge"`).
- Scoring is **oracle-jelly, reachability-pruned**: an edge polint resolves in a
  function it cannot connect to a module-execution root is pruned and counted FN.
  This is what makes `helloworld` "double-leverage" (§2.3).

---

## 2. The `helloworld` bucket (292 FN)

### 2.1 FN by target file — it is a dependency-tree problem

```
  59  node_modules/depd/index.js            (deprecation helper — its own closures/this)
  40  node_modules/debug/src/debug.js        (debug factory — curried returns, exports)
  39  node_modules/express/lib/application.js (the app object model)
  29  node_modules/ipaddr.js/lib/ipaddr.js   (prototype-based OOP at scale)
  17  node_modules/express/lib/response.js
  14  node_modules/http-errors/index.js
  10  node_modules/express/lib/utils.js
   8  node_modules/{content-type,mime,express/router/layer}.js …
```

Express's own files (`application.js`, `response.js`, `utils.js`, `router/*`) are
~90 FN; the **non-Express dependencies are ~200 FN**. Those dependencies are plain
JavaScript: `depd` builds deprecation wrappers with closures and `this`; `debug` is
a curried factory (`require('debug')('express:application')`); `ipaddr.js` is
classic `X.prototype.m = …` OOP. They miss for the *same reasons* the micro
fixtures missed before this PR — they are the long tail at scale.

### 2.2 The Express object-model chain (~60–80 FN) — the coherent core

The chain `const app = express(); app.get('/', cb); app.listen(PORT)` resolves only
if four interlocking mechanisms all land:

1. **Cross-file function-return summaries.** `express` is
   `exports = module.exports = createApplication` (`express.js:28`), and
   `createApplication()` (`express.js:37`) returns `app`. So `express()` must flow
   the module export *and* the function's return value across files.
2. **`mixin` / `merge-descriptors`.** `createApplication` builds
   `app = function(req,res,next){…}` then `mixin(app, proto, false)` where
   `mixin = require('merge-descriptors')` and `proto = require('./application')`
   (`express.js:42-43`). `mixin` copies **every own property descriptor** of `proto`
   onto `app`. This is structurally identical to the `Object.assign` merge **landed
   in iteration 48 of this PR** — but invoked through a required helper, so it needs
   either module-summary recognition of `merge-descriptors` or a named-helper model.
3. **`methods.forEach`.** The HTTP verbs are attached in a loop —
   `methods.forEach(method => { app[method] = function(path){…} })` — a dynamic
   computed-key assignment. The `for-in` computed-key work **landed in iteration 51**
   is the read side of this; the write side (`app[method] = …` over a loop variable)
   is the missing dual.
4. **`proto` is the `application.js` module**, whose methods (`app.use`,
   `app.handle`, `app.route`, …) are defined as `app.init = function(){…}` on the
   module's exported object — i.e. property-assignment-to-an-exported-object, then
   read cross-file.

### 2.3 Double-leverage (reachability pruning)

Because scoring prunes unreachable edges, polint **already resolves ~70 edges**
inside `application.js`/`response.js`/`request.js` that are currently counted FN
**only because** their functions cannot be connected to the entry through the
unresolved `app.get`/`app.listen` chain. Landing §2.2 therefore pays twice: it adds
the chain edges **and** un-prunes those ~70. This is why §2.2 is the highest-ceiling
single piece, and why pure leaf-resolution in the dependencies (§2.1) scores less
than its raw FN count until the entry chain connects.

### 2.4 What this PR already put in place

Two of the four §2.2 prerequisites now exist as general mechanisms:
- **Object/descriptor merge** (`Object.assign`, `Object.create`,
  `defineProperty/defineProperties`, prototype assignment, dynamic `__proto__`
  links) — iterations 47–49. `merge-descriptors` is the same operation behind a
  require.
- **Computed-key read over an iteration variable** (`for-in`) — iteration 51. The
  `methods.forEach` write is the symmetric dual.

Still missing for §2.2: **cross-file function-return summaries** (reverted twice
when attempted alone, iter 37 and again mid-session — they move nothing without the
property-copy + closure pieces, which now exist) and **closure parameter-capture**
(`filter(cb)(arr)` style).

### 2.5 The 6 `helloworld` FP

Localized intra-dependency over-approximation (e.g. `ipaddr.js` parenthesized-call
spans). Not descriptor-shape; precision is not the constraint here.

---

## 3. The non-`helloworld` tail (261 FN)

The micro fixtures still missing, grouped by the mechanism they need. None is a
clean single-construct win anymore — each needs infrastructure beyond a local
recognizer:

| Mechanism | Example fixtures | ~FN | Why deferred |
|---|---|---:|---|
| **Flow-sensitive prototype/`this`** | `prototypes3`, `accessors5`, `super` (object) | ~20 | `setPrototypeOf` *replaces* the prototype between two calls; the flattened model merges → FP (see iter 51 `getPrototypeOf` revert). Needs a replaceable prototype slot. |
| **Getter/setter + `this` chains** | `accessors5/6`, `defineProperty` residue | ~12 | `obj.bar = x` (setter) → `this._foo = x`; `t1 = obj.foo` (getter) → returns `this._foo`. Needs setter side-effect + getter-return threaded through `this`. |
| **`this`-dispatch through a computed/logical receiver** | `receiver-callee-mixup`, `super3` | ~10 | The method-body walk keys on a *named* receiver; `(o1\|\|o2).f()`'s `this.g()` needs it to accept an expression receiver. |
| **Cross-module class/namespace** | `client2/3/4/5` | ~20 | `new lib.Arit()`, `(0, lib.foo)()` over `require('./lib')`. The empty-plan benchmark doesn't run the module graph (see the benchmark setup notes); needs in-analyzer cross-file class export. |
| **Default-parameter call attribution** | `default-parameter` | 4 | Binding `cb = () => {}` is clean and general but the fixture's `f` is reachable only through another default; Jelly attributes default-initializer calls to the *enclosing declaration scope*. Plumbing-heavy for 4 FN. |
| **Curried / parenthesized / `arguments`** | `srcLoc`, `arguments`, `arrays3/4/5` | ~30 | "Notoriously difficult" parenthesization, curried returns, `arguments.callee`, native-callback-reassignment. Fragmented; Jelly-specific precision. |
| **Framework object models** | `test`, `test-with-hook` (mocha) | ~17 | Same shape as `helloworld` at micro scale. |
| **Long object/value-flow depth** | `simple`, `more1`, `obj2`, `library`, `dynamic`, … | ~130 | Incremental object/property/return depth — the part that only a propagation fixpoint closes cheaply. |

The bottom row is the key one: **~130 of the 261 tail FN are "just a bit more
value-flow depth"** — exactly what a unified heap gives for free and what local
recognizers give one painful constraint at a time.

---

## 4. The architectural lever

Seven iterations this session (plus two reverts) reconfirm the structural finding
from `2026-06-06-jelly-gap-closure-research.md`: **polint resolves JS with bounded,
per-syntax-form local recognizers reading/writing a flat heap-of-maps (`FlowEnv`),
while Jelly emits constraints into one whole-program token-propagation fixpoint.**
Under the recognizer model each construct is a separate hand-written change, and
features compose only where the maps are explicitly threaded together. That is why:

- `helloworld` is mostly the long tail at scale (§2.1) — no single recognizer closes
  it.
- Flow-sensitivity (prototype replacement, two-state `this`) cannot be expressed
  without a different state model (§3).
- ~130 tail FN are pure value-flow depth (§3) that a fixpoint closes for free.

The high-leverage move is a **small private JS value-token heap** (under
`crates/polint/src/analysis/…`, not the SDK): function / object / array / promise /
iterator / module tokens; storage variables for lexical bindings, object
properties, returns, `arguments`, `this`; subset/copy + property read/write + call
listeners propagated to a bounded fixpoint. Most of §2.1, §3's depth rows, and the
flow-sensitivity cases collapse into "feed the heap the right constraints." It is a
multi-iteration project, but it is the only path that addresses `helloworld` **and**
the tail together instead of one fixture at a time.

---

## 5. Recommended roadmap

| Phase | Work | Est. ΔTP | Effort / risk |
|---|---|---:|---|
| **A** | Express object-model chain (§2.2): cross-file return summaries **paired** with `merge-descriptors`/`mixin` (reuse the iter-48 merge) + `forEach` computed-key **write** (dual of iter-51) + closure param-capture. Gate on a `mixin` fixture and a `client1` closure fixture **before** re-introducing the global return-summary map. | +60–80 here, plus ~70 un-pruned (§2.3) | high / high — but two of four prerequisites now exist |
| **B** | Dependency-internal long tail (§2.1) — falls out of Phase A's reachability un-pruning **and** the value-flow-depth work below | large | medium, follows A |
| **C** | Tail flow-sensitivity & getter/setter-`this` chains (§3 rows 1–3) | ~40 | medium; needs a replaceable prototype slot + `this`-threaded accessors |
| **D** | **Value-token heap** (§4); migrate A–C onto it; the ~130 depth FN close | large | large / architectural — the real endgame |

**Recommended next step:** Phase A, done as a *gated* push (fixtures as acceptance
gates, revert cleanly per the iter-37 lesson if a slice doesn't move the benchmark),
because its two hardest prerequisites — dynamic property copy and computed-key
handling — landed in this PR, and it carries the §2.3 double-leverage. Phase D is the
strategic endgame once Phase A proves the constraints out on real code.

---

## Sources

- This PR's iteration log: `performance/2026-06-06-jelly-gap-closure-research.md`
  (iterations 41–51).
- Prior gap analysis and the token-heap recommendation:
  `performance/2026-06-07-jelly-recall-roadmap.md`.
- Per-case evidence: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`.
- Express object model: `tests/helloworld/node_modules/express/lib/{express,application}.js`
  in the pinned Jelly checkout.

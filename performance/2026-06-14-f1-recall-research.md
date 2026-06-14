# Research: How to further increase F1 on the Jelly JS/TS call-graph benchmark — 2026-06-14

## Executive summary

Current: **1076 TP / 44 FP / 403 FN — precision 96.07%, recall 72.75%, F1 82.80%.**

**F1 is now entirely recall-bound.** Fixing *all 44* FP raises F1 only to 84.2% (+1.4pp);
converting 100 of the 403 FN raises it to 87.1% (+4.3pp), and closing helloworld alone
(189 FN) reaches **90.8%**. So the whole F1 program is now a recall program: chase the
403 FN, not the 44 FP.

Two structural facts shape *how*:

1. **The reachability-root rule is the master constraint** (`eval/external/jelly_callgraph.rs:230-281`).
   Scoring is reachability-pruned. Only `FunctionTokenFlow` recognizer edges (emitted
   from a *proven-walked* body, `ts_value_flows.rs:570-576`) and synthetic module
   functions seed roots. **Points-to (`PointsTo`) edges never seed roots** — by design,
   because the global fixpoint resolves edges inside dead code and rooting them re-opens
   the iter-53 +48-FP explosion. Consequence: **the heap can resolve a dependency's
   internal call graph but it scores zero until a recognizer root connects the entry.**
   So recall splits into: *recognizer = connect entries (un-prune); heap = supply depth
   inside reachable code.* Neither alone closes helloworld.

2. **403 FN = 239 real call-sites (`call2fun`) + 164 aggregate mirrors (`fun2fun`).**
   The mirrors auto-convert when their call-site sibling resolves, so each fix yields
   **~1.7× TP** per call-site closed. The "real work" is ~239 resolutions.

## The FN map

| Slice | FN | Nature |
|---|---:|---|
| **helloworld** (1 Express app) | **189 (47%)** | Internal call graphs of ~20 real npm deps at scale |
| non-helloworld tail (~46 micro fixtures) | 214 | Cross-cutting JS-construct mechanisms |

helloworld by dependency: express→express 62, ipaddr.js 29, http-errors 18, depd 30
(across express/send/http-errors callers), send 8, body-parser/content-type/etag/… ~16.

## The ranked opportunities (evidence-backed, de-duplicated across investigations)

Effort/risk are relative; FN estimates include un-pruning + fun2fun multipliers where
noted. File:line evidence in the per-area notes below.

### Tier 1 — recognizer "root-seeding" fixes (un-prune real-library call graphs)
These are the biggest recall bucket (helloworld), mostly independent and self-contained
in `ts_value_flows.rs`, and each carries double-leverage (un-pruning).

| # | Mechanism | ~FN | Where | Effort/Risk |
|---|---|---:|---|---|
| 1 | **depd: factory returns a closure carrying assigned function-properties** — `function depd(ns){ function deprecate(){…}; deprecate.function = wrapfunction; return deprecate }`. The export summary captures the callable but drops its `.function`/`.property` members, so every `deprecate.function(...)` site misses. Fold property-assignments on the returned binding into `ModuleExportSummary`/`function_return_summary` (`ts_value_flows.rs:184-195, 634-657`). Cross-cuts express/send/http-errors/body-parser. | ~19 | recognizer | med / low-med |
| 2 | **express: `var X = Object.create(proto); module.exports = X` is not an export alias** — so response.js/request.js method bodies (`res.send`/`res.set`/`res.get`/`res.type`, `req.get`) are never `this`-walked. Extend `expression_aliases_exports` / the declarator handling (`ts_value_flows.rs:7056, 1636-1641`) to alias `Object.create(...)`-then-`module.exports` modules and handle chained `res.set = res.header = fn`. | ~26 | recognizer | low |
| 3 | **express: `app.init()` from `createApplication` is unresolved → prunes the whole `init → defaultConfiguration → this.set/this.enable/debug` subtree.** Keystone: the mixin merge (`collect_merge_helper_call`, `ts_value_flows.rs:3142`) builds `env.objects['app']` but the `app.init()` member-call doesn't connect to it; cascades into application.js + router once fixed. | ~22 | recognizer | med |
| 4 | **ipaddr.js (and any CoffeeScript lib): nested-IIFE class** `X = (function(){ function X(){}; X.prototype.m=fn; return X })()`. `collect_function_declarations`/`collect_class_declarations` scan only top-level `program.body` (`ts_value_flows.rs:1120,1171`), so the inner constructor is never registered → `new X()` can't root. Descend into `var X = (function(){…return X})()` bodies; scope narrowly to avoid FP. (Pairs with the heap prototype-link, #8.) | ~29 | recognizer (+heap) | med |
| 5 | **http-errors: `codes.forEach(cb)` over an opaque/imported collection** — the callback body is walked only when the collection is a tracked binding (`ts_value_flows.rs:2841`); `codes` is imported, so the body (and its `createClientErrorConstructor`/`createServerErrorConstructor`/`toIdentifier` calls + their cascade) is never rooted. Walk the inline/named callback body even when the collection is unknown, attributing inner calls to the callback (reachability still requires the outer call live). Must be gated — FP-prone. | ~14 | recognizer | low-med / med |
| 6 | **debug/depd curried-factory return** — `require('debug')('ns')` must return the inner `function debug()`; `require('depd')('ns')` the inner `deprecate`. Extend `callable_return_targets_from_call` (`ts_value_flows.rs:3977`) for factories that declare-and-return an inner function. (Overlaps #1's mechanism; the heap already flows `ret→result` in `solver.rs:522`, so wiring the inner-fn return in `harvest.rs` could catch debug there too.) | ~12 (debug) | recognizer or heap | med |

### Tier 2 — heap "depth" fixes (cheap, field-sensitive, low-risk)
The heap is correctly scoped and already closes value-flow depth, curried/object returns,
higher-order params, function-objects, const computed keys, ES6 classes. Its missing
high-FN constructs:

| # | Mechanism | ~FN | Where | Effort/Risk |
|---|---|---:|---|---|
| 7 | **Constant-key dynamic property, producer side** — consumer machinery exists (`harvest.rs:821-862` folds `"a"+k` keys); the misses are `this[name]=fn` inside a constructor needing the field-store to land on the `new`-instance token (approx/this, receiver-callee-mixup, private `#foo`). Wire `this` dyn-property stores into `Construct`'s instance token. | ~26 (tail) | heap | low |
| 8 | **Function-style `C.prototype.m = fn` dispatch** — the heap links prototypes only for ES6 `class` (`harvest.rs:741-795`); a function token's `"prototype"` field is never tied to its `Construct` prototype token, so `new C().m()` doesn't inherit. ~30-50 lines reusing existing `set_class_prototype`/`Construct` plumbing. Directly addresses ipaddr.js (pairs with #4 for the root). | ~29 (overlaps #4) | heap | low-med |
| 9 | **`Object.assign`/`mixin`/merge-descriptors property copy** — no native model; `mixin(app, proto)` is an opaque call. A field-sensitive "copy every property cell from arg-base into target" model (~200 lines, plan §P3) is clean in the heap, messy in the recognizer's flat maps. The *resulting* `app.get` still needs a recognizer root (#3). | enabler | heap | med |

### Tier 3 — the big structural project
| # | Mechanism | ~FN | Where | Effort/Risk |
|---|---|---:|---|---|
| 10 | **Arrays (index/positional/spread/pop/at/map)** — the heap has *zero* array support (`harvest.rs:462` array literal → fresh empty cell; `:639` spread → single cell; `solver.rs` fields string-keyed only). Largest single bucket (~26 tail FN **+** the 31-FP array bucket). The naive specific-index attempt **regressed −11 TP** (memory: `jelly-fp-decomposition`). Needs Jelly's exact "specific-index ∪ unknown-index" routing + per-method transfer functions, reverse-engineered first, bench-gated per increment. The heap (field-sensitive element cells) is the right home. | ~26+ | heap | high |

### Tier 4 — high effort, lower yield (defer)
- **Static blocks / static-field init + `super` chain** (~22 tail FN, classes/super): both
  subsystems have *zero* handling (no `StaticBlock`/`Super` in harvest or recognizer).
- **Promises/async/generators producer→consumer** (~16 tail FN): no heap model.
- **Cross-module class/namespace** (client2/4/5, ~6 FN): in-analyzer cross-file class export.
- **pirates `require`-hook monkeypatch** (mocha test-with-hook, 4 FN): real-world node lib;
  not worth modeling.
- **Flow-sensitivity / strong-updates** (computedProperties, accessors, arguments, private):
  needs a different state model; large change for ~5-8 FN.

## Recommended sequencing (with F1 projection)

The order maximizes recall-per-risk and exploits un-pruning + fun2fun multipliers.

1. **Tier-1 recognizer fixes #1, #2, #4(+#8), #5, #3, #6** — the helloworld dependency
   tail. Independent, self-contained, root-seeding. Estimated **~100-130 TP** with
   un-pruning and fun2fun mirrors → **F1 ≈ 86-88%**. Do them as separate benchmark-gated
   landings (revert any that doesn't net-win, per the iter-37 / array lesson).
2. **Tier-2 heap depth #7, #8** — cheap, low-risk, field-sensitive. ~30-40 TP → **F1 ≈ 88-89%**.
3. **Tier-3 arrays #10** — the big project, carefully. High ceiling (closes the last big
   recall **and** precision bucket together), high risk; do only after reverse-engineering
   Jelly's array semantics and gating each increment.
4. **Tier-4** — only if pushing past ~90% F1.

## What NOT to do
- **Do not make `PointsTo` edges reachability roots.** High effort, re-opens the iter-53
  +48-FP class; the heap's non-root status is a feature. Un-prune via recognizer roots.
- **Do not chase the 44 FP for F1.** Precision is solved (96%); FP work is ≤1.4pp of F1.
- **Do not retry specific-index-only arrays** (documented −11 TP regression).
- **Do not attempt cross-file return summaries alone** (reverted twice; they move nothing
  without the property-copy + closure pieces — which, for depd, is exactly #1).

## ADDENDUM (same day, post-implementation-attempt): the dominant blocker is NESTED FUNCTIONS

All per-fix Tier-1 attempts were **byte-identical** (no movement) and reverted. Two
fundamental blockers explain why, and reframe everything above:

**Blocker A — nested functions are never emitted as FunctionFacts.** The frontend
`extract_declarations` (ts/adapter.rs:1109) and MIR `collect_functions`
(mir/lower_ts.rs:259) recurse into bodies only to harvest *anonymous* callables +
class methods — a **nested function declaration** (`function depd(){ function
deprecate(){} return deprecate }`) or **named function expression** inside a body is
never emitted. No FunctionFact → no FunctionId → NEITHER the recognizer NOR the
points-to heap can emit an edge to it (the heap maps tokens→FunctionId by span;
`function_value`/`arrow_value` early-return when no fact exists, so the body isn't even
walked). **Measured: 190 of 239 call-site (call2fun) FN have a nested-function target
— and 118 of 119 *non-helloworld* FN do (≈99% of the tail).** This is THE recall
lever, not one bucket among many.

**Blocker B — helloworld is reachability-root-pruned** (proven: export-alias +
env.objects-seeding both byte-identical; depd-internal FN sit inside `wrapfunction`,
pruned). The express entry chain needs all 4 prerequisites at once; PointsTo edges are
non-roots so the heap can't un-prune dependency internals.

**Interaction (the key insight):** emit nested functions → the **heap converts the
edge automatically** (span-keyed, lexically scoped). For the **non-helloworld tail the
call sites are already reachable** (module-top-level), so heap edges there **score
directly** → ~118 call2fun FN (+ fun2fun mirrors) convert *without* a recognizer root.
For **helloworld** the same emission is necessary but **not sufficient** — those bodies
are still root-pruned until the express chain connects. So nested-function emission is
(a) a large standalone tail win and (b) the prerequisite that makes the helloworld
Tier-1/express work resolvable at all.

**Implementation shape (M difficulty, ~60-120 LOC, 2 layers):** (1) idempotency guard
in `push_ts_function` keyed on (file,span) — `push_ts_function` does NO dedup today,
unlike `push_ts_class` (adapter.rs:1961); this is the #1 correctness item. (2) emit
`push_ts_function` at the nested `FunctionDeclaration` site (adapter.rs:1413) AND add
the matching `TsFunctionCandidate` in MIR (lower_ts.rs:432) so its body gets a MirBody
+ Call sites (call sites come ONLY from MIR Call ops, extract.rs:14-90) — frontend and
MIR names MUST agree (matching_function, lower_ts.rs:158). (3) bump TS_CACHE_SCHEMA v11
→v12 and TS_SYNTAX_LAYER_SCHEMA v9→v10. Heap/provider need NO changes (auto-converts).
A handful of TS unit-test count assertions will shift. Gate each increment on the
benchmark; precision risk = new heap-FP targets + any new FunctionTokenFlow roots
(moderate) — revert any non-net-winning increment.

**Precision lever (separate):** the 44 FP are ~29 array-positional structural
(specific∪unknown-index model — the documented −11 TP regression trap, also closes ~26
array FN), 7 helloworld dead-branches (honest), 2 computedProperties + misc
flow-sensitivity. The array model is the only sizeable precision lever and is a
high-risk recall+precision project on its own.

**Next steps, ranked:**
1. **Nested-function emission** (frontend + MIR, gated) — biggest recall lever; converts
   most of the tail for-free via the heap; prerequisite for helloworld. START HERE.
2. **Express object-model chain** (helloworld root-seeding) — only after #1; multi-iteration.
3. **Array specific∪unknown-index model** — combined recall+precision; high-risk, its own project.

## RESULT (implemented + validated, same day)

Lever #1 landed in two benchmark-gated increments — **the session's biggest move**:
- **1a. Nested function DECLARATIONS** (frontend `extract_declarations` nested-`FunctionDeclaration`
  arm → `push_ts_function` + a (file,span) idempotency guard on `push_ts_function`; MIR
  `collect_anonymous_functions_from_statement` → `collect_function`; schemas v11→v12 / v9→v10):
  **1076/44/403 → 1127/46/352** (+51 TP, +2 FP). F1 82.80→84.99%.
- **1b. Nested function EXPRESSIONS** (`include_self` false→true for nested var-init functions
  in frontend + MIR; schemas v12→v13 / v10→v11): **1127/46/352 → 1144/47/335** (+17 TP, +1 FP).
  F1 84.99→85.69%.

**Cumulative (this turn): 1076/44/403 → 1144/47/335 — +68 TP, −68 FN, +3 FP, precision held
~96.05%, recall 72.8→77.4%, F1 82.80→85.69% (+2.89pp).** 2268+ lib tests green; clippy clean.

Surprise vs. prediction: the gain was mostly **helloworld** (189→132 FN, its *reachable* nested
functions — ipaddr IIFE classes, depd factory internals — convert via the heap), not the tail
(which mostly needs recognizer resolution the heap can't do). "helloworld fully pruning-gated"
was too strong. Small collateral: `client-this` 2→6 FN (cross-module callback this-flow).

Remaining nested-function increments (untested): member-assigned function expressions
(`obj.m = function(){}` inside bodies), object-literal methods inside bodies, recognizer
name-scoping (un-prunes more helloworld — heap-only resolves them today), then the express
chain and the array model.

## Method / sources
- F1 leverage + FN decomposition: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`
  (`cases[].matches[]`, `item_kind=="graph_edge"`). 403 FN = 239 call2fun + 164 fun2fun.
- Four parallel root-cause investigations (express internals; ipaddr/http-errors/depd;
  non-helloworld tail mechanism buckets; points-to heap capability/limits) — 2026-06-14.
- Prior (pre-heap): `performance/2026-06-08-jelly-remaining-bucket-research.md`,
  `2026-06-07-jelly-recall-roadmap.md`. Memory: `jelly-fp-decomposition`,
  `jelly-points-to-heap`, `jelly-express-object-model-chain`.

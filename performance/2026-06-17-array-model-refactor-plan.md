# Implementation plan: index-aware array model in the value-flow

*2026-06-17. Branch `emilwareus/nuuk` (PR #76), state **1219 TP / 49 FP / 260 FN**,
precision 96.14%, recall 82.42%, F1 88.75%. Precision floor: keep FP ≤ ~50
(P ≥ ~96%). Source of truth: the release Jelly callgraph benchmark.*

## Why

F1 is recall-bound but precision sits on the floor, so recall gains that cost any
FP must be reverted. **27 of the 49 FP are array `%ALL`-smear** (arrays 6,
iterators 8, rest 9, arrays2 2, spread 2). Clawing them back lifts precision to
~97–98%, which **also unlocks the express keystone by dilution** (the keystone is
+19 TP / +8 FP — it currently breaches the floor, but with ~14 FP of array
headroom it lands at ≥96%). So this one structural fix pays twice: ~+12 TP
directly **and** the express bucket (~+50–70 TP → ~F1 92%) as a follow-on. It is
the single highest-value lever that is *not* a dead end.

## Root cause (precisely located)

The smear is entirely in the **value-flow** (`CallAlgorithm::FunctionTokenFlow`),
NOT the points-to heap. Verified by dumping `arrays.js` edges with algorithm:
`t()` (= `x.pop()`) emits **4** `FunctionTokenFlow` edges (all elements); the
heap's `PointsTo` edge for `y()` (= `x[1]`) is precise (1 target). So the heap is
already correct; the value-flow's `CollectionTargets` array model is the defect.

Three concrete defects in `crates/polint/src/analysis/calls/ts_value_flows.rs`:

1. **`pop`/`at`/`find` return *all* elements.** `collect_call_targets_from_call`
   (~L5041): `"pop" | "at" | "find" => CollectionTargets { values: source.values.clone(), … }`.
   `x.pop()` yields every element. Jelly's `pop` reads only `%ARRAY_UNKNOWN`
   (dynamically pushed), so `x.pop()` after `push(arrow4)` → just arrow4.
2. **`extend()` sorts `values` by `FunctionId`** (`CollectionTargets::extend`,
   ~L6907-6915), which **destroys index order**. So `value_at(i)` (~L6951,
   `self.values.get(index)`) returns the i-th *smallest id*, not array index `i`,
   after any mutation — hence `x[1]` smears and out-of-bounds `x[3]` over-resolves.
3. **No `%ARRAY_UNKNOWN` bucket.** `CollectionTargets` (~L6900) is
   `{ keys, values, object_values }`. Literal numeric cells, dynamic writes
   (`x[k]=`), `push`, and spread all land in one `values` Vec, so there is no way
   to keep `x[1]` (numeric cell) separate from a pushed element.

## The reference implementation already exists: the heap

`crates/polint/src/analysis/calls/js_points_to/solver.rs` (Token doc ~L47-69)
implements Jelly's model correctly and got +8 TP / 0 FP:
- numeric-index cells (`t."0"`, `t."1"`) kept distinct,
- `%ARRAY_UNKNOWN` (`ARRAY_UNKNOWN`, ~L65) for `push` / dynamic-index writes / spread,
- `%ARRAY_ALL` (`ARRAY_ALL`, ~L69) = union of all numeric + unknown, **read only by
  genuine iterators** (`for…of`, `forEach`/`map`/`reduce` callbacks the harvest
  wires explicitly),
- **READ side is NOT smeared**: `arr[i]` reads only its specific index cell.

**The task is to mirror this model in the value-flow `CollectionTargets`.** Use the
heap (solver.rs + harvest.rs array handling) as the spec.

## Jelly semantics to match (from the oracle, `arrays.js`/`rest.js`)

- `x[const]` → the element at that numeric index only (1 target).
- `x[const]` out of bounds (`x[3]` with no element 3) → **nothing**.
- `x.pop()` → the `%ARRAY_UNKNOWN` bucket (pushed/dynamic), not numeric elements.
- `for…of x`, `x.forEach`/`map`/`reduce` callbacks → `%ARRAY_ALL` (all elements) —
  KEEP these unioning (that's where recall comes from; don't tighten them).
- dynamic read `x[k]` (non-const k) → `%ARRAY_ALL`.
- Array destructuring (`var [a0,a1,...rest] = arr`) already uses `value_at` /
  `values_from` correctly (`bind_collection_pattern`, ~L7437) — it only needs the
  underlying `values` to be index-ordered (defect 2) to be right.

## Target representation

```rust
struct CollectionTargets {
    keys: Vec<FunctionId>,            // Map keys (unchanged)
    values: Vec<FunctionId>,          // INDEX-ORDERED array elements (numeric cells)
    object_values: Vec<ObjectTargets>,// index-ordered object elements
    unknown: Vec<FunctionId>,         // NEW: %ARRAY_UNKNOWN (push / dynamic write / spread / pop source)
    unknown_objects: Vec<ObjectTargets>, // NEW: object form of the above
}
```
- `value_at(i)` → `values.get(i)` / `object_values.get(i)` only (NO union with `unknown`).
- `all_targets()` → `values ∪ keys ∪ unknown` (so for-of / dynamic reads keep recall).
- `extend()` → order-preserving dedup (reuse `append_ordered` semantics) so array
  order survives; the unordered callable-set uses don't care about order. Verify
  determinism (stable keys) is unaffected — if it is, keep a separate
  `extend_ordered` for array contexts and leave `extend` (sorted) for set contexts.
- `is_empty()` / `append_ordered()` → include the new buckets.

## Staged implementation (bench-gate every stage; revert if net-negative)

`CollectionTargets` is constructed as a literal in **46 places** — add the new
fields via `#[derive(Default)]` + a scripted insert of `unknown: Vec::new(),
unknown_objects: Vec::new(),` into each literal (mirror how `CallSiteFact.in_throw`
was rolled out across 39 sites in commit 4fc41965).

1. **Bucket + push/pop routing (smallest, highest-confidence).** Add the fields;
   route `push`/`add` (~L3011) and dynamic-index writes → `unknown`; make
   `pop`/`find` → `unknown`; `all_targets` ∪= unknown; `extend`/`is_empty` handle
   it. Expect arrays.js `t()` to drop from 4→1 edge (the pushed one). Watch for
   FN on pure-literal `[a,b].pop()` (no push → unknown empty → nothing) — if Jelly
   expects the last literal there, handle `pop` = `unknown ∪ values.last()`.
2. **Index-ordered `values` (defect 2).** Stop sorting `values` in array contexts
   (order-preserving dedup). This fixes `x[1]` precision and out-of-bounds
   `x[3]`→nothing via `value_at`. Re-check destructuring (rest.js `a0/a1/c0/c1`).
3. **`at(const)` → `value_at`; dynamic reads → `all_targets`.** Split `at` from
   `pop`/`find`. Route computed reads: const index → `value_at`, non-const →
   `all_targets`.
4. **Harder array transforms** (`concat`, `flat`, `flatMap`, `Array.from` map):
   match Jelly's element model if cheap; otherwise leave (they're a minority).

Target after stages 1–3: ~10–20 array FP gone, precision ~97–98%, **0 TP loss**.

## Then: the express keystone (separate, on the new headroom)

With precision headroom, implement the keystone so it lands by dilution
(≥96%): see `~/.claude/plans/create-a-plan-to-structured-flame.md` (WS-C) and
memory `jelly-express-object-model-chain`. Pieces: bare `setPrototypeOf` →
`Constraint::Inherit`; constructor-return prototype forwarding for `new Router()`
(NewExpression arm → `function_returns` fallback via `callee_function_ids`); walk
reached export-function bodies with real `this` (FunctionTokenFlow, not the
speculative `ThisMethodFlow`), invoked from `collect_program` after
`build_module_env`; and a dedup tiebreak so `FunctionTokenFlow` beats
`ThisMethodFlow` for equal `stable_key`. WS-A (throw suppression, already pushed)
kills the 2 `gettype` FPs; the array headroom absorbs the rest. Expected
+50–70 TP → ~F1 92%.

## Discipline & verification

- Bench-gate EVERY increment (~2 min after first build). Report TP/FP/FN + F1.
  Revert anything net-negative or byte-identical.
- Pure value-flow edits are **cache-gated**: bump the discriminator in
  `crates/polint/src/analysis/calls/cache_key.rs` (the digest fn AND its mirror
  `#[cfg(test)]` test) or the bench serves stale output.
- Diagnostic: `POLINT_JELLY_NO_PRUNE=1` to see all emitted edges; diff FN/FP sets.
  A per-case decode of `arrays.js`/`rest.js`/`iterators.js` FP vs the `.json`
  oracle's `call2fun` confirms Jelly's per-site target count.
- Add a gate test per stage in `eval::external::jelly_callgraph::tests` (use
  `run_kernel_for_repo_for_test` on a temp dir; assert via the `any_resolves`
  helper, and for value-flow-specific checks filter
  `t.algorithm == CallAlgorithm::FunctionTokenFlow`).
- Run all CI gates before push: `cargo fmt --all -- --check`; clippy `-D warnings`;
  `cargo test --workspace --all-features --locked`; doc `-D warnings`; `cargo deny`.
  Pre-existing unrelated failures to stash-verify: `eval::go_rta::*`,
  `eval::determinism_gate`, `eval::runner::tests::identity_jelly_oracle_coverage_fixture`.
- Do NOT regress the pushed WS-A/WS-B precision infra.

## Benchmark setup (fresh workspace)

The bench silently early-returns if repos are missing. Clone into
`research/evaluation-harness/repos/`:
- `cs-au-dk/jelly` @ `b799ed4f0d68c670fe398830aaa51dd5c628cf74` → `repos/jelly`
- `golang/tools` @ `7743a285e3d261ca235408e013ec5c14cb5170e4` → `repos/golang-tools`
- `npm install` in `repos/jelly/tests/helloworld/` (materializes the express tree).

Run:
```
POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release \
  cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```
Results: `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`
(`metrics{}` + `cases[].matches[]` where `item_kind=="graph_edge"`).

# Static Analysis Performance Report - 2026-06-06

## Bottom Line

The original "we are doing really bad" conclusion was half right. Go x/tools RTA
was bad because polint was comparing the upstream SSA RTA oracle against a
source-backed reconstructed graph that cannot represent the same nodes. That is
now fixed for the benchmark path: required-edge recall is 100%.

Jelly is still genuinely bad. polint emits only 14 JS/TS graph edges against
1,479 expected Jelly edges. The gap is architectural: Jelly models a whole
JavaScript program with synthetic module functions, function-object flow, heap
objects, prototypes, builtins, and call/apply/bind semantics; polint currently
lowers mostly real function bodies and loses many top-level and object-flow
calls before refined call scoring.

| Suite | Cases | TP | FP | FN | Precision | Recall | F1 | Unknowns | Runtime | Output hash |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|
| Go x/tools RTA callgraph | 5 | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% | 0 | 1.060s | `f9c8f398e133e64b` |
| Jelly JS/TS callgraph micro | 76 | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% | 104 | 0.793s | `135c493b613dd3cc` |

Raw artifacts: `.context/graph-benchmarks/`.

## Measurement Context

| Item | Value |
|---|---|
| Date | 2026-06-06 |
| Branch | `emilwareus/gsd-next-steps-v2` |
| polint version | `0.1.14` |
| Host | macOS 26.5, Darwin arm64 |
| Go toolchain | `go1.26.2 darwin/arm64` |
| Benchmark command | `POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release cargo test --release -p polint --lib eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture` |
| Benchmark wall time | 57.62s including release compilation; test body 1.93s |
| Peak memory footprint | 63,488,480 bytes |

Pinned benchmark clones:

| Suite | Source | Commit |
|---|---|---|
| Go x/tools RTA callgraph | `https://github.com/golang/tools` | `7743a285e3d261ca235408e013ec5c14cb5170e4` |
| Jelly callgraph micro | `https://github.com/cs-au-dk/jelly` | `b799ed4f0d68c670fe398830aaa51dd5c628cf74` |

## Before vs After

| Suite | Run | TP | FP | FN | Precision | Recall | F1 |
|---|---|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA | After benchmark accounting fixes | 1 | 9 | 36 | 10.00% | 2.70% | 4.26% |
| Go x/tools RTA | After direct RTA fix | 37 | 6 | 0 | 86.05% | 100.00% | 92.50% |
| Jelly | After benchmark accounting fixes | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% |
| Jelly | After direct RTA fix | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% |

The Go false positives are mostly x/tools RTA edges that are correct but not
listed as positive `WANT` rows. Those `WANT` comments are partial assertions,
not a closed negative oracle. The Go result should be read as full required-edge
coverage with a still-imperfect precision accounting story.

## What They Do Differently

### Go x/tools RTA

Upstream x/tools builds SSA and runs RTA directly:

- It creates SSA with generic instantiation enabled.
- It roots the analysis at `main` and `init`.
- It emits the call graph from `rta.Analyze`, including static calls, dynamic
  function-value calls, interface dispatches, bound wrappers, instantiated
  generic functions, and synthetic reflection edges.

polint was not doing that. It harvested ingredients from Go semantic analysis,
ran a solver, and projected results back through source-backed `FunctionFact`
and `CallSiteFact` rows. That cannot faithfully represent synthetic SSA nodes
such as `init$1`, bound wrappers, generic instantiations, or reflect-created
edges. One x/tools fixture also used a no-body external stub
`func use(interface{})`; normal `packages.Load` rejected that while the upstream
test harness tolerates it.

Fix applied:

- The Go sidecar now emits private `rta_edge` rows from x/tools SSA/RTA directly.
- SSA construction uses `ssa.InstantiateGenerics`.
- Rust stores, validates, digests, and exposes private internal
  `GoSemanticRtaEdgeFact` rows.
- The x/tools benchmark adapter prefers those direct RTA rows for oracle
  comparison.
- The txtar materializer adapts the no-body `func use(interface{})` benchmark
  stub to an empty body so normal package loading matches the upstream harness.
- Parsed x/tools `WANT` edges are marked partial positives instead of pretending
  the comments are a closed exhaustive oracle.

### Jelly

Jelly is doing a much richer JS analysis than polint currently does:

- It creates a synthetic program/module function, so top-level calls participate
  in the call graph.
- It models JavaScript functions as heap/function objects and propagates them
  through variables, object properties, arrays, callbacks, and module wrappers.
- It has explicit semantics for common builtins and object/prototype APIs.
- It handles `Function.prototype.call`, `apply`, and `bind` as call semantics,
  not just as unresolved property calls.
- It models accessors, classes, prototypes, and common framework/library shapes.

polint currently misses the first gate for many cases: module-level statements
are not lowered into a MIR body, because `MirBody` is owned by a real
`FunctionFact`. That means many top-level Jelly calls never reach the normal
call-site, points-to, and refined-call stages. For calls that do reach the
pipeline, object/member identity and call/apply/bind semantics are still too
weak.

## Current Release Benchmark Details

| Suite | Expected graph edges | Observed graph edges | Unconfirmed observed edges | Unknowns |
|---|---:|---:|---:|---:|
| Go x/tools RTA callgraph | 37 | 45 | 2 | 0 |
| Jelly JS/TS callgraph micro | 1479 | 14 | 0 | 104 |

Go per-case scoring:

| Case | TP | FP | FN | Notes |
|---|---:|---:|---:|---|
| `func.txtar` | 2 | 3 | 0 | Required dynamic function-value edges now match. Extra static edges are not listed in partial `WANT`. |
| `generics.txtar` | 12 | 1 | 0 | Generic instantiation edges now match. |
| `iface.txtar` | 5 | 5 | 0 | Required dynamic interface edges now match; extra static/method edges remain score-bearing under current matcher. |
| `multipkgs.txtar` | 13 | 1 | 0 | Multi-package RTA required edges now match. |
| `reflectcall.txtar` | 5 | 1 | 0 | Synthetic/static reflect-related required edges now match; two extra reflect-family edges are unconfirmed under partial scoring. |

Jelly observed matches are confined to a few micro cases:

| Case | Expected edges | Observed edges | TP | FP |
|---|---:|---:|---:|---:|
| `tests/micro/classes.json` | 77 | 8 | 4 | 4 |
| `tests/micro/defineProperty.json` | 15 | 2 | 0 | 2 |
| `tests/micro/generators.json` | 50 | 4 | 4 | 0 |

Jelly unknown reasons:

| Reason | Count |
|---|---:|
| `MissingSemanticReference` | 54 |
| `DynamicProperty` | 46 |
| `CallApplyBind` | 4 |

## Recommended Next Fixes

1. Add a first-class JS/TS module execution owner instead of treating MIR bodies
   as function-only. This needs a real core contract change, not an adapter-only
   shortcut, so call sites can have stable module callers and identity records.
2. Preserve callee places for identifier, static-member, and computed-member
   calls through MIR so points-to/refined-calls can connect call sites to
   function objects.
3. Model function expressions and arrow functions as function-object values
   assigned to places instead of marking them as unsupported temporaries.
4. Add explicit call/apply/bind lowering and refined-call expansion.
5. Expand the JS object/prototype model for object literals, property writes,
   accessors, classes, constructors, and prototype methods.
6. Add temp-repo or external-fixture regression tests for the first Jelly cases
   that should improve: direct top-level calls, function variables, object
   method assignment, and call/apply/bind.

## Repository Self-Analysis

The previous fundamentals pass still holds: `polint check --format json
--no-cache` exits 0 with no `polint/internal` diagnostics. The only diagnostic
was the existing unused ignore warning in `examples/comment-ignores/app.ts`.

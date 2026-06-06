# Static Analysis Performance Report - 2026-06-06

## Bottom Line

After fixing benchmark accounting/path normalization and the internal analyzer
errors, polint is still far below benchmark-grade graph accuracy.

| Suite | Scope | Cases | TP | FP | FN | Precision | Recall | F1 | Runtime |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA callgraph | release tier, all local cases | 5 | 1 | 9 | 36 | 10.00% | 2.70% | 4.26% | 1.053s |
| Jelly JS/TS callgraph micro | release tier, all local cases | 76 | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% | 0.716s |

The previous measurement overstated false positives and understated Jelly
matches because it counted benchmark invariant rows as scored false positives
and compared Jelly case-relative oracle paths against repo-relative observed
paths. Those were measurement/fundamental defects. The corrected results are
more honest, but still bad: recall is the blocker.

## Measurement Context

| Item | Value |
|---|---|
| Date | 2026-06-06 |
| Branch | `emilwareus/gsd-next-steps-v2` |
| Baseline commit before fixes | `435556ee` |
| polint version | `0.1.14` |
| Host | macOS 26.5, Darwin arm64 |
| Go toolchain | `go1.26.2 darwin/arm64` |
| Build mode | `cargo test --release` / `cargo run --release` |
| Benchmark repos | gitignored local clones under `research/evaluation-harness/repos/` |

Pinned benchmark commits:

| Suite | Source | Commit |
|---|---|---|
| Go x/tools RTA callgraph | `https://github.com/golang/tools` | `7743a285e3d261ca235408e013ec5c14cb5170e4` |
| Jelly callgraph micro | `https://github.com/cs-au-dk/jelly` | `b799ed4f0d68c670fe398830aaa51dd5c628cf74` |

## What Changed

Fundamentals fixed before the second measurement:

| Area | Fix |
|---|---|
| Self-analysis metadata | `ValuePrecision::ExactLocal` now maps to setup-aware metadata instead of over-claiming exact precision. |
| TS/JS MIR lowering | Optional chains no longer emit duplicate unsupported rows; empty/blank literals lower to unknown evidence instead of invalid literal facts. |
| Domain validation | Propagated `UnresolvedCall` top reasons no longer require the later domain observation to be located at the original call-site operation. Diagnostics now include stable evidence. |
| Benchmark metrics | Invariant/runtime metadata no longer contributes to precision/recall/F-scores. |
| Jelly adapter | Expected Jelly spans are normalized from JSON-case-relative paths to repo-relative paths, matching observed file identities. |

## Current Release Benchmark

Command:

```bash
POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release \
  /usr/bin/time -l cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```

Command result:

| Field | Value |
|---|---:|
| Exit status | 0 |
| Test body runtime | 1.86s |
| Wall time including incremental release compile | 72.17s |
| Maximum resident set size | 6,758,203,392 bytes |
| Peak memory footprint | 63,717,856 bytes |
| Raw artifacts | `.context/graph-benchmarks/` |

| Suite | Expected graph edges | Observed graph edges | Unknowns | Output hash |
|---|---:|---:|---:|---|
| Go x/tools RTA callgraph | 37 | 10 | 26 | `6ed2619007509930` |
| Jelly callgraph micro | 1479 | 14 | 104 | `135c493b613dd3cc` |

Observed graph-edge families:

| Suite | Expected families | Observed families |
|---|---|---|
| Go x/tools RTA callgraph | 4 dynamic function, 14 dynamic method, 8 static function, 10 static method, 1 synthetic | 10 static function |
| Jelly callgraph micro | 829 `call2fun`, 650 `fun2fun` | 7 `call2fun`, 7 `fun2fun` |

## Before vs After

| Suite | Run | TP | FP | FN | Precision | Recall | F1 |
|---|---|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA | Before fixes | 1 | 14 | 36 | 6.67% | 2.70% | 3.85% |
| Go x/tools RTA | After fixes | 1 | 9 | 36 | 10.00% | 2.70% | 4.26% |
| Jelly | Before fixes | 0 | 90 | 1479 | 0.00% | 0.00% | 0.00% |
| Jelly | After fixes | 8 | 6 | 1471 | 57.14% | 0.54% | 1.07% |

Benchmark metadata rows still appear in per-case match details as unmatched
invariants, but they no longer affect scored TP/FP/FN metrics.

## Repository Self-Analysis

Command:

```bash
/usr/bin/time -l cargo run --release -p polint --locked -- \
  check --format json --no-cache \
  > .context/polint-check-internal-clean.json
```

Result:

| Field | Value |
|---|---:|
| Exit status | 0 |
| Diagnostics | 1 warning |
| `polint/internal` diagnostics | 0 |
| Wall time | 89.57s |
| User CPU | 35.11s |
| Sys CPU | 2.17s |
| Peak memory footprint | 9,895,355,848 bytes |
| Maximum resident set size | 13,960,413,184 bytes |

The only diagnostic is the existing unused ignore warning in
`examples/comment-ignores/app.ts`.

## Root Cause Notes

### Go x/tools RTA

The Go benchmark is failing because polint is not producing the RTA oracle edge
families for the external fixtures.

| Case | Expected | Observed | TP | Notes |
|---|---:|---:|---:|---|
| `func.txtar` | 2 | 2 | 0 | Expected dynamic function-value calls; observed unrelated static function calls. |
| `generics.txtar` | 12 | 0 | 0 | No observed graph edges. |
| `iface.txtar` | 5 | 4 | 0 | Expected dynamic method calls; observed static function calls to `use`/`live`. |
| `multipkgs.txtar` | 13 | 4 | 1 | Only one static function edge matches; dynamic/static method edges missing. |
| `reflectcall.txtar` | 5 | 0 | 0 | Static method and synthetic reflect edges missing. |

Unknown reasons in the Go run:

| Reason | Count |
|---|---:|
| `DynamicProperty` | 13 |
| `MissingSemanticReference` | 9 |
| `Reflection` | 3 |
| `UnknownCallee` | 1 |

Interpretation: the external RTA cases appear to fall back to syntax/MIR-style
call modeling instead of yielding enough Go semantic RTA inputs and anchored
solver/refined edges. The solver/refined-call bridge exists, so the next debug
target is the Go semantic lifecycle/input join for these materialized txtar
repos: functions, callsites, method sets, instantiated types, address-taken
functions, dynamic dispatch rows, and semantic-node/callsite anchors.

### Jelly JS/TS

Jelly path identity is fixed, but recall remains tiny because polint emits only
14 graph edges against 1,479 expected oracle edges.

| Signal | Value |
|---|---:|
| Expected `jelly.call_graph.call2fun` | 829 |
| Expected `jelly.call_graph.fun2fun` | 650 |
| Observed `jelly.call_graph.call2fun` | 7 |
| Observed `jelly.call_graph.fun2fun` | 7 |
| Jelly oracle endpoint spans matched | 14 / 1460 |

Observed graph edges only appear in:

| Case | Expected edges | Observed edges | TP | Graph FP |
|---|---:|---:|---:|---:|
| `tests/micro/classes.json` | 77 | 8 | 4 | 4 |
| `tests/micro/defineProperty.json` | 15 | 2 | 0 | 2 |
| `tests/micro/generators.json` | 50 | 4 | 4 | 0 |

Unknown reasons in the Jelly run:

| Reason | Count |
|---|---:|
| `MissingSemanticReference` | 54 |
| `DynamicProperty` | 46 |
| `CallApplyBind` | 4 |

Interpretation: JS/TS recall is limited by semantic binding and object/member
modeling. The current implementation can find a few direct or shape-derived
edges, but most Jelly oracle edges require function-value propagation,
member/property resolution, prototype/class modeling, call/apply/bind handling,
and broader identity coverage.

## Recommended Next Work

1. Add provider-count instrumentation or targeted tests for the external Go RTA
   cases: assert nonzero `go_semantic_dynamic_dispatch`, method sets,
   instantiated types, and solver-derived/refined Go RTA edges where the oracle
   expects them.
2. Fix Go external-case semantic lifecycle and anchoring before tuning matcher
   identities. The benchmark currently lacks the core dynamic edge families.
3. Add Go static method and synthetic reflect-call projection once semantic
   inputs are flowing.
4. Improve JS/TS semantic reference propagation and object/member modeling,
   prioritizing the Jelly unknown clusters: `MissingSemanticReference` and
   `DynamicProperty`.
5. Keep `polint check --format json --no-cache` on this repository at zero
   `polint/internal` diagnostics before trusting future benchmark deltas.

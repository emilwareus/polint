# Static Analysis Performance Report - 2026-06-06

## Bottom Line

The current static-analysis engine does **not** meet the v1.3 benchmark target.
On the full locally available external graph suites, polint baseline performance
is:

| Suite | Scope | Cases | TP | FP | FN | Precision | Recall | F1 | Runtime |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA callgraph | release tier, all local cases | 5 | 1 | 14 | 36 | 6.67% | 2.70% | 3.85% | 1.124s |
| Jelly JS/TS callgraph micro | release tier, all local cases | 76 | 0 | 90 | 1479 | 0.00% | 0.00% | 0.00% | 0.877s |

This means the promotion infrastructure exists, but the analyzer output is not
yet benchmark-grade. The earlier `<3% -> >25-30%` recall goal is **not achieved**
by the measured implementation.

## Measurement Context

| Item | Value |
|---|---|
| Date | 2026-06-06 |
| Branch | `emilwareus/gsd-next-steps-v2` |
| Commit measured | `b707ff12` plus the test-only benchmark tier selector added in this task |
| polint version | `0.1.14` |
| Host | macOS 26.5, Darwin arm64 |
| Go toolchain | `go1.26.2 darwin/arm64` |
| Build mode | `cargo test --release` / `target/release/polint` |
| Benchmark repos | gitignored local clones under `research/evaluation-harness/repos/` |

Pinned benchmark commits:

| Suite | Source | Commit |
|---|---|---|
| Go x/tools RTA callgraph | `https://github.com/golang/tools` | `7743a285e3d261ca235408e013ec5c14cb5170e4` |
| Jelly callgraph micro | `https://github.com/cs-au-dk/jelly` | `b799ed4f0d68c670fe398830aaa51dd5c628cf74` |

## External Graph Benchmark Results

### Release Tier

Command:

```bash
POLINT_WRITE_GRAPH_BENCH=1 POLINT_GRAPH_BENCH_TIER=release \
  /usr/bin/time -l cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```

Command result:

- Exit status: 0
- Test body runtime: 2.09s
- Command wall time including incremental release compile: 65.00s
- Time output peak memory footprint: 64,012,768 bytes
- Generated raw artifacts: `.context/graph-benchmarks/`

| Suite | Expected graph edges | Observed graph edges | Unknowns | Output hash |
|---|---:|---:|---:|---|
| Go x/tools RTA callgraph | 37 | 10 | 26 | `350b040700d09f0d` |
| Jelly callgraph micro | 1479 | 14 | 104 | `44681262ca6a1cc3` |

Observed graph-edge families:

| Suite | Observed families | Status / precision |
|---|---|---|
| Go x/tools RTA callgraph | 10 `go.rta.call_graph.static_function_call` edges | all `resolved`, all `Heuristic` |
| Jelly callgraph micro | 7 `jelly.call_graph.fun2fun`, 7 `jelly.call_graph.call2fun` | all `resolved`, all `Heuristic` |

Worst false-negative clusters:

| Suite | Case | TP | FP | FN | Runtime |
|---|---|---:|---:|---:|---:|
| Go | `go/callgraph/rta/testdata/multipkgs.txtar` | 1 | 4 | 12 | 179ms |
| Go | `go/callgraph/rta/testdata/generics.txtar` | 0 | 1 | 12 | 204ms |
| Go | `go/callgraph/rta/testdata/iface.txtar` | 0 | 5 | 5 | 208ms |
| Jelly | `tests/helloworld/app.json` | 0 | 1 | 342 | 5ms |
| Jelly | `tests/micro/classes.json` | 0 | 9 | 77 | 27ms |
| Jelly | `tests/micro/classes2.json` | 0 | 1 | 76 | 41ms |

### Fast Tier Smoke Run

Command:

```bash
POLINT_WRITE_GRAPH_BENCH=1 \
  /usr/bin/time -l cargo test --release -p polint --lib \
  eval::external::tests::external_graph_baseline_reports_can_be_generated \
  --locked -- --nocapture
```

Command result:

- Exit status: 0
- Test body runtime: 1.26s
- Command wall time including release compile: 80.56s
- Generated raw artifacts: `.context/graph-benchmarks-fast/`

| Suite | Scope | Cases | TP | FP | FN | Precision | Recall | F1 | Runtime |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Go x/tools RTA callgraph | fast tier; all 5 local cases selected | 5 | 1 | 14 | 36 | 6.67% | 2.70% | 3.85% | 0.991s |
| Jelly callgraph micro | fast tier deterministic subset | 20 | 0 | 30 | 315 | 0.00% | 0.00% | 0.00% | 0.213s |

## Repository Scan Runtime

These runs measure the current release CLI against this workspace. They do not
produce precision/recall, but they show throughput and internal diagnostic
health on a real repository.

Workspace source-file count, excluding `target/` and benchmark clones:

| Extension | Count |
|---|---:|
| `.rs` | 387 |
| `.ts` | 57 |
| `.go` | 54 |
| `.js` | 10 |
| `.tsx` | 8 |
| Total | 516 |

Commands:

```bash
/usr/bin/time -l target/release/polint check --format json --no-cache \
  > .context/polint-check-no-cache-seq.json \
  2> .context/polint-check-no-cache-seq.time.txt

/usr/bin/time -l target/release/polint check --format json \
  > .context/polint-check-cache-seq.json \
  2> .context/polint-check-cache-seq.time.txt
```

| Run | Exit | Wall time | User CPU | Sys CPU | Peak memory footprint | Diagnostics |
|---|---:|---:|---:|---:|---:|---:|
| No cache | 1 | 35.60s | 33.35s | 1.84s | 9,568,086,312 bytes | 14 |
| Cache enabled | 1 | 35.76s | 33.39s | 1.77s | 9,505,007,912 bytes | 17 |

Diagnostic summary:

| Run | Errors | Warnings | Main finding |
|---|---:|---:|---|
| No cache | 13 | 1 | Fact metadata precision-ceiling/stable-key conflicts plus one unused ignore |
| Cache enabled | 13 | 4 | Same internal errors plus 3 `internal/cache` warnings |

Cache did not materially improve the full-workspace scan in this run. The
cache-enabled run was slightly slower and produced additional cache warnings.

## Interpretation

- **Accuracy is the blocker.** Runtime for the external graph benchmark is low
  enough for iteration, but precision/recall are far below the promotion target.
- **Go RTA is not scoring as intended yet.** It emits a small number of heuristic
  graph edges, but only one matches the x/tools oracle.
- **JS/TS callgraph scoring is effectively failing.** Jelly reports zero true
  positives across all 76 local cases.
- **The repository scan is not healthy.** Running `polint check` on this repo
  exits non-zero because internal metadata validation emits 13 errors.
- **The promotion gate should currently fail on real measurements.** The gate
  infrastructure works, but the measured analyzer should not be promoted as
  benchmark-grade.

## Reproduction Notes

The graph benchmark writer is currently an internal test helper, not a public
CLI. This task added a test-only selector:

```bash
POLINT_GRAPH_BENCH_TIER=fast    # default
POLINT_GRAPH_BENCH_TIER=nightly
POLINT_GRAPH_BENCH_TIER=release
```

Raw generated files are intentionally under `.context/` and are not committed:

- `.context/graph-benchmarks/summary.md`
- `.context/graph-benchmarks/go-x-tools-rta-callgraph-baseline.json`
- `.context/graph-benchmarks/jelly-callgraph-micro-baseline.json`
- `.context/graph-benchmarks-fast/summary.md`
- `.context/polint-check-no-cache-seq.json`
- `.context/polint-check-cache-seq.json`

## Recommended Next Work

1. Fix `polint check` internal metadata errors first. A benchmark run is hard to
   trust while the analyzer fails on its own repository.
2. Debug identity normalization for callgraph matching. The engine emits
   heuristic graph edges, but they mostly do not match Go/Jelly oracle identities.
3. Add a supported `polint benchmark` or `polint eval` CLI surface for benchmark
   execution. Today the only runnable path is a `#[cfg(test)]` helper.
4. Promote release-tier external graph benchmark execution into CI only after
   precision and recall are above the gate floors.
5. Re-run this report after fixes and compare against this baseline:
   Go 6.67% precision / 2.70% recall; Jelly 0.00% precision / 0.00% recall.

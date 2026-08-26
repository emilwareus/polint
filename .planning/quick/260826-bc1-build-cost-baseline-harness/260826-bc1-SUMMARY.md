---
quick_id: 260826-bc1
task: build-cost-baseline-harness
status: completed
---

# Summary

Added `polint-bench build-cost`, a measurement harness for what a repo-local
rule host costs to build today, plus `make build-cost` / `make
build-cost-baseline` and a measured baseline at
`research/evaluation-harness/baselines/build-cost.json`
(`schema_version = polint-build-cost-1`).

No product behaviour changed and no file under `crates/polint/src` was touched.

## How it measures

- **Cargo invocations** — `POLINT_CARGO` already selects the Cargo program the
  CLI spawns, so it points at the `polint-bench` binary. In shim mode the binary
  records the invocation and runs the real Cargo with the same argument vector.
- **Compiled units** — the Cargo shim installs itself as `RUSTC_WRAPPER`. One
  `rustc` invocation carrying `--crate-name` and no `--print`/`-vV` query is one
  compiled unit, observed rather than inferred from the dependency graph.
- **Rule-host wall-clock and peak RSS** — read from the sidecar the rule host
  already writes under `POLINT_GOLDEN_COST_PATH` (`golden_cost.rs` ->
  `measure::TimedRun`). Nothing new was instrumented in the engine.
- **Bytes** — the rule-host `CARGO_TARGET_DIR`, the polint cache, and
  `CARGO_HOME/registry` are walked either side of the measured command.
  Hard-linked content counts once, because Cargo links a built binary into both
  `deps/` and the profile root.
- **Isolation** — each cell copies the repository under test into
  `target/polint-build-cost/`, rewrites the pack manifest into the standalone
  shape a consumer gets, and points the cache and target directories at scratch,
  so no checked-in example is edited. A cell deletes its scratch tree once its
  readings are taken, keeping peak disk at one cell rather than the matrix.

## What the baseline replaced

The research report carried three unverified figures. Measured on the recorded
machine, a cold `examples/basic` scan compiles **225 units** in **one** Cargo
invocation and retains **582.7 MB** in the rule-host target directory. A
`warm-noop` re-run still starts Cargo once and compiles nothing; a
`warm-source-edit` does the same; a one-line rule edit compiles exactly one unit;
`polint test` starts Cargo once per fixture case.

## Limits recorded, not estimated

`compiler_peak_rss_bytes` is `null` — Cargo and `rustc` memory is not observed.
Wall-clock was taken on a shared, contended host and moves by multiples with
load; invocation, unit, and byte counts do not. Every limit is repeated in the
artifact's own `limits` array and in `research/evaluation-harness/README.md`.

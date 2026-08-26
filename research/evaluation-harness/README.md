# Evaluation Harness Research

Date: 2026-05-26

This folder defines how polint evaluates graph/fact quality, scanner accuracy,
runtime, cache behavior, and repo-local adaptation.

## Current Benchmark Scope

polint currently supports only:

- Go
- TypeScript / JavaScript

Current scored benchmark work must stay inside those languages. Unsupported
language suites are not part of promotion gates, comparison tables, baseline
tables, or adapted-run tables.

## Supported External Suites

| Suite | Language | Purpose |
|---|---|---|
| Go x/tools RTA callgraph | Go | Primary Go graph benchmark for call-edge expectations from the official Go tools test corpus. |
| Jelly JS/TS callgraph micro | TypeScript / JavaScript | Primary JS/TS graph benchmark for suite-native call graph edge expectations. |
| SecBench.js smoke | TypeScript / JavaScript | Executable server-side JavaScript security benchmark smoke coverage. |
| gosec samples | Go | Practical Go security sample coverage and competitor comparison against gosec. |

Graph benchmarks are the main external benchmark track. Security suites remain
supported secondary benchmarks for vulnerability detection and adapted-rule
measurement. Native polint fixtures remain the first promotion gate before
external suites.

## Benchmark Table Contract

For each supported suite, reports should separate:

- other-product baseline rows, such as Semgrep, CodeQL, gosec, or suite-native references when reproducible;
- `polint_baseline`, with no repo adaptation;
- `polint_agent_adapted`, produced by a separate adaptation agent using a recorded prompt and budget.

Adapted runs must record prompt path/hash, allowed and forbidden inputs, changed
rule or extension artifacts, digests, accepted/rejected facts, case-level deltas,
runtime/cache overhead, and limitations.

## Rule-Host Build Cost

`polint check` in a repository that has `.polint/rules` hands the scan to a
subprocess Cargo builds first, so the cost a user waits for is a compile.
`polint-bench build-cost` measures that compile; `make build-cost` runs the
matrix and prints ratios against
`baselines/build-cost.json` (`schema_version = polint-build-cost-1`), and
`make build-cost-baseline BUILD_COST_LABEL=<machine>` rewrites it.

Each cell copies the repository under test into `target/polint-build-cost/`,
rewrites the rule pack into the standalone manifest a consumer gets, and points
`POLINT_CACHE_DIR` and `POLINT_RULES_TARGET_DIR` at scratch directories, so no
checked-in example is edited and no developer cache is disturbed. A cell deletes
its scratch tree once its readings are taken, which keeps peak disk at one cell
rather than the whole matrix; pass `--keep-scratch` to inspect one.

### Scenarios

| Scenario | State the measured command starts from |
|---|---|
| `cold` | no compiler output, no analysis cache |
| `warm-noop` | a completed check, nothing changed |
| `warm-rule-edit` | a completed check, one line appended to a rule source |
| `warm-source-edit` | a completed check, one byte appended to an analyzed source |
| `test-suite` | `polint test` over the fixture suite, from a fresh cache |

Warm scenarios run one unmeasured warm-up first; `cold` and `test-suite` delete
the cache and the rule-host target directory before every run.

### Metrics

| Key | Meaning |
|---|---|
| `wall_clock_ms` | the measured `polint` process, start to exit |
| `cargo_invocations` | Cargo processes `polint` started, counted by a `POLINT_CARGO` shim |
| `cargo_failed_invocations` | of those, the ones that exited non-zero |
| `cargo_wall_clock_ms` | total time inside those Cargo processes |
| `rustc_invocations` | `rustc` processes Cargo started, counted through `RUSTC_WRAPPER` |
| `compiled_units` | those that compiled a crate rather than answering a `--print`/`-vV` probe |
| `rules_target_bytes_before` / `_after` | rule-host `CARGO_TARGET_DIR` size either side of the run |
| `rules_target_bytes_written` | bytes in files that directory gained a modification time for during the run |
| `rules_target_files_after` | files retained there |
| `polint_cache_bytes_before` / `_after` / `_written` | same three readings for the polint cache |
| `cargo_registry_bytes_before` / `_after` | `CARGO_HOME/registry` either side of the run |
| `rule_host_wall_clock_ms` | the rule-host process itself, from its `POLINT_GOLDEN_COST_PATH` sidecar |
| `rule_host_peak_rss_bytes` / `_delta_bytes` | that process's peak RSS and the growth it caused |
| `compiler_peak_rss_bytes` | always `null`; see limits |
| `rule_tests_passed` / `_failed` / `_total` | `polint test` tally, `test-suite` only |

Byte totals sum regular-file sizes and count hard-linked content once, because
Cargo links a built binary into both `deps/` and the profile root.

### Limits

Every report repeats its own limits in a `limits` array. They are:

- `compiler_peak_rss_bytes` is never measured. Peak RSS comes from the sidecar
  the rule host already writes, and the harness adds no process instrumentation,
  so Cargo and `rustc` memory is not observed.
- `rule_host_*` describes the last rule-host process a run started. `test-suite`
  starts one per fixture case and reports only the last.
- Rule packs resolve `polint` through a path dependency on the checkout, so
  `cargo_registry_*` excludes polint itself; a cold cell downloads only the
  third-party closure, not the two-sided download a crates.io consumer pays.
- `RUSTC_WRAPPER` is installed to count compiled units and participates in Cargo
  fingerprints, so numbers are not comparable to runs taken without the harness.
  Every cell primes its own state for that reason.
- No example repository ships `.polint/tests` fixtures, so `test-suite` generates
  them (`test_cases_generated: true`). A generated case asserts nothing, so its
  pass/fail tally carries no signal; the case count and the Cargo invocations it
  causes do.
- Wall-clock is the load-sensitive metric. Cargo invocation counts, compiled-unit
  counts, and byte totals repeat exactly across runs on the same checkout;
  `wall_clock_ms` and `cargo_wall_clock_ms` move by multiples on a shared or
  memory-constrained host. Compare timings only against a baseline taken on the
  same machine under the same conditions, and re-measure locally before drawing
  a conclusion from a ratio.
- A committed baseline records the one machine it was taken on, named by
  `environment.label`. Machines that were not available are absent rather than
  estimated, and a cell that could not run is `status: "failed"` carrying the
  reason, never a filled-in number.

## Folder Structure

| Path | Purpose |
|---|---|
| `FINAL-REPORT.md` | Supported-scope benchmark recommendation. |
| `RECOMMENDED_IMPLEMENTATION.md` | Concrete implementation path for the internal harness. |
| `RESEARCH-ANALYSIS.md` | Supported-suite tradeoffs and accuracy caveats. |
| `STANDARD.md` | Vocabulary and manifest schema for supported-suite adapters. |
| `REPO-INDEX.md` | Supported benchmark repositories cloned and inspected. |
| `PAPER-INDEX.md` | Supported benchmark papers and sources. |
| `VALIDATION.md` | What was validated and remaining supported-scope risks. |
| `algorithms/` | Scoring, matching, scheduling, baselines, and adaptation deltas. |
| `benchmarks/` | Go and TS/JS benchmark map. |
| `implementation/` | Internal architecture and phased implementation notes. |
| `oss/` | Supported external benchmark comparison. |
| `decisions/` | Decision log. |
| `papers/` | Downloaded supported benchmark PDFs. |
| `repos/` | Local clones of supported benchmark repositories. This directory is gitignored. |

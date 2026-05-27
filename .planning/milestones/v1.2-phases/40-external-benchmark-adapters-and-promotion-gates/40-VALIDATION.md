---
phase: 40
slug: external-benchmark-adapters-and-promotion-gates
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-05-26
validated: 2026-05-26
---

# Phase 40 - Validation Strategy

Phase 40 is validated for the implemented scope: internal benchmark manifests,
comparison/adaptation records, deterministic eval reports, promotion gates,
supported-language external adapters, real Go/TS graph benchmark baseline
reports, and public-boundary proof.

The higher-accuracy graph engine is intentionally out of Phase 40 scope and is
tracked as the next milestone in GitHub issue #49.

## Test Infrastructure

| Property | Value |
|----------|-------|
| Framework | Rust `cargo test`, `cargo clippy`, focused unit/integration-style fixture tests |
| Config file | `Cargo.toml`, `Cargo.lock`, `research/evaluation-harness/suites/*.toml`, `tests/eval-fixtures/**/expected.polint-eval.toml` |
| Quick run command | `cargo test -p polint --lib eval --locked` |
| Full phase command | `cargo fmt --all --check && cargo clippy -p polint --all-targets --locked -- -D warnings && cargo test -p polint --lib eval --locked` |
| Graph report command | `POLINT_WRITE_GRAPH_BENCH=1 cargo test -p polint eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture` |
| Estimated runtime | Focused eval suite ~214s on this machine; graph report test ~1s after build |

## Sampling Rate

- After eval schema/report changes: run the relevant focused `eval::*` module tests.
- After graph benchmark adapter changes: run `cargo test -p polint --lib eval --locked`
  and the `POLINT_WRITE_GRAPH_BENCH=1` report generation command.
- Before PR closeout: run formatting, clippy, public-boundary checks, and graph
  report generation.
- Max practical feedback latency: under 5 minutes for the focused validation set.

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|----------|-----------|-------------------|-------------|--------|
| 40-01-01 | 01 | 1 | SAE-PROM-01 | T40-01 | Suite manifests validate tier/support/checkout metadata safely. | unit | `cargo test -p polint --lib eval::suite --locked` | yes | GREEN |
| 40-01-02 | 01 | 1 | SAE-PROM-01 | T40-01 | Report rows distinguish imported scanners, reproduced scanners, polint baseline, polint adapted, and adapter-only results. | unit | `cargo test -p polint --lib eval::report --locked` | yes | GREEN |
| 40-01-03 | 01 | 1 | SAE-PROM-01 | T40-02 | Adapted runs require prompt path/hash, budget, allowed inputs, forbidden inputs, and changed artifact evidence. | unit | `cargo test -p polint --lib eval::adaptation --locked` | yes | GREEN |
| 40-02-01 | 02 | 2 | SAE-PROM-01 | - | Unsupported-language benchmark scope removed; only Go and TS/JS remain active. | manifest/docs | `cargo test -p polint committed_evaluation_suite_manifests_parse_and_validate --locked` | yes | GREEN |
| 40-03-01 | 03 | 3 | SAE-PROM-01 | T40-06 | Runtime/cache/provider rows serialize while volatile runtime is excluded from output hashes. | unit | `cargo test -p polint --lib eval::performance --locked` | yes | GREEN |
| 40-03-02 | 03 | 3 | SAE-PROM-01 | T40-07 | Metric sections include scanner, graph, path, unknown, performance, suite-native, and adaptation slots. | unit | `cargo test -p polint --lib eval::metrics --locked` | yes | GREEN |
| 40-03-03 | 03 | 3 | SAE-PROM-01 | T40-07 | Markdown summaries are deterministic derived output from eval JSON. | unit | `cargo test -p polint --lib eval::markdown --locked` | yes | GREEN |
| 40-04-01 | 04 | 4 | SAE-PROM-01 | T40-08 | Promotion gates report pass/warn/fail with metric and threshold names. | unit | `cargo test -p polint --lib eval::gates --locked` | yes | GREEN |
| 40-04-02 | 04 | 4 | SAE-PROM-01 | T40-09 | Native promotion fixture covers graph, facts, paths, unknowns, budgets, and cache determinism. | fixture | `cargo test -p polint --lib eval_observed --locked` | yes | GREEN |
| 40-04-03 | 04 | 4 | SAE-PROM-01 | T40-08 | Partial-truth graph/path extras become unconfirmed unless forbidden by truth. | unit | `cargo test -p polint --lib eval::matcher --locked` | yes | GREEN |
| 40-05-01 | 05 | 5 | SAE-PROM-01 | T40-10 | Tier runner selects cases deterministically and rejects unsafe case ids. | unit | `cargo test -p polint --lib eval::tiers --locked` | yes | GREEN |
| 40-05-02 | 05 | 5 | SAE-PROM-01 | T40-11 | SecBench.js and gosec supported-language smoke adapters enumerate local clones and skip absent clones as limitations. | unit | `cargo test -p polint --lib eval::external --locked` | yes | GREEN |
| 40-06-01 | 06 | 6 | SAE-PROM-01 | T40-12/T40-13 | Adaptation prompt forbids expected-label access and benchmark case hardcoding. | text assertion | `rg -n "Do not read benchmark expected labels|Do not hardcode benchmark case IDs|Use the polint skill" research/evaluation-harness/prompts/default-adaptation-agent.md` | yes | GREEN |
| 40-06-02 | 06 | 6 | SAE-PROM-01 | T40-12 | Default-vs-adapted deltas name changed cases, unknown changes, graph/path changes, and rejected extension facts. | unit/fixture | `cargo test -p polint --lib eval::delta --locked` | yes | GREEN |
| 40-07-01 | 07 | 7 | SAE-PROM-01 | T40-14 | Competitor records require citation or local reproduction metadata and sort deterministically. | unit | `cargo test -p polint --lib eval::competitors --locked` | yes | GREEN |
| 40-07-02 | 07 | 7 | SAE-PROM-01 | T40-15 | Baseline comparisons reject adapter-only rows and gate precision/recall/runtime/cache regressions. | unit | `cargo test -p polint --lib eval::baseline --locked` | yes | GREEN |
| 40-08-01 | 08 | 8 | SAE-PROM-01 | T40-16 | Internal eval helper writes deterministic JSON/Markdown without public CLI exposure. | unit/boundary | `cargo test -p polint --lib eval::runner --locked` | yes | GREEN |
| 40-08-02 | 08 | 8 | SAE-PROM-01 | T40-16 | Public CLI help, SDK, runner, README, and docs do not expose hidden eval or unpromoted graph/data-flow/evidence views. | boundary | `cargo run -q -p polint -- --help > /tmp/polint-help.txt && ! rg -n "\\beval\\b" /tmp/polint-help.txt` | yes | GREEN |
| 40-GRAPH-01 | quick extension | post-40 | SAE-PROM-01 | T40-15 | Go x/tools and Jelly graph suites generate real baseline JSON/Markdown reports with TP/FP/FN, precision, recall, unknowns, runtime, and output hash. | integration-style unit | `POLINT_WRITE_GRAPH_BENCH=1 cargo test -p polint eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture` | yes | GREEN |
| 40-GRAPH-02 | quick extension | post-40 | SAE-PROM-01 | T40-15 | Direct/refined call and CFG regression checks remain green after graph benchmark normalization changes. | regression | `cargo test -p polint direct_calls --locked -- --nocapture`; `cargo test -p polint refined_calls --locked -- --nocapture`; `cargo test -p polint cfg_core --locked -- --nocapture` | yes | GREEN |

## Current Validation Run

Commands run on 2026-05-26:

- `cargo fmt --all --check` - passed.
- `cargo test -p polint --lib eval --locked` - passed, 206 tests.
- `POLINT_WRITE_GRAPH_BENCH=1 cargo test -p polint eval::external::tests::external_graph_baseline_reports_can_be_generated --locked -- --nocapture` - passed.
- `cargo test -p polint direct_calls --locked -- --nocapture` - passed, 17 tests plus `direct_calls_internals_stay_private`.
- `cargo test -p polint refined_calls --locked -- --nocapture` - passed, 31 tests.
- `cargo test -p polint cfg_core --locked -- --nocapture` - passed, 3 tests.
- `cargo clippy -p polint --all-targets --locked -- -D warnings` - passed.
- `cargo run -q -p polint -- --help > /tmp/polint-help.txt && ! rg -n "\\beval\\b" /tmp/polint-help.txt` - passed.
- `rg -n "polint eval|CallGraph<'_|DataFlow<'_|Evidence<'_" README.md docs/facts crates/polint/src/sdk crates/polint/src/runner || true` - found only `docs/facts/data-flow.md`, which explicitly says `DataFlow<'_>` is reserved and unsupported.

Generated graph benchmark summary:

| Suite | Mode | TP | FP | FN | Precision | Recall | Unknowns | Runtime ms | Output hash |
|---|---|---:|---:|---:|---:|---:|---:|---:|---|
| go-x-tools-rta-callgraph | PolintBaseline | 1 | 9 | 36 | 0.1000 | 0.0270 | 26 | 309 | `80d0165b07a079fc` |
| jelly-callgraph-micro | PolintBaseline | 2 | 6 | 313 | 0.2500 | 0.0063 | 28 | 668 | `27214e12046c2f18` |

The runtime values changed from prior runs, but output hashes remained stable.

## Wave 0 Requirements

Existing Rust test infrastructure covers the phase. No Wave 0 test scaffolding
is missing.

## Manual-Only Or Deferred Verifications

| Behavior | Requirement | Why Manual/Deferred | Validation |
|----------|-------------|---------------------|------------|
| Higher-recall graph engine for Go RTA and Jelly call graph oracles | Next milestone, not Phase 40 | Requires scanner-core work: stable semantic graph identities, reachability, Go semantic/SSA/RTA, JS/TS binding/token/property solvers | Deferred to GitHub issue #49 and `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` |
| `polint_agent_adapted` graph benchmark rows | Next milestone after core solver/model layer | Adaptation needs validated model facts consumed by a capable graph solver; Phase 40 only adds prompt/artifact contracts and real baseline rows | Deferred to GitHub issue #49 |

## Validation Sign-Off

- [x] All implemented Phase 40 tasks have automated verification.
- [x] No unsupported-language benchmark manifests remain in active scope.
- [x] Real Go and TS/JS graph benchmark baseline reports are generated from local external suites.
- [x] Public CLI/SDK/runner/docs boundary remains closed for hidden eval internals.
- [x] Known benchmark accuracy limitations are documented and routed to the next milestone.
- [x] `nyquist_compliant: true` set in frontmatter for implemented Phase 40 scope.

Approval: validated 2026-05-26.

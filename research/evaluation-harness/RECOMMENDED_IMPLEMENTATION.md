# Recommended Implementation

Date: 2026-05-26

## Current Boundary

Build and run benchmark support only for Go and TypeScript / JavaScript until
polint supports additional language frontends.

## Implementation Path

1. Keep evaluation internals crate-private.
   - `crates/polint/src/eval` remains internal.
   - No public `polint eval` command is advertised.

2. Keep graph benchmarks as the main supported external adapters.
   - `eval::external::go_x_tools_callgraph` for Go call-edge expectations.
   - `eval::external::jelly_callgraph` for TS/JS call-edge expectations.

3. Keep vulnerability benchmarks as secondary supported external adapters.
   - `eval::external::secbench_js` for TS/JS security cases.
   - `eval::external::gosec` for Go security cases.

4. Keep supported manifests only.
   - `research/evaluation-harness/suites/go-x-tools-rta-callgraph.toml`
   - `research/evaluation-harness/suites/jelly-callgraph-micro.toml`
   - `research/evaluation-harness/suites/secbench-js-smoke.toml`
   - `research/evaluation-harness/suites/gosec-samples.toml`

5. Preserve native promotion fixtures.
   - Native fixtures prove engine behavior that external scanner suites cannot:
     CFG, calls, summaries, data flow, evidence, cache, deterministic reports,
     extension acceptance/rejection, and adaptation deltas.

6. Produce benchmark tables with three lanes.
   - Other-product baseline, sourced from published or locally reproduced results.
   - polint baseline, with no repo-specific adaptation.
   - polint adapted, produced by a separate adaptation agent.

7. Require adaptation provenance.
   - Record prompt path/hash, budget, allowed inputs, forbidden inputs, changed
     rules/extensions, digests, accepted/rejected facts, case deltas, runtime,
     cache overhead, and limitations.

## Supported-Suite Runner Behavior

- Missing local clones should produce honest skipped/limitation output, not
  synthetic scores.
- Fast tiers should run before nightly/release tiers.
- Release-tier claims should pin suite commit, polint commit, tool versions, and
  output hash.

## Explicit Non-Goals

- Do not keep active manifests for unsupported language suites.
- Do not keep adapter code whose only current purpose is unsupported-language
  scoring.
- Do not include unsupported-language rows in current scorecards, baselines, or
  promotion gates.

# Recommended Implementation

Date: 2026-05-26

## Current Boundary

Build and run benchmark support only for Go and TypeScript / JavaScript until
polint supports additional language frontends.

## Implementation Path

1. Keep evaluation internals crate-private.
   - `crates/polint/src/eval` remains internal.
   - No public `polint eval` command is advertised.

2. Keep the supported external adapters.
   - `eval::external::secbench_js` for TS/JS.
   - `eval::external::gosec` for Go.

3. Keep supported manifests only.
   - `research/evaluation-harness/suites/secbench-js-smoke.toml`
   - `research/evaluation-harness/suites/gosec-samples.toml`

4. Preserve native promotion fixtures.
   - Native fixtures prove engine behavior that external scanner suites cannot:
     CFG, calls, summaries, data flow, evidence, cache, deterministic reports,
     extension acceptance/rejection, and adaptation deltas.

5. Produce benchmark tables with three lanes.
   - Other-product baseline, sourced from published or locally reproduced results.
   - polint baseline, with no repo-specific adaptation.
   - polint adapted, produced by a separate adaptation agent.

6. Require adaptation provenance.
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

# Internal Eval Path

Date: 2026-05-26

## Boundary

The eval path remains internal and currently supports benchmark execution only
for Go and TypeScript / JavaScript suites.

## Current Manifests

```text
research/evaluation-harness/suites/
  secbench-js-smoke.toml
  gosec-samples.toml
```

## Intended Operational Flow

1. Ensure the supported external repositories are cloned at pinned commits under
   `research/evaluation-harness/repos/`.
2. Run native promotion fixtures.
3. Run supported external fast tiers.
4. Reproduce or import other-product baselines for the same supported suite.
5. Run polint baseline with no repo adaptation.
6. Run the adaptation agent with `prompts/default-adaptation-agent.md`.
7. Generate comparison and adaptation-delta reports.

## Example Internal Inputs

```text
suite: research/evaluation-harness/suites/secbench-js-smoke.toml
tier: fast
mode: polint_baseline
```

```text
suite: research/evaluation-harness/suites/gosec-samples.toml
tier: fast
mode: polint_agent_adapted
```

## Public API Policy

Do not document a public eval CLI until the schema, report format, and supported
suite lifecycle are stable. The hidden/test-only helper can keep producing JSON
and Markdown reports for verification.

## Exclusions

Unsupported-language suite manifests, adapters, and scorecards are deliberately
absent. Adding a new language frontend should include a separate benchmark phase
that introduces that language's external suites and promotion gates.

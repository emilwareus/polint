# Supported OSS Benchmark Comparison

Date: 2026-05-26

This comparison is restricted to languages polint supports today: Go and
TypeScript / JavaScript.

## Priority Ranking

| Rank | Suite | Language | Priority | Why |
|---:|---|---|---|---|
| 1 | SecBench.js | TypeScript / JavaScript | High | Executable server-side JS package vulnerability benchmark with published research framing. |
| 2 | gosec samples | Go | High | Practical Go security samples and direct gosec competitor baseline. |
| 3 | Native polint fixtures | Go, TypeScript / JavaScript | Required | Measures graph/fact/data-flow/evidence/cache/adaptation properties external suites do not cover. |
| 4 | CodeQL Go/JS tests | Go, TypeScript / JavaScript | Reference | Useful microcase taxonomy; expected outputs reflect CodeQL modeling. |
| 5 | Jelly | TypeScript / JavaScript | Reference | Useful call graph evaluation ideas. |
| 6 | BugsJS | JavaScript | Reference | Project-scale regression inputs, not scanner ground truth. |

## Current Implementation Order

1. Native polint promotion fixtures.
2. SecBench.js smoke adapter and manifest.
3. gosec samples adapter and manifest.
4. Competitor rows for Semgrep/CodeQL/gosec where results are published or
   locally reproduced against the same supported suite.
5. Agent-adapted rows for the same supported suite, with prompt and artifact
   provenance recorded.

## Exclusions

Suites for unsupported languages are excluded from current scorecards, gates,
and adaptation prompts. They can be reconsidered only after the corresponding
language frontend exists.

# Supported-Scope Benchmark Analysis

Date: 2026-05-26

## Scope

This analysis covers current polint language support only:

- Go
- TypeScript / JavaScript

Benchmark work outside those languages is out of current scope.

## What External Suites Can Measure

| Suite | Language | Useful Signal | Limit |
|---|---|---|---|
| SecBench.js | TypeScript / JavaScript | Executable package vulnerability cases and JS scanner comparison. | Security-focused; does not measure repo-local engineering policies. |
| gosec samples | Go | Practical Go security examples and gosec comparison. | Tool-owned samples, not broad independent ground truth. |
| CodeQL Go/JS tests | Go, TypeScript / JavaScript | Microcase taxonomy and expected-output inspiration. | CodeQL modeling choices are not polint truth. |
| Jelly | TypeScript / JavaScript | Static/dynamic call graph comparison ideas. | Research/reference, not a scanner scorecard. |
| BugsJS | JavaScript | Project-scale regression inputs. | Bug corpus, not direct static-analysis ground truth. |

## What Native Fixtures Must Measure

External scanner suites do not prove the internal engine properties polint needs
for repo-local static analysis. Native fixtures must cover:

- deterministic scheduling and report hashing;
- provenance, precision, confidence, and validation metadata;
- cache keys and cache reuse;
- CFG, call, summary, data-flow, and evidence facts;
- unknown/setup-missing accounting;
- extension acceptance/rejection;
- default-vs-adapted deltas.

## Adaptation Measurement

The central product question is not only "what does polint detect by default?"
It is also "what does polint detect after a repo-local agent writes rules and
extensions for this codebase?"

For each supported suite, benchmark tables should include:

- other-product baseline;
- polint baseline;
- polint agent-adapted.

The adaptation agent must not receive expected labels, answer keys, benchmark
case IDs, generated filenames, or suite-specific path patterns as detection
logic. The report must record the exact prompt and artifacts so reviewers can
distinguish real adaptation from benchmark gaming.

## Go Caveat

Go has less broad public scanner benchmark coverage than TS/JS. Treat gosec as
a practical supported-suite baseline, then lean on native fixtures for engine
claims and on competitor rows only when results are comparable and reproduced or
properly sourced.

## TS/JS Caveat

SecBench.js is the strongest current external signal, but it is security-focused.
Repo-local policy rules, framework lifecycle facts, and adaptation deltas still
need native fixtures and realistic TS/JS target repos.

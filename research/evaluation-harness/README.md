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

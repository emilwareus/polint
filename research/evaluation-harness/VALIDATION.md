# Validation Notes

Date: 2026-05-26

## Validated Supported-Scope Facts

- SecBench.js is the current external TypeScript / JavaScript benchmark target.
- gosec samples are the current external Go benchmark target.
- Suite manifests exist only for supported-language suites:
  - `research/evaluation-harness/suites/secbench-js-smoke.toml`
  - `research/evaluation-harness/suites/gosec-samples.toml`
- Cloned repositories, when present, live under `research/evaluation-harness/repos/`
  and are gitignored.

## Known Limits

- SecBench.js is a security benchmark, not a complete TS/JS policy benchmark.
- gosec samples are useful practical Go cases, but they are not broad independent
  ground truth.
- CodeQL, Semgrep, gosec, and suite-native expected outputs reflect those tools'
  modeling choices. Use them as comparison rows or microcase inspiration, not as
  unquestioned correctness definitions.
- Native polint fixtures remain required for CFG, call facts, data flow,
  evidence quality, cache reuse, and adaptation-delta promotion gates.

## License And Source Policy

Before committing extracted fixtures, expected outputs, or copied source snippets,
check each suite's license. Prefer adapter manifests that reference external
checkouts over copying benchmark content.

## Next Validation Steps

- Clone or refresh SecBench.js and gosec at the pinned commits when running full
  external benchmarks.
- Run supported-suite smoke tiers before nightly/release tiers.
- Reproduce or import competitor rows only for the same supported suites.
- Run the adaptation agent only against Go or TS/JS suites.

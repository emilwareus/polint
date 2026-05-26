# Evaluation Harness Standard

Date: 2026-05-26

This standard describes how polint represents supported benchmark suites, native
fixtures, expected outputs, observed outputs, scanner comparisons, and adaptation
deltas.

Current scored benchmark scope is limited to Go and TypeScript / JavaScript.

## Vocabulary

| Term | Meaning |
|---|---|
| Suite | A supported benchmark source such as SecBench.js, gosec samples, or native polint fixtures. |
| Case | One independently scored target: a file, package, repo, or test case. |
| Expected item | Ground truth or target fact/diagnostic/path for the case. |
| Observed item | What polint or another scanner produced. |
| Match | The relation between expected and observed item: TP, FP, FN, TN, unknown, or unconfirmed. |
| Suite-native metric | The benchmark's own scoring model, preserved for comparability. |
| Unified metric | polint's shared precision/recall/F-score/unknown/runtime/cache metric vocabulary. |
| Adapted run | A polint run after a separate agent writes repo-local rules/models/extensions for the target codebase. |

## Manifest Shape

Example supported external suite:

```toml
schema_version = "polint-eval-suite-1"
id = "secbench-js-smoke"
name = "SecBench.js smoke"
kind = "scanner_vulnerability"
languages = ["javascript", "typescript"]
adapter_id = "secbench_js_smoke"
source_url = "https://github.com/SecBench/SecBench.js"
source_commit = "bc3156219138"
license = "license-review-needed"
language_support = "supported"

[checkout]
strategy = "local_clone"
path = "research/evaluation-harness/repos/SecBench.js"
ignored_by_git = true

[expected]
format = "suite_native"
path = "suite-native-secbench-js"

[scoring]
native = ["secbench_js.smoke_case_count"]
unified = ["precision", "recall", "f1", "f2", "f3", "false_positive_rate"]

[tiers.fast]
enabled = true
selector = "sample:balanced:20"
max_cases = 20
deterministic_seed = "secbench-js-fast"
```

## Required Report Columns

Each supported scanner benchmark table should include:

- other-product baseline rows where available;
- polint baseline with no repo adaptation;
- polint adapted to the target codebase.

Rows must include source provenance, suite commit, tool version when reproduced,
limitations, runtime/cache data when available, and deterministic report hashes.

## Adaptation Rules

Adaptation agents may inspect the target repository and use the polint skill.
They must not receive expected labels, answer keys, suite-specific case IDs, or
generated benchmark filenames as rule logic. Adapted reports must include prompt
path/hash, budget, changed artifacts, digests, accepted/rejected facts, and
case-level deltas.

## Scope Rule

Do not create scored manifests or gates for languages outside Go and TS/JS until
polint implements the corresponding language frontend.

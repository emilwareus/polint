# Evaluation Harness Standard

Date: 2026-05-26

This standard describes how polint represents supported benchmark suites, native
fixtures, expected outputs, observed outputs, scanner comparisons, and adaptation
deltas.

Current scored benchmark scope is limited to Go and TypeScript / JavaScript.

## Vocabulary

| Term | Meaning |
|---|---|
| Suite | A supported benchmark source such as Go x/tools callgraph fixtures, Jelly callgraph fixtures, SecBench.js, gosec samples, or native polint fixtures. |
| Case | One independently scored target: a file, package, repo, or test case. |
| Expected item | Ground truth or target fact/diagnostic/path for the case. |
| Observed item | What polint or another scanner produced. |
| Match | The relation between expected and observed item: TP, FP, FN, TN, unknown, or unconfirmed. |
| Suite-native metric | The benchmark's own scoring model, preserved for comparability. |
| Unified metric | polint's shared precision/recall/F-score/unknown/runtime/cache metric vocabulary. |
| Adapted run | A polint run after a separate agent writes repo-local rules/models/extensions for the target codebase. |

## Manifest Shape

Example supported external graph suite:

```toml
schema_version = "polint-eval-suite-1"
id = "jelly-callgraph-micro"
name = "Jelly JS/TS callgraph micro"
kind = "call_graph_precision"
languages = ["javascript", "typescript"]
adapter_id = "jelly_callgraph_micro"
source_url = "https://github.com/cs-au-dk/jelly"
source_commit = "b799ed4f0d68c670fe398830aaa51dd5c628cf74"
license = "BSD-3-Clause"
language_support = "supported"

[checkout]
strategy = "local_clone"
path = "research/evaluation-harness/repos/jelly"
ignored_by_git = true

[expected]
format = "suite_native"
path = "suite-native-jelly-callgraph"

[scoring]
native = ["jelly.callgraph_micro.case_count", "jelly.callgraph_micro.expected_edge_count"]
unified = ["precision", "recall", "f1", "f2", "f3", "false_positive_rate"]

[tiers.fast]
enabled = true
selector = "sample:balanced:20"
max_cases = 20
deterministic_seed = "jelly-callgraph-fast"
```

## Required Report Columns

Each supported graph benchmark table should include:

- suite-native oracle row where available, such as Jelly JSON or Go x/tools
  `WANT` expectations;
- `polint_baseline`, produced by the built-in graph/fact providers with no
  repo adaptation;
- `polint_agent_adapted`, produced after a separate agent adds repo-local
  graph models, call-resolution hints, summaries, or extensions;
- other-product baseline rows where reproducible, such as Jelly or Go
  `x/tools` output.

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

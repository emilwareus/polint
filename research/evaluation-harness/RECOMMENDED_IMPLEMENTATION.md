# Recommended Implementation: Evaluation Harness

## Goal

Create an internal evaluation harness that makes polint's analysis quality measurable across languages, analysis families, and agent-authored extensions.

The harness should support:

- external benchmark adapters;
- native engine fixtures;
- default-vs-extension comparisons;
- scanner metrics;
- fact/graph/path metrics;
- runtime and memory baselines;
- cache invalidation metrics;
- stable JSON and Markdown reports.

Keep the first version hidden/internal. Do not expose a public CLI contract until the model survives several benchmark adapters.

## Target Architecture

```text
polint eval --suite <suite> --mode default
polint eval --suite <suite> --mode extended --extension <crate>

suite manifest
  -> suite adapter loads cases
  -> runner creates temp/pinned target workspace
  -> polint analysis runs
  -> observed outputs normalized
  -> matcher compares expected vs observed
  -> metrics computed
  -> report written
```

Internal modules:

```text
crates/polint/src/eval/
  mod.rs
  manifest.rs
  suite.rs
  adapter.rs
  runner.rs
  observed.rs
  expected.rs
  matcher.rs
  metrics.rs
  report.rs
  baseline.rs
  compare.rs
  fixtures.rs
```

Keep this `pub(crate)` and hide any CLI flag behind an internal/unstable command until ready.

## Stage 1: Canonical Evaluation Model

Implement:

```rust
pub(crate) struct EvaluationSuite;
pub(crate) struct BenchmarkCase;
pub(crate) enum ExpectedItem;
pub(crate) enum ObservedItem;
pub(crate) struct CaseResult;
pub(crate) struct EvaluationRun;
pub(crate) struct MetricSet;
```

The model should support these expected item kinds from day one:

- diagnostics;
- facts;
- graph edges;
- paths;
- invariants;
- runtime budgets.

Acceptance:

- JSON serialization is deterministic.
- Output hash excludes timestamps and machine-specific paths.
- Tests can construct expected/observed items without running full analysis.

## Stage 2: Matchers And Metrics

Implement generic matchers:

```text
diagnostic matcher
fact matcher
graph edge matcher
path matcher
invariant matcher
```

Implement generic metrics:

```text
TP, FP, FN, TN
precision
recall
F1/F2/F3
FPR
unknown counts
graph metrics
path metrics
performance metrics
extension delta
```

Acceptance:

- matcher tests cover exact, tolerant, partial, and forbidden cases;
- graph extras against partial truth become `UNCONFIRMED`;
- line tolerance is suite-configurable;
- false-positive traps are first-class expected items.

## Stage 3: Native Fixture Adapter

Build a tiny native fixture suite first because it exercises the harness without depending on third-party setup.

Fixture layout:

```text
tests/eval-fixtures/
  kernel/
    scheduling/
    provenance/
    cache-invalidation/
    extension-merge/
  facts/
    symbols/
    references/
    imports/
  graphs/
    call-edges/
    unknown-calls/
  data-flow/
    local-flow/
    sanitizer/
    missing-model/
```

Each fixture should include:

```text
repo/
expected.polint-eval.toml
README.md
```

Acceptance:

- one fixture validates provenance;
- one fixture validates an extension fact rejection;
- one fixture validates cache invalidation;
- one fixture validates default-vs-extension delta;
- one fixture validates deterministic output by running twice.

## Stage 4: OWASP Adapter

Implement the first external adapter for OWASP expected-results CSV.

Adapter input:

```text
expectedresults-*.csv
benchmark root
suite config
```

Adapter output:

```text
BenchmarkCase {
    case_id: BenchmarkTest00001,
    expected: ExpectedDiagnostic {
        category: pathtraver,
        cwe: CWE-22,
        vulnerable: true,
    }
}
```

Initial scope:

- parse expected results;
- run polint on benchmark source if language supported;
- otherwise allow dry-run adapter validation;
- score imported scanner JSON once polint emits compatible diagnostics.

Acceptance:

- parses BenchmarkJava and BenchmarkPython expected files;
- reproduces case counts;
- computes OWASP-native confusion-matrix metrics for synthetic observed outputs;
- writes suite-native and unified metrics.

## Stage 5: Provider Stats And Cache Metrics

Wire evaluation reports to the analysis kernel stats:

```rust
struct ProviderStats {
    provider_id: ProviderId,
    input_digest: Digest,
    output_digest: Digest,
    facts_emitted: usize,
    diagnostics_emitted: usize,
    validation_rejections: usize,
    wall_time_ms: u64,
    cpu_time_ms: Option<u64>,
    peak_rss_bytes: Option<u64>,
    cache_hit: bool,
}
```

Acceptance:

- report shows provider time by layer;
- report shows cache hits and misses by layer;
- repeated run can prove deterministic output and expected cache reuse.

## Stage 6: Default-Vs-Extension Delta

Evaluation must compare:

```text
default mode run
extended mode run
```

Delta output:

```json
{
  "default_score": { "recall": 0.51, "precision": 0.88 },
  "extended_score": { "recall": 0.74, "precision": 0.84 },
  "resolved_unknowns": 83,
  "new_false_positives": 4,
  "removed_false_negatives": 29,
  "accepted_extension_facts": 120,
  "rejected_extension_facts": 3,
  "runtime_overhead_ratio": 1.18
}
```

Acceptance:

- extension run cannot overwrite default results;
- report names each changed finding/fact/edge/path;
- rejected extension facts are visible;
- score improvement is separated from runtime/cost impact.

## Stage 7: Add High-Value External Adapters

Add adapters in this order:

1. **OWASP expected-results CSV**: easiest scoring shape and broad Java/Python scanner precedent.
2. **RealVuln JSON**: best real Python scanner benchmark and false-positive traps.
3. **gosec sample adapter**: necessary for Go until stronger external benchmarks exist.
4. **SecBench.js adapter**: strong JS/TS executable benchmark, but setup is heavier.
5. **Jelly dynamic call graph adapter**: graph comparison for JS/TS.
6. **CodeQL/Pyre/Pysa microcase adapters**: reference taxonomy and targeted analysis behavior.
7. **DroidBench/CryptoAPI-Bench/SecuriBench Micro**: deeper Java/Android/security family coverage.
8. **CrossCommitVuln/SecCodeBench**: advanced temporal and agentic workflows.

## Stage 8: Benchmark Tiers

Implement `--tier`:

```text
fast     -> local CI smoke
nightly  -> broad external suites
release  -> full benchmark pack
research -> expensive or experimental suites
```

Acceptance:

- each suite manifest declares tier membership;
- tier selection is deterministic;
- sampled subsets are stable by seed and suite version;
- release reports include all source commits and manifest digests.

## Stage 9: Baseline And Regression Gates

Add baseline comparison:

```text
polint eval --suite owasp-python --tier nightly --write-baseline
polint eval --suite owasp-python --tier nightly --compare-baseline baseline.json
```

Gate types:

- fail if recall drops more than threshold;
- fail if precision drops more than threshold;
- fail if new high-severity FP trap appears;
- fail if output hash is nondeterministic;
- fail if provider runtime regresses beyond threshold;
- fail if cache invalidation expands unexpectedly;
- fail if extension rejection count changes unexpectedly.

Keep thresholds suite-specific.

## Stage 10: Reports For Humans And Agents

Generate:

```text
eval-report.json
eval-summary.md
case-results.jsonl
provider-stats.json
delta-report.md
```

The Markdown report should be short and actionable:

- top regressions;
- biggest false-negative clusters;
- biggest false-positive clusters;
- unresolved facts by provider;
- extension facts rejected and why;
- provider runtime/memory hotspots;
- recommended next modeling tasks.

Agents should be able to read the report and decide what extension or rule to write next.

## Why This Should Come Before Call Graph/Data Flow Implementation

Without this harness, polint will have no objective way to decide:

- which call graph tier is worth its cost;
- whether a data-flow summary improved recall or just added noise;
- whether an extension model improved a real benchmark;
- whether a cache optimization broke correctness;
- whether a new provider changed output order nondeterministically.

Build the scoreboard first. Then every algorithm implementation has a feedback loop.

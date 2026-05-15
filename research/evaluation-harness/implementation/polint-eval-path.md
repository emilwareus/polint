# Implementation Path: `polint eval`

## Recommended Shape

Create the evaluation harness inside `crates/polint` first, hidden behind an internal command or feature flag.

Why inside `crates/polint` first:

- it needs direct access to internal analysis-kernel stats;
- it should not freeze a public API;
- it will evolve quickly while adapters are added;
- it can reuse existing config, runner, cache, and diagnostics structures.

Later, if it becomes large, split an internal `polint-eval` crate.

## Hidden Command Sketch

```text
polint eval \
  --suite research/evaluation-harness/suites/owasp-python.toml \
  --tier fast \
  --mode default \
  --format json \
  --out target/polint-eval/owasp-python-fast.json
```

Extended mode:

```text
polint eval \
  --suite suites/realvuln-python.toml \
  --tier nightly \
  --mode extended \
  --extension .polint/extensions/framework_model \
  --compare-default \
  --out target/polint-eval/realvuln-extended/
```

Do not document this publicly until the schema is stable.

## Minimal First Pull Request

Scope:

- `eval` internal module with data model;
- JSON report output;
- diagnostic matcher and metrics;
- native fixture adapter;
- OWASP expected-results parser;
- tests for metrics and matching;
- no public docs except internal research references.

The first PR can run without Java/Python language support by scoring synthetic observed diagnostics. The point is to prove adapter parsing and scoring before wiring every language.

## Second Pull Request

Scope:

- integrate analysis-kernel provider stats;
- record cache stats;
- run real polint analysis on Go/TS native fixtures;
- output default-vs-extension delta skeleton;
- deterministic output hash.

## Third Pull Request

Scope:

- add one real external benchmark suite that matches a supported language.
- for current polint, this likely means Go via gosec/CodeQL-inspired sample adapter or TS/JS via a small SecBench.js subset.
- keep OWASP adapters present but mark unsupported languages as adapter-only until Java/Python support exists.

## Adapter Interfaces

```rust
pub(crate) trait BenchmarkAdapter {
    fn id(&self) -> AdapterId;
    fn load_suite(&self, manifest: &SuiteManifest) -> Result<EvaluationSuite>;
    fn prepare_case(&self, case: &BenchmarkCase, workspace: &Workspace) -> Result<PreparedCase>;
    fn normalize_observed(&self, case: &BenchmarkCase, raw: RawPolintOutput) -> Result<Vec<ObservedItem>>;
    fn suite_native_metrics(&self, results: &[CaseResult]) -> Result<BTreeMap<String, f64>>;
}
```

Adapters should not own the analysis engine. They translate suite-specific structure into canonical expected/observed items.

## Suite Manifests

Keep suite manifests under research initially:

```text
research/evaluation-harness/suites/
  owasp-java-1.2.toml
  owasp-python-0.1.toml
  gosec-samples.toml
  secbench-js-smoke.toml
  native-kernel.toml
```

When stable, move product-supported manifests under `crates/polint/eval-suites/` or a separate package.

## CI Integration

Fast CI:

```text
cargo test eval::
polint eval --suite native-kernel --tier fast --determinism-check
polint eval --suite supported-language-smoke --tier fast
```

Nightly:

```text
polint eval --suite external-pack --tier nightly --compare-baseline
```

Release:

```text
polint eval --suite external-pack --tier release --write-report
```

## Report Contract

The JSON report should be the source of truth. Markdown is generated from JSON.

Keep JSON stable within a schema version:

```json
{
  "schema_version": "polint-eval-0",
  "run": {},
  "suite": {},
  "metrics": {},
  "cases": [],
  "provider_stats": [],
  "cache_stats": {},
  "extension_delta": null
}
```

## Integration With Research Roadmap

Build this before implementing full call graphs and data flow. Then every analysis implementation can answer:

- which suite did this improve?
- which suite got worse?
- did unknowns decrease?
- did false positives increase?
- what was the runtime cost?
- what cache layers did it invalidate?
- did agent extensions improve the default?

That is the discipline needed for "most capable" instead of "most complex."

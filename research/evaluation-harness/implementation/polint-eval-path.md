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

## Required Benchmark Comparison Shape

Phase 40 reports should make three different claims visibly separate:

| Suite | Other scanner/product | polint baseline | polint agent-adapted |
|---|---|---|---|
| `suite-id` | Published or locally reproduced results from Semgrep, CodeQL, gosec, suite-native reference tools, or other relevant scanners | polint analysis with no repo-local adaptation beyond normal config | polint after a dedicated adaptation agent writes repo-local rules, models, or provider extensions for that target |

The report must not collapse these into one score. The product thesis is that
polint should be useful by default and materially better after a code-aware
agent adapts it to the target repository. Both claims need separate evidence.

Other-product results can be imported from benchmark-published tables when local
reproduction is not practical, but the report must label the source, product
version, suite version, and whether the value was reproduced locally or copied
from a pinned/public result. Locally reproduced competitor runs should record the
command, config, version, and output artifact path.

`polint baseline` means no repo-specific generated rules or provider extensions
written for the benchmark target. Normal project config and built-in/native
analysis are allowed. `polint agent-adapted` means a separate agent was asked to
inspect the target repository and create repo-local polint rules, model facts, or
provider extensions before the benchmark is rerun.

## Agent Adaptation Protocol

Each adapted benchmark run must create an adaptation record:

```json
{
  "schema_version": "polint-eval-adaptation-0",
  "suite_id": "...",
  "case_selection": "...",
  "agent": {
    "kind": "subagent",
    "model": "...",
    "prompt_path": "...",
    "budget": {
      "wall_time_minutes": 60,
      "max_iterations": 5
    }
  },
  "inputs_allowed": [
    "target repository source",
    "polint docs and skill",
    "baseline polint output",
    "benchmark suite manifest without expected labels"
  ],
  "inputs_forbidden": [
    "expected labels before adaptation",
    "case IDs mapped to vulnerability truth",
    "hardcoded benchmark filenames or generated naming conventions as detection logic"
  ],
  "outputs": {
    "rules_or_extensions_changed": [],
    "rule_digests": [],
    "extension_digests": [],
    "notes_path": "..."
  }
}
```

The adaptation agent should have access to the polint skill
(`.claude/skills/polint/SKILL.md`) and the relevant extension/rule-authoring
research. It should not be asked to maximize the benchmark score directly.
It should be asked to model the target codebase honestly, with validation,
provenance, precision labels, and minimal benchmark-specific assumptions.

### Adaptation Prompt Template

Store the exact prompt text beside each adapted benchmark report. The default
prompt should be close to:

```text
You are adapting polint to one benchmark target repository.

Goal:
Improve polint's scanner accuracy on this target by writing repo-local polint
rules, model facts, or provider extensions that honestly describe this
repository's framework, lifecycle, APIs, sources, sinks, sanitizers, barriers,
entrypoints, summaries, dispatch behavior, and trust boundaries.

Allowed context:
- Read the target repository source, build/config files, tests, docs, and lockfiles.
- Use the polint skill at .claude/skills/polint/SKILL.md.
- Use polint's rule-authoring and extension documentation available in this repo.
- Run polint baseline output and inspect unknowns, unresolved calls, missing
  entrypoints, setup gaps, false-positive examples surfaced by baseline output,
  and unsupported framework behavior.
- Run tests and polint eval iterations within the declared budget.

Forbidden context and behavior:
- Do not read benchmark expected labels, answer keys, or vulnerability truth
  before writing the adaptation.
- Do not hardcode benchmark case IDs, generated filenames, expected-result CSV
  data, or suite naming conventions as detection logic.
- Do not add broad facts or rules only to increase recall if they create
  unjustified false positives.
- Do not bypass extension validation, precision labels, provenance, cache inputs,
  or public rule-authoring constraints.

Adaptation process:
1. Map the target's language, package layout, framework/lifecycle entrypoints,
   request/job/CLI boundaries, generated dispatch, and test fixtures.
2. Identify where baseline polint is uncertain: unresolved calls, unknown flows,
   missing summaries, missing framework entrypoints, missing source/sink/barrier
   models, unsupported lifecycle, or noisy evidence.
3. Choose the narrowest repo-local mechanism:
   - normal #[polint::rule] diagnostics for repository policy checks;
   - model/provider extension facts for analysis semantics such as entrypoints,
     source/sink/sanitizer/barrier models, call edges, summaries, or trust
     boundaries.
4. Implement the rule or extension using public/approved polint authoring
   surfaces; keep internals private.
5. Add small fixture tests or documented checks that prove the adaptation models
   real repository behavior rather than benchmark labels.
6. Run polint eval in baseline and adapted modes, inspect the delta, and iterate
   only to fix real modeling mistakes.
7. Produce an adaptation note listing explored files, assumptions, rules or
   extensions changed, validation results, unresolved limitations, and any
   precision/runtime/cache tradeoffs.

Deliverables:
- Repo-local rule/extension files.
- Adaptation note.
- Commands run.
- Any fixture or smoke tests added.
- Final default-vs-adapted eval report path.
```

The evaluation report should embed or link this prompt, not just the resulting
score. If a future agent uses a modified prompt, that modified prompt is part of
the benchmark artifact and must be included in the report.

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

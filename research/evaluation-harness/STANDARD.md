# Standard: Evaluation Harness Vocabulary And Schemas

This standard gives polint one way to describe external benchmarks, native fixtures, expected facts, diagnostics, graph edges, data-flow paths, runtime metrics, and extension deltas.

The goal is not to freeze a public format. Keep this internal until enough adapters prove it.

## Core Terms

| Term | Meaning |
|---|---|
| Suite | A benchmark source such as OWASP Benchmark Java, RealVuln, SecBench.js, or a native fixture pack. |
| Case | One independently scored benchmark target. Could be one file, one test case, one package, one repo, or one CVE. |
| Adapter | Rust code that converts suite-specific inputs and expected outputs into polint's canonical evaluation model. |
| Expected item | A canonical expected diagnostic, fact, edge, path, metric, or invariant. |
| Observed item | What polint emitted during a run. |
| Match | A relationship between expected and observed items, possibly exact, tolerant, partial, or rejected. |
| Suite-native metric | The benchmark's own scoring model, preserved for comparability. |
| Unified metric | Polint's cross-suite scoring model. |
| Default mode | Native polint analysis without repo-local analysis extensions. |
| Extension mode | Same target with selected agent/user-authored Rust extensions enabled. |
| Delta | The normalized difference between default and extension mode. |

## Suite Manifest

```toml
[suite]
id = "owasp-benchmark-java-1.2"
name = "OWASP Benchmark Java"
kind = "scanner_vulnerability"
languages = ["java"]
adapter = "owasp_expected_csv"
source_url = "https://github.com/OWASP-Benchmark/BenchmarkJava"
source_commit = "61b831658171"
license = "unknown"

[suite.checkout]
strategy = "local_clone"
path = "research/evaluation-harness/repos/BenchmarkJava"
ignored_by_git = true

[suite.expected]
format = "owasp_expectedresults_csv"
path = "expectedresults-1.2.csv"

[suite.scoring]
native = ["owasp_confusion_matrix", "owasp_score"]
unified = ["precision", "recall", "f1", "f2", "f3", "fpr"]

[suite.tiers]
fast = { enabled = true, selector = "sample:balanced:50" }
nightly = { enabled = true, selector = "all" }
release = { enabled = true, selector = "all" }

[suite.cost]
expected_runtime = "medium"
requires_network = false
requires_docker = false
```

## Case Model

```rust
struct BenchmarkCase {
    suite_id: SuiteId,
    case_id: CaseId,
    language: Language,
    root: CaseRoot,
    target_files: Vec<PathBuf>,
    tags: BTreeSet<Tag>,
    expected: Vec<ExpectedItem>,
    setup: CaseSetup,
    budgets: CaseBudgets,
}
```

`CaseRoot` may point to:

- a single file;
- a package directory;
- a checked-out repo;
- a generated temp repo;
- a commit pair or commit chain;
- a container image input.

## Expected Items

```rust
enum ExpectedItem {
    Diagnostic(ExpectedDiagnostic),
    Fact(ExpectedFact),
    GraphEdge(ExpectedGraphEdge),
    Path(ExpectedPath),
    Invariant(ExpectedInvariant),
    RuntimeBudget(ExpectedRuntimeBudget),
}
```

### Expected Diagnostic

```rust
struct ExpectedDiagnostic {
    id: ExpectedId,
    kind: DiagnosticKind,
    cwe: Option<CweId>,
    severity: Option<Severity>,
    file: PathMatcher,
    line: LineMatcher,
    function: Option<SymbolMatcher>,
    sink: Option<SymbolMatcher>,
    source: Option<SymbolMatcher>,
    vulnerable: bool,
    acceptable_kinds: Vec<DiagnosticKind>,
    acceptable_cwes: Vec<CweId>,
    evidence: EvidenceExpectation,
}
```

Line matching should be tolerant by default for external security benchmarks. Exact lines are brittle across parser versions, formatting, generated code, and diagnostic span choices.

### Expected Fact

```rust
struct ExpectedFact {
    id: ExpectedId,
    family: FactFamily,
    stable_key: Option<StableKey>,
    selector: FactSelector,
    required_precision: PrecisionFloor,
    allowed_provenance: ProvenancePolicy,
    expected_value: FactValueMatcher,
}
```

Native polint fixtures should prefer stable keys. External suites often require structural selectors because they do not label facts directly.

### Expected Graph Edge

```rust
struct ExpectedGraphEdge {
    id: ExpectedId,
    graph: GraphKind,
    caller: NodeMatcher,
    callee: NodeMatcher,
    call_site: Option<SpanMatcher>,
    truth_kind: TruthKind,
    precision: PrecisionExpectation,
}
```

`TruthKind` matters:

```rust
enum TruthKind {
    MustExist,
    MustNotExist,
    DynamicObserved,
    StaticExpected,
    PartialGroundTruth,
}
```

Dynamic call graph traces should normally be treated as `DynamicObserved`, not complete truth.

### Expected Path

```rust
struct ExpectedPath {
    id: ExpectedId,
    path_kind: PathKind,
    source: NodeMatcher,
    sink: NodeMatcher,
    required_intermediate: Vec<NodeMatcher>,
    forbidden_intermediate: Vec<NodeMatcher>,
    max_explanation_steps: Option<usize>,
    evidence_required: EvidenceRequirement,
}
```

For security data flow, endpoint correctness is more important than exact internal path equality. A path can be semantically correct even if the analyzer explains it with a different but equivalent intermediate sequence.

## Observed Run Model

```rust
struct EvaluationRun {
    run_id: RunId,
    polint_commit: String,
    suite_id: SuiteId,
    suite_commit: Option<String>,
    mode: EvaluationMode,
    started_at: Timestamp,
    config_digest: Digest,
    rule_digest: Digest,
    extension_digests: Vec<Digest>,
    cases: Vec<CaseResult>,
    metrics: MetricSet,
    provider_stats: Vec<ProviderStats>,
    cache_stats: CacheStats,
    output_hash: Digest,
}
```

## Result Classes

Diagnostics and facts:

```text
TP = expected vulnerable/required item was observed
FN = expected vulnerable/required item was not observed
TN = expected safe/forbidden item was not observed
FP = expected safe/forbidden item was observed
EXTRA = observed item with no matching expected item
UNCONFIRMED = observed item cannot be classified because truth is partial
REJECTED = extension/provider item failed validation
```

Graph edges:

```text
MATCHED_REQUIRED = expected edge found
MISSING_REQUIRED = expected edge missing
FORBIDDEN_FOUND = forbidden edge found
EXTRA_CONFIRMED_FALSE = extra edge known false
EXTRA_UNCONFIRMED = extra edge not in partial truth
```

Paths:

```text
ENDPOINT_MATCH = source and sink correct
EVIDENCE_MATCH = source, sink, and required evidence match
NOISY_MATCH = correct endpoints, poor explanation
MISSING_PATH = no source-to-sink path
SPURIOUS_PATH = path to safe sink or from safe source
```

## Metric Set

```rust
struct MetricSet {
    counts: ConfusionCounts,
    precision: Option<f64>,
    recall: Option<f64>,
    f1: Option<f64>,
    f2: Option<f64>,
    f3: Option<f64>,
    fpr: Option<f64>,
    suite_native: BTreeMap<String, f64>,
    graph: GraphMetrics,
    paths: PathMetrics,
    unknowns: UnknownMetrics,
    performance: PerformanceMetrics,
    extension_delta: Option<ExtensionDelta>,
}
```

## Extension Delta

```rust
struct ExtensionDelta {
    new_true_positives: usize,
    removed_false_positives: usize,
    removed_false_negatives: usize,
    new_false_positives: usize,
    new_unknowns: usize,
    resolved_unknowns: usize,
    accepted_extension_facts: usize,
    rejected_extension_facts: usize,
    runtime_overhead_ratio: f64,
    cache_invalidation_scope: InvalidationSummary,
}
```

An extension improves analysis only if the delta is visible. It should not be possible for an extension to silently rewrite the answer and hide the cost.

## Determinism Requirements

Every evaluation run must be deterministic for the same inputs:

- stable case ordering;
- stable diagnostic ordering;
- stable fact ordering;
- stable graph edge ordering;
- stable JSON keys;
- normalized paths;
- normalized line endings;
- normalized timestamps excluded from output hash;
- cache keys included in debug output.

The report should include an output hash over normalized results. Fast CI should repeat selected cases twice and fail on output drift.

# Validation Plan

## Validation Thesis

Module graph accuracy must be measured at multiple layers. A package manager can be exact about selected dependencies while import resolution remains wrong, and a build tool can be exact about targets while package-manager lockfile edges are absent.

Validation should therefore score:

```text
roots
packages/projects/source sets
declared requirements
resolved dependency edges
import-to-package edges
repo topology overlays
extension deltas
```

## Validation Layers

### 1. Parser Tests

Use small fixtures for each format:

- valid minimal file;
- valid real-world-ish file;
- unknown fields;
- comments/trailing commas where supported;
- malformed file diagnostics;
- source spans.

### 2. Fact Invariant Tests

Every emitted fact must satisfy:

- stable ID exists;
- provider/provenance exists;
- precision label exists;
- source file exists unless marked generated/external;
- dependency target either resolves or has explicit unresolved status;
- conditions are preserved.

### 3. Tool Oracle Tests

For external tools, run only in opt-in benchmark jobs:

```python
observed = polint_module_graph(repo)
expected = external_tool_graph(repo)
score = compare(expected, observed)
```

Use tool output as validation evidence, not as mandatory product runtime dependency.

### 4. Cache Tests

Change one input at a time:

- manifest dependency change;
- lockfile version change;
- workspace glob change;
- tsconfig path change;
- go.work use change;
- Python dependency group change;
- extension provider code change.

Assert affected layers invalidate and unrelated layers remain cached.

### 5. Extension Merge Tests

Fixtures:

- extension adds package root;
- extension resolves import alias;
- extension adds generated source set;
- extension conflicts with lockfile exact edge;
- extension suppresses heuristic fact;
- extension emits invalid path.

Expected:

- valid additions merge with `ExtensionAsserted` or `ExtensionValidated`;
- invalid facts are rejected with diagnostics;
- conflicts keep both facts in evidence side table.

## Accuracy Metrics

```text
precision = true_positive_edges / observed_edges
recall = true_positive_edges / expected_edges
unknown_rate = unknown_facts / total_relevant_facts
condition_preservation = conditioned_edges_with_conditions / conditioned_edges
false_exactness = facts_claimed_exact_but_oracle_disagrees
```

False exactness should block promotion. Unknown facts are acceptable when they are specific and actionable.

## Required Regression Gates

- deterministic output ordering;
- no hidden network dependency in default scans;
- no mandatory external package manager execution;
- no exact facts from unsupported dynamic build logic;
- lockfile schema version included in provenance;
- package-manager config included in cache key;
- declared and resolved edges remain separate.

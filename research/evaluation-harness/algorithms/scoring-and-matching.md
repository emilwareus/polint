# Algorithms: Scoring And Matching

Pseudo-code is intentionally stripped down.

## Diagnostic Matching

Goal: match benchmark expected findings to polint diagnostics without brittle exact-line matching.

```python
def score_diagnostics(expected, observed, config):
    observed_index = index_observed_diagnostics(observed, config)
    used_observed = set()
    counts = Counts()
    matches = []

    for exp in expected:
        candidates = observed_index.lookup(
            file=normalize_path(exp.file),
            kind_set=exp.acceptable_kinds,
            cwe_set=exp.acceptable_cwes,
            line_bucket=line_bucket(exp.line, config.line_tolerance),
        )

        best = choose_best_diagnostic_match(exp, candidates, used_observed, config)

        if exp.vulnerable:
            if best is None:
                counts.fn += 1
                matches.append(Match("FN", exp, None))
            else:
                counts.tp += 1
                used_observed.add(best.id)
                matches.append(Match("TP", exp, best))
        else:
            if best is None:
                counts.tn += 1
                matches.append(Match("TN", exp, None))
            else:
                counts.fp += 1
                used_observed.add(best.id)
                matches.append(Match("FP", exp, best))

    for obs in observed:
        if obs.id not in used_observed and not config.ignore_unmatched_observed:
            matches.append(Match("EXTRA", None, obs))

    return Score(counts=counts, matches=matches)
```

Complexity:

```text
index build: O(O)
matching:    O(E * average_candidates)
with file/CWE/line buckets, average_candidates should be small
```

Do not use an all-pairs matcher unless a suite truly requires fuzzy matching across the entire corpus.

## Diagnostic Match Ranking

```python
def choose_best_diagnostic_match(exp, candidates, used, config):
    best = None
    best_score = -1

    for obs in candidates:
        if obs.id in used:
            continue

        score = 0

        if paths_equal(exp.file, obs.file):
            score += 50

        if cwe_compatible(exp, obs):
            score += 25

        if kind_compatible(exp, obs):
            score += 20

        if line_within_tolerance(exp.line, obs.line, config.line_tolerance):
            score += max(0, 15 - abs(exp.line - obs.line))

        if exp.function and symbols_compatible(exp.function, obs.function):
            score += 10

        if exp.sink and symbols_compatible(exp.sink, obs.sink):
            score += 10

        if exp.source and symbols_compatible(exp.source, obs.source):
            score += 10

        if score > best_score:
            best = obs
            best_score = score

    if best_score < config.minimum_match_score:
        return None

    return best
```

The match score is not a quality metric. It is a deterministic way to select the best observed diagnostic when a suite allows tolerant matching.

## Confusion Metrics

```python
def metrics(counts):
    tp, fp, fn, tn = counts.tp, counts.fp, counts.fn, counts.tn

    precision = safe_div(tp, tp + fp)
    recall = safe_div(tp, tp + fn)
    fpr = safe_div(fp, fp + tn)

    return {
        "precision": precision,
        "recall": recall,
        "f1": f_beta(precision, recall, beta=1),
        "f2": f_beta(precision, recall, beta=2),
        "f3": f_beta(precision, recall, beta=3),
        "fpr": fpr,
        "tpr_minus_fpr": recall - fpr,
    }

def f_beta(precision, recall, beta):
    if precision is None or recall is None:
        return None
    b2 = beta * beta
    return safe_div((1 + b2) * precision * recall, b2 * precision + recall)
```

Suite-native scores can be useful for comparability. They should not replace precision/recall/F-score reporting.

## Fact Matching

```python
def score_facts(expected_facts, observed_facts):
    by_stable_key = {}
    by_structural_key = {}

    for fact in observed_facts:
        if fact.stable_key:
            by_stable_key[fact.stable_key] = fact
        by_structural_key[structural_key(fact)].append(fact)

    results = []

    for exp in expected_facts:
        obs = None

        if exp.stable_key:
            obs = by_stable_key.get(exp.stable_key)

        if obs is None:
            obs = choose_structural_fact_match(
                exp,
                by_structural_key.get(structural_key(exp), []),
            )

        if obs is None:
            results.append(FactResult("MISSING", exp, None))
            continue

        if not precision_satisfies(obs.precision, exp.required_precision):
            results.append(FactResult("WRONG_PRECISION", exp, obs))
            continue

        if not value_matches(exp.expected_value, obs.value):
            results.append(FactResult("WRONG_VALUE", exp, obs))
            continue

        results.append(FactResult("MATCH", exp, obs))

    return results
```

Complexity:

```text
index build: O(O)
match:       O(E + small structural buckets)
```

Native fixtures should prefer stable keys. External fixtures may need structural selectors.

## Graph Edge Matching

```python
def score_graph_edges(expected_edges, observed_edges):
    observed = {edge_key(edge): edge for edge in observed_edges}
    used = set()
    results = []

    for exp in expected_edges:
        key = edge_key(exp)
        obs = observed.get(key)

        if exp.truth_kind in ["MustExist", "StaticExpected", "DynamicObserved"]:
            if obs:
                results.append(EdgeResult("MATCHED_REQUIRED", exp, obs))
                used.add(key)
            else:
                results.append(EdgeResult("MISSING_REQUIRED", exp, None))

        elif exp.truth_kind == "MustNotExist":
            if obs:
                results.append(EdgeResult("FORBIDDEN_FOUND", exp, obs))
                used.add(key)
            else:
                results.append(EdgeResult("FORBIDDEN_ABSENT", exp, None))

    for edge in observed_edges:
        key = edge_key(edge)
        if key in used:
            continue

        if truth_is_complete_for(edge):
            results.append(EdgeResult("EXTRA_CONFIRMED_FALSE", None, edge))
        else:
            results.append(EdgeResult("EXTRA_UNCONFIRMED", None, edge))

    return results
```

Complexity:

```text
O(expected_edges + observed_edges)
```

Important: for dynamic traces, missing observed dynamic edges are false negatives, but extra static edges are not automatically false positives because the trace did not exercise every path.

## Path Matching

```python
def score_paths(expected_paths, observed_paths):
    index = index_paths_by_endpoint(observed_paths)
    results = []

    for exp in expected_paths:
        candidates = index.lookup(exp.source, exp.sink)
        best = None
        best_score = -1

        for path in candidates:
            score = path_similarity(exp, path)
            if score > best_score:
                best = path
                best_score = score

        if best is None:
            results.append(PathResult("MISSING_PATH", exp, None))
        elif best_score >= 90:
            results.append(PathResult("EVIDENCE_MATCH", exp, best))
        elif best_score >= 60:
            results.append(PathResult("NOISY_MATCH", exp, best))
        else:
            results.append(PathResult("ENDPOINT_ONLY", exp, best))

    return results

def path_similarity(exp, path):
    score = 0

    if endpoint_matches(exp.source, path.source):
        score += 35
    if endpoint_matches(exp.sink, path.sink):
        score += 35
    if all(contains_node(path, node) for node in exp.required_intermediate):
        score += 15
    if not any(contains_node(path, node) for node in exp.forbidden_intermediate):
        score += 10
    if exp.max_explanation_steps is None or len(path.steps) <= exp.max_explanation_steps:
        score += 5

    return score
```

Complexity:

```text
index build: O(P)
matching:    O(E * average_endpoint_candidates * path_length)
```

Avoid comparing every expected path to every observed path.

## Default-Vs-Extension Delta

```python
def compare_default_vs_extended(default_run, extended_run):
    default_items = normalize_run_items(default_run)
    extended_items = normalize_run_items(extended_run)

    default_by_key = {item.key: item for item in default_items}
    extended_by_key = {item.key: item for item in extended_items}

    delta = Delta()

    for key, item in extended_by_key.items():
        if key not in default_by_key:
            delta.added.append(item)
        elif item != default_by_key[key]:
            delta.changed.append((default_by_key[key], item))

    for key, item in default_by_key.items():
        if key not in extended_by_key:
            delta.removed.append(item)

    delta.metric_delta = subtract_metrics(extended_run.metrics, default_run.metrics)
    delta.resolved_unknowns = count_resolved_unknowns(default_run, extended_run)
    delta.new_unknowns = count_new_unknowns(default_run, extended_run)
    delta.accepted_extension_facts = count_extension_facts(extended_run, status="accepted")
    delta.rejected_extension_facts = count_extension_facts(extended_run, status="rejected")
    delta.runtime_overhead_ratio = safe_div(
        extended_run.performance.wall_time_ms,
        default_run.performance.wall_time_ms,
    )

    return delta
```

The delta report should name concrete cases. Aggregate metrics are not enough for an agent to improve a model.

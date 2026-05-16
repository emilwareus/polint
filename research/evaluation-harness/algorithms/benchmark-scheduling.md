# Algorithms: Benchmark Scheduling, Baselines, And Invalidation

## Tier Selection

```python
def select_cases(suites, tier, seed):
    selected = []

    for suite in suites:
        tier_config = suite.tiers.get(tier)
        if not tier_config or not tier_config.enabled:
            continue

        cases = suite.adapter.load_cases(suite)
        cases = filter_cases(cases, tier_config.selector)
        cases = stable_sample(cases, tier_config.sample, seed, suite.version_digest)

        selected.extend(cases)

    return sorted(selected, key=lambda case: (case.suite_id, case.case_id))
```

Sampling must be stable. If the fast tier samples 50 cases, the same suite version and seed should always select the same 50 cases.

## Run Scheduling

```python
def run_evaluation(cases, mode, machine, config):
    groups = group_by_setup(cases)
    results = []

    for group in topological_setup_order(groups):
        workspace = prepare_workspace(group)

        for batch in parallel_batches(group.cases, config.max_parallelism):
            batch_results = parallel_map(
                lambda case: run_case(case, workspace, mode, machine, config),
                batch,
            )
            results.extend(batch_results)

        cleanup_workspace(workspace, keep_on_failure=config.keep_failed)

    return normalize_results(results)
```

External suites often have setup constraints. Do not blindly parallelize every case if they share package installs, Docker services, or generated repos.

## Case Cache Keys

```python
def case_eval_key(case, mode, config):
    return hash_all([
        case.suite_id,
        case.suite_version,
        case.case_id,
        case.source_commit,
        case.expected_digest,
        mode.name,
        config.polint_commit,
        config.polint_eval_schema_version,
        config.rule_digest,
        config.extension_digests,
        config.analysis_config_digest,
        config.provider_versions_digest,
    ])
```

This is the evaluation cache key. It is separate from the analysis-kernel layer cache keys.

## Baseline Comparison

```python
def compare_to_baseline(current, baseline, thresholds):
    failures = []

    for suite_id in current.suites:
        cur = current.metrics[suite_id]
        old = baseline.metrics[suite_id]
        t = thresholds.for_suite(suite_id)

        if dropped(cur.recall, old.recall, t.max_recall_drop):
            failures.append(Failure("recall_drop", suite_id, old.recall, cur.recall))

        if dropped(cur.precision, old.precision, t.max_precision_drop):
            failures.append(Failure("precision_drop", suite_id, old.precision, cur.precision))

        if increased(cur.false_positive_traps, old.false_positive_traps, t.max_new_fp_traps):
            failures.append(Failure("fp_trap_regression", suite_id))

        if increased_ratio(cur.wall_time_ms, old.wall_time_ms, t.max_runtime_ratio):
            failures.append(Failure("runtime_regression", suite_id))

        if increased_ratio(cur.peak_rss_bytes, old.peak_rss_bytes, t.max_memory_ratio):
            failures.append(Failure("memory_regression", suite_id))

    return failures
```

Baselines should be compared on the same CI machine class when possible. For local developer runs, report regressions but do not fail unless explicitly requested.

## Determinism Check

```python
def determinism_check(case, mode, config):
    first = run_case(case, mode, config)
    second = run_case(case, mode, config)

    if first.normalized_output_hash != second.normalized_output_hash:
        return Failure(
            "nondeterministic_output",
            case_id=case.case_id,
            first_hash=first.normalized_output_hash,
            second_hash=second.normalized_output_hash,
            diff=diff_normalized_outputs(first, second),
        )

    return None
```

Run this only on fast subsets by default. Full-suite determinism checks can double runtime.

## Extension Safety Gate

```python
def extension_gate(default, extended, thresholds):
    delta = compare_default_vs_extended(default, extended)
    failures = []

    if delta.rejected_extension_facts > thresholds.max_rejected_facts:
        failures.append(Failure("too_many_rejected_extension_facts"))

    if delta.new_false_positives > thresholds.max_new_false_positives:
        failures.append(Failure("extension_added_false_positives"))

    if delta.metric_delta.recall < thresholds.min_recall_improvement:
        failures.append(Failure("extension_no_recall_gain"))

    if delta.runtime_overhead_ratio > thresholds.max_runtime_overhead:
        failures.append(Failure("extension_too_expensive"))

    if delta.new_unknowns > thresholds.max_new_unknowns:
        failures.append(Failure("extension_added_unknowns"))

    return failures
```

This gate should be configurable. Some extensions are intended to improve precision by reducing false positives, not recall.

## Cache Invalidation Regression

```python
def cache_invalidation_check(original_case, edit, expected_invalidated_layers):
    first = run_case(original_case, mode="default")
    edited_case = apply_edit(original_case, edit)
    second = run_case(edited_case, mode="default")

    actual = second.cache_stats.invalidated_layers

    if not set(actual).issubset(set(expected_invalidated_layers)):
        return Failure(
            "over_invalidation",
            expected=expected_invalidated_layers,
            actual=actual,
        )

    if required_layers_not_invalidated(edit, actual):
        return Failure(
            "under_invalidation",
            edit=edit,
            actual=actual,
        )

    return None
```

This belongs in native fixtures. External benchmarks do not generally specify cache behavior.

# Agent Authoring Algorithms

The pseudocode here describes the intended authoring loop for AI agents.

## Artifact Classification

```python
def classify_gap(gap):
    if gap.policy_expressible_with_existing_views and gap.output_is_diagnostic:
        return "rule"

    if gap.describes_existing_api_behavior and gap.fits_fixed_model_schema:
        return "model"

    if gap.describes_function_behavior_reused_by_callers:
        return "summary"

    if gap.needs_new_fact_family or gap.needs_repo_code_to_recover_facts:
        return "provider_extension"

    return "fixture_or_research"
```

## Agent Loop

```python
def author_artifact(goal):
    facts = run_json("polint facts list")
    unknowns = run_json("polint unknowns --all")
    gap = analyze_goal_against_facts(goal, facts, unknowns)
    artifact_kind = classify_gap(gap)

    scaffold = run_json(f"polint new-{artifact_kind} {gap.name}")
    edit(scaffold.files)

    while True:
        test_result = run_json("polint test --format json")
        if test_result.ok:
            break

        failures = summarize_failures(test_result)
        edit(repair_from_failures(scaffold.files, failures))

    diff = run_json(f"polint diff --{artifact_kind} {gap.name} --format json")
    if not acceptable_delta(diff):
        add_fixture_or_downgrade_precision(diff)
        rerun_tests()

    return CommitCandidate(scaffold.files, test_result, diff)
```

## Rule Manifest Generation

```python
def expand_rule_macro(rule_fn):
    validate_plain_sync_function(rule_fn)
    ctx = validate_first_param_is_rule_ctx(rule_fn.params[0])
    views = []
    options = None

    for param in rule_fn.params[1:]:
        if is_fact_view(param.type):
            views.append(param.type)
        elif is_options_type(param.type):
            options = param.type
        else:
            compile_error("unsupported rule parameter")

    capabilities = derive_capabilities(views)

    manifest = RuleManifest(
        id=rule_attr.id,
        description=rule_attr.description,
        severity=rule_attr.severity,
        docs=rule_attr.docs,
        tags=rule_attr.tags,
        fact_views=views,
        capabilities=capabilities,
        option_schema=derive_option_schema(options),
        fixability=rule_attr.fixability,
        stability=rule_attr.stability,
        sdk_version=current_sdk_version(),
    )

    return GeneratedRule(run_wrapper=build_run_wrapper(rule_fn, views), manifest=manifest)
```

## Capability Planning

```python
def plan_rule(rule_manifest, engine_capabilities):
    missing = []
    setup_missing = []
    providers = []

    for capability in rule_manifest.capabilities:
        support = engine_capabilities.lookup(capability)
        if support is None:
            missing.append(capability)
        elif support.status == "setup_missing":
            setup_missing.append((capability, support.reason))
        else:
            providers.extend(support.provider_chain)

    if missing or setup_missing:
        return CapabilityPlan(
            runnable=False,
            diagnostics=capability_diagnostics(rule_manifest, missing, setup_missing),
        )

    return CapabilityPlan(runnable=True, providers=dedupe_toposort(providers))
```

## Fixture Runner

```python
def run_polint_test(pack, filters):
    rule_host = compile_rule_pack_once(pack.rules_crate)
    cases = discover_cases(pack.tests_dir, filters)
    results = []

    for case in parallel(cases):
        temp_repo = copy_fixture_to_temp_repo(case)
        output = run_polint_check(
            temp_repo,
            rule_host=rule_host,
            config=case.config,
            format="json",
            cache=case.cache_policy,
        )

        normalized = normalize_diagnostics(output)
        inline_result = check_inline_markers(case.files, normalized)
        snapshot_result = compare_snapshot(case.expected_snapshot, normalized)

        if case.kind in ["model", "provider"]:
            facts = export_fact_snapshot(temp_repo, case)
            fact_result = compare_snapshot(case.fact_snapshot, facts)
        else:
            fact_result = None

        results.append(CaseResult(case, inline_result, snapshot_result, fact_result))

    return TestRun(results)
```

## Inline Marker Matching

```python
def check_inline_markers(files, diagnostics):
    markers = parse_markers(files)

    for marker in markers.expect:
        if not diagnostic_matches(marker, diagnostics):
            fail("missing expected diagnostic", marker)

    for marker in markers.ok:
        if diagnostic_at_marker(marker, diagnostics):
            fail("unexpected diagnostic", marker)

    for diagnostic in diagnostics:
        if not covered_by_expect_or_snapshot(diagnostic, markers):
            warn_or_fail("unmarked diagnostic", diagnostic)
```

## Model Validation

```python
def validate_model_row(row, semantic_index):
    symbol = semantic_index.resolve(row.matcher)
    if symbol is None:
        return reject("unknown symbol")

    if row.access_path and not access_path_valid(symbol, row.access_path):
        return reject("invalid access path")

    if row.argument_index and row.argument_index >= symbol.arity:
        return reject("impossible argument index")

    if not model_kind_supported(row.kind, row.target_analysis):
        return reject("unsupported model kind")

    return accept(ModelFact(
        symbol=symbol,
        kind=row.kind,
        access_path=row.access_path,
        precision=row.precision,
        provenance=row.provenance,
    ))
```

## Provider Extension Validation

```python
def run_provider_extension(provider, input_snapshot):
    handshake = provider.handshake()
    validate_protocol(handshake)

    output = provider.run(input_snapshot)
    facts = []
    diagnostics = []

    for candidate in output.facts:
        result = validate_candidate_fact(candidate)
        if result.accepted:
            facts.append(result.fact)
        else:
            diagnostics.append(result.diagnostic)

    digest = stable_digest(facts, diagnostics, handshake.provider_version)
    return ProviderResult(facts=facts, diagnostics=diagnostics, output_digest=digest)
```

## Default Versus Extended Delta

```python
def compare_default_vs_extended(case, extension_or_model):
    default = run_polint(case, extensions=[])
    extended = run_polint(case, extensions=[extension_or_model])

    return DeltaReport(
        added_facts=extended.facts - default.facts,
        removed_unknowns=default.unknowns - extended.unknowns,
        added_diagnostics=extended.diagnostics - default.diagnostics,
        removed_diagnostics=default.diagnostics - extended.diagnostics,
        runtime_delta=extended.runtime - default.runtime,
        precision_delta=score_precision(extended) - score_precision(default),
    )
```

The agent should not activate a model/provider unless the delta is understood.

# Capability Plans

`polint explain plan` shows the analysis plan derived from enabled repo-local
rules. It is intended for rule authors, CI, and agents that need to inspect
required capabilities without parsing source files. For normal
`#[polint::rule]` rules, capabilities come from the typed fact-view parameters in
the rule function signature.

Use machine output when another tool consumes the plan:

```bash
polint explain plan --format json
```

The JSON root fields are `schema`, `digest`, `rules`, `capabilities`, and `setup_checks`.

## JSON Shape

- `schema` is currently `analysis-plan-v1`.
- `digest` is the stable digest used to identify the resolved plan.
- `rules` lists enabled rules with `id`, `description`, `severity`, and
  `capabilities`.
- `capabilities` lists requested capability rows with `name`, `status`, `rules`,
  `reason`, `hint`, and `docs_path`.
- `setup_checks` lists setup probes with `id`, `status`, `message`, and
  `docs_path`.

Status values are lowercase: `supported`, `unsupported`, and `setup_missing`.

## Phase 11 Support

Phase 11 treats these capability names as supported:

- `syntax`
- `imports`
- `go_tests`
- `branch_obligations`
- `ts_components`
- `ts_classes`
- `string_literals`
- `jsx_attributes`

These names are reserved but unsupported in Phase 11:

- `cfg`
- `call_graph`
- `coverage_facts`
- `test_suite_metrics`

Unsupported reserved capabilities appear in `polint explain plan` and produce a
`polint/capability` diagnostic during `polint check`.

Use go_tests for current Go test evidence; test_suite_metrics is reserved for normalized metrics.

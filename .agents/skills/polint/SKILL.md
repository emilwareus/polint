---
name: polint
description: Use polint to write and run repo-local static-analysis policy rules.
allowed-tools: Bash(polint:*) Bash(cargo:*) Read Write Edit Glob Grep LS
---

# polint Repo-Local Policy Rules

Use this skill for project-specific linting rules that generic tools cannot
know. Rules live in `.polint/rules/`, import `polint::sdk::prelude::*`, use
`#[polint::rule]`, and register through `polint::runner::run_cli`.
Start rule modules with `use polint::sdk::prelude::*;`.

## Fast Workflow

```bash
polint init
polint new-rule ts no-raw-colors
polint new-rule ts no-secret-logs --template secret-to-log
polint test --format json
polint inspect rule --format json
polint check --format ai-friendly --fail-on none
```

`polint new-rule <lang> <name>` creates a rule module, wires it into
`.polint/rules/src/main.rs`, and creates positive and negative fixture cases
under `.polint/tests/rules/<name_with_underscores>/`. For v1.4 policy-query starters, use
`--template <id>` with TypeScript for `request-to-shell`, `secret-to-log`,
`pii-to-analytics`, `sensitive-write-guard`, `transaction-cleanup`,
`raw-reachable-api`, `ssrf`, `dangerous-html`, `unsafe-deserialization`, or
`user-file-path`. Go currently supports `sensitive-write-guard`,
`transaction-cleanup`, and `raw-reachable-api`. Templates are repo-local
scaffolds to edit, not built-in rules enabled by polint.

## Agent JSON

Use bounded, versioned JSON for agent workflows:

```bash
polint inspect rule --format json
polint test --format json
polint facts list --format json
polint facts sample --cap resolved_imports --limit 20 --format json
polint unknowns --cap references --format json
polint explain --rule local/no-raw-colors --format json
```

Do not rely on internal debug, eval, provider, parser, or layer-cache output as a
public contract. `facts list` reports stable and reserved fact-view
dispositions. `unknowns` reports supported public setup/resolution gaps,
preview policy query unknowns, and reserved-capability unsupported rows.
`explain` reports macro-derived fact views and capability support without
exposing provider execution graphs.

## Rule Authoring

Prefer typed fact views in the rule signature:

- `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` for architecture policies.
- `Symbols<'_>` and `References<'_>` for identity-aware policies.
- `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>` for
  reusable quality signals.
- `StringLiterals<'_>` and `JsxAttributes<'_>` for TS/JS literal and JSX rules.
- `GoTests<'_>` and `BranchObligations<'_>` for Go branch/test policies.
- `ChangedFiles<'_>` for review rules (see below).
- Preview policy query views `Events<'_>` and `Calls<'_>` are provider-backed
  for Phase 56 call-event and reachable-call policies. `ControlFlow<'_>` uses
  CFG-backed operation order plus refined call targets for Phase 57
  same-function call-event guard and cleanup policies, with MIR/source ordering
  only as fallback when CFG facts are absent. `DataFlow<'_>` is provider-backed
  for Phase 58 bounded
  source/sink/barrier policies. Policy diagnostics include `policy_query`,
  `policy_query_version`, `query_digest`, `policy_status`, and
  `policy_precision` evidence.

Keep `RuleCtx` narrow: diagnostics, options/settings, path helpers, and
capability/setup metadata. Do not import `polint::core`, parser adapters,
`AnalysisDb`, provider modules, or eval/debug internals from repo-local rules.

## Review Rules

`polint review <ref>` is `polint check` gated to a diff against a target branch or
commit. Mark a rule `#[polint::rule(..., kind = "review")]` and request
`ChangedFiles<'_>` to read the diff (`iter`, `contains_path`, `matches_glob`,
`lines_for`; per-entry `path`/`status`/`lines`/`is_*`). The view is empty under
`polint check`. By default `polint review` surfaces only diagnostics intersecting the
diff (changed file plus changed line ranges); `--no-diff-gate` shows all review
findings and `--whole-file` gates by file only. Anchor whole-file watchers on a
changed line so the line-aware gate keeps them. Scaffold with
`polint new-rule generic <name> --review`. See `docs/facts/changed-files.md` and
`examples/review-rules/`. Keep heuristic claims heuristic.

Reserved advanced views such as raw `Cfg<'_>`, raw `CallGraph<'_>`,
`Evidence<'_>`, model packs, provider extensions, and `polint eval` are not
stable rule-authoring APIs unless a future public docs page and temp-repo test
explicitly promote them.

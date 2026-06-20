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
polint test --format json
polint inspect rule --format json
polint check --format ai-friendly --fail-on none
```

`polint new-rule <lang> <name>` creates a rule module, wires it into
`.polint/rules/src/main.rs`, and creates positive and negative fixture cases
under `.polint/tests/rules/<name>/`.

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
dispositions. `unknowns` reports supported public setup/resolution gaps and
reserved-capability unsupported rows. `explain` reports macro-derived fact views
and capability support without exposing provider execution graphs.

## Rule Authoring

Prefer typed fact views in the rule signature:

- `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` for architecture policies.
- `Symbols<'_>` and `References<'_>` for identity-aware policies.
- `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>` for
  reusable quality signals.
- `StringLiterals<'_>` and `JsxAttributes<'_>` for TS/JS literal and JSX rules.
- `GoTests<'_>` and `BranchObligations<'_>` for Go branch/test policies.
- Preview policy query views `Events<'_>`, `Calls<'_>`, `ControlFlow<'_>`,
  and `DataFlow<'_>` compile through the SDK prelude, but fail closed with
  `polint/capability` until their provider-backed query behavior lands.

Keep `RuleCtx` narrow: diagnostics, options/settings, path helpers, and
capability/setup metadata. Do not import `polint::core`, parser adapters,
`AnalysisDb`, provider modules, or eval/debug internals from repo-local rules.

Reserved advanced views such as raw `Cfg<'_>`, raw `CallGraph<'_>`,
`Evidence<'_>`, model packs, provider extensions, and `polint eval` are not
stable rule-authoring APIs unless a future public docs page and temp-repo test
explicitly promote them.

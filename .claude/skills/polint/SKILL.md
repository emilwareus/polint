---
name: polint
description: Use polint to write and run repo-local static-analysis policy rules.
allowed-tools: Bash(polint:*) Bash(cargo:*) Read Write Edit MultiEdit Glob Grep LS
---

# polint Repo-Local Policy Rules

Use this skill when the user wants project-specific linting rules, policy checks,
or static analysis that generic tools cannot know. polint ships no built-in
policy rules; every policy belongs to the repository that needs it.

## Fast Workflow

```bash
polint init
polint new-rule go require-error-branch-tests
polint new-rule ts no-raw-colors
polint check --profile fast --fail-on none
```

Use `polint check --format json` when you need machine-readable diagnostics. JSON
is a versioned report object with a `diagnostics` array (not a bare array at the
root); the schema lives in `docs/schemas/polint-report-v1.json` in the polint repo.
Human output uses ANSI colors on a TTY unless `NO_COLOR` is set; use `--color never` for plain text. Use `polint check --format sarif` for CI upload paths. Use `--fail-on warn`, `error`,
or `none` to control the exit status. Use `polint explain go-test --file … --test …` to print one harvested `TestFact` as JSON when debugging Go tests.

## Rule Layout

Repo-local rules live in **one** Rust package under `.polint/rules/`:

```text
.polint.toml
.polint/rules/Cargo.toml
.polint/rules/src/main.rs          # calls polint::runner::run_cli(vec![...])
.polint/rules/src/my_rule.rs       # one module per #[polint::rule] function
```

`polint new-rule <lang> <name>` adds `src/<name_with_underscores>.rs` and wires it
into `src/main.rs`. See `examples/multiple-rules` in the polint repo for several
rules in one pack.

## Writing A Rule

Start with `use polint::sdk::prelude::*;`, give the rule a stable local ID, and
request the facts it needs as typed parameters. The `#[polint::rule]` macro
derives capabilities from those fact-view parameters. Keep the function shape
plain: first parameter `&mut RuleCtx<'_>`, typed fact views like
`Imports<'_>`, and a `RuleResult` or `RuleResult<()>` return.

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-colors",
    description = "Require design tokens instead of raw color literals.",
    severity = "error"
)]
fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
    jsx: JsxAttributes<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    for literal in literals.iter() {
        if literal.value.starts_with('#') {
            ctx.report(
                Diagnostic::error(
                    rule_id.clone(),
                    ctx.file_path(literal.file),
                    literal.span.diagnostic_range(),
                    "Use a design token instead of a raw color literal.",
                )
                .with_evidence("literal", literal.value.clone()),
            );
        }
    }
    let _ = jsx.iter().count();
    Ok(())
}
```

## Config Pattern

Keep the profile explicit so CI and local runs execute the same policies:

```toml
[workspace]
include = ["src/**"]
exclude = ["**/node_modules/**", "**/vendor/**"]

[rules]
paths = [".polint/rules"]

[profiles.fast]
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
severity = "error"
files = ["src/**/*.{ts,tsx}"]
allow_files = ["src/theme/**"]
```

## Agent Rules

- Do not add project policies to the polint CLI as built-ins.
- Keep rules small and specific to the repository convention they enforce.
- State when a rule is heuristic, especially for test evidence or branch coverage.
- Prefer parser facts and SDK helpers over ad hoc text scanning.
- Do not implement `Rule` manually or write handwritten capability declarations.
- Do not use async, generics, local lookalike fact types, or type aliases as
  fact-view parameters in `#[polint::rule]` functions.
- Add the smallest real fixture that demonstrates the policy violation.
- Run the rule through the CLI before claiming it works.

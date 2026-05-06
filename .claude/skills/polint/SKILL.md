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
.polint/rules/src/my_rule.rs       # one module per rule (pub struct + impl Rule)
```

`polint new-rule <lang> <name>` adds `src/<name_with_underscores>.rs` and wires it
into `src/main.rs`. See `examples/multiple-rules` in the polint repo for several
rules in one pack.

## Writing A Rule

Start with `use polint::sdk::prelude::*;`, give the rule a stable local ID, declare
only the facts it needs in `capabilities`, then report diagnostics from `run`.

```rust
use polint::sdk::prelude::*;

struct NoRawColors;

impl Rule for NoRawColors {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "local/no-raw-colors".to_string(),
            description: "Require design tokens instead of raw color literals.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().string_literals().jsx_attributes()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult {
        for literal in ctx.string_literals() {
            if literal.value.starts_with('#') {
                ctx.report(
                    Diagnostic::error(
                        self.meta().id,
                        ctx.file_path(literal.file),
                        literal.span.diagnostic_range(),
                        "Use a design token instead of a raw color literal.",
                    )
                    .with_evidence("literal", literal.value.clone()),
                );
            }
        }
        Ok(())
    }
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
- Add the smallest real fixture that demonstrates the policy violation.
- Run the rule through the CLI before claiming it works.

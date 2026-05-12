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
polint check --fail-on none
```

Use `polint check --format json` when you need machine-readable diagnostics. JSON
is a versioned report object with a `diagnostics` array (not a bare array at the
root); the schema lives in `docs/schemas/polint-report-v1.json` in the polint repo.
Human output uses ANSI colors on a TTY unless `NO_COLOR` is set; use `--color never`
for plain text. Use `polint check --format sarif` for CI upload paths. Use
`--fail-on warn`, `error`, or `none` to control the exit status. Use `polint check
--shortstat` or `polint check --stat` for human scan summaries; these flags do
not add prose to JSON or SARIF output.

Use a compact YAML baseline at `.polint/baseline.yaml` when existing findings
should not block new work:

```bash
polint baseline create
polint check --baseline --new-only
polint baseline update
```

The baseline file has one string per entry:

```yaml
version: 1

baseline:
  - "local/backend-context-propagation e337fbb73d44b2b7 backend/app/handler.go"
ignore:
  - "local/no-raw-colors 1b7c9a00e493aa21 frontend/Button.tsx"
```

`baseline` is existing debt; it stays visible but does not fail. `ignore` is a
central accepted suppression; it is hidden from output and failure. Baseline
matching uses `rule_id + fingerprint` and refreshes unambiguous moved paths;
ignore matching is file-specific so unrelated findings with the same fingerprint
stay visible.

Use `polint ignores` when you need to find suppressions that should be fixed:

```bash
polint ignores --shortstat
polint ignores --stat --filter local/no-raw-colors,local/*
polint ignores --format json --filter local/no-raw-colors
```

Ignore comments look like
`// polint-ignore-next-line local/no-raw-colors -- legacy fixture`. Selectors are
required. Ignores suppress policy diagnostics only; parser, internal,
capability, and `polint/*` diagnostics stay visible. Repositories can require
reasons with `[ignores] require_reason = true` in `.polint.toml`.

## Rule Layout

Repo-local rules live in **one** Rust package under `.polint/rules/`:

```text
.polint.toml
.polint/rules/Cargo.toml
.polint/rules/src/main.rs          # calls polint::runner::run_cli(vec![...])
.polint/rules/src/my_rule.rs       # one #[polint::rule] function per rule
```

`polint new-rule <lang> <name>` adds `src/<name_with_underscores>.rs` and wires it
into `src/main.rs`. See `examples/multiple-rules` in the polint repo for several
rules in one pack.

## Writing A Rule

Start with `use polint::sdk::prelude::*;`, register the rule with
`polint::runner::run_cli`, give the rule a stable local ID in `#[polint::rule]`,
and request facts as typed fact-view parameters. polint derives the rule's
capabilities from those parameter types.
Use `ctx.options().settings` for rule-specific TOML fields that are not covered
by the common shortcuts (`max`, `deny`, `forbidden_imports`, etc.).

`src/main.rs`:

```rust
use std::process::ExitCode;

mod no_raw_colors;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_raw_colors::no_raw_colors()])
}
```

`src/no_raw_colors.rs`:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-colors",
    description = "Require design tokens instead of raw color literals.",
    severity = "error"
)]
pub(crate) fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    for literal in literals.iter() {
        if literal.value.starts_with('#') {
            ctx.report(
                Diagnostic::error(
                    ctx.rule_id(),
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
```

## Reusable Metric Signals

For code-quality policies, prefer reusable signal views over rules calling other
rules. `FileMetrics<'_>` exposes file line/byte/function counts,
`FunctionMetrics<'_>` exposes per-function size, and `ComplexityMetrics<'_>`
exposes per-function syntax-level cyclomatic complexity. A composite rule can
request several of these typed views in one `#[polint::rule]` signature.

## Config Pattern

Profiles are explicit named subsets. `polint check` with no `--profile` runs
every discovered rule. Add a named profile only when the repository explicitly
needs a subset, and treat unknown profile names as errors:

```toml
[workspace]
include = ["src/**"]
exclude = ["**/node_modules/**", "**/vendor/**"]

[rules]
paths = [".polint/rules"]

[[rules.config]]
id = "local/no-raw-colors"
severity = "error"
files = ["src/**/*.{ts,tsx}"]
allow_files = ["src/theme/**"]
```

## Agent Rules

- Do not add project policies to the polint CLI as built-ins.
- Document only stable, supported CLI workflows; keep debug helpers, exploratory analysis surfaces, and future/TBD behavior out of generated skills until they are intentionally promoted.
- Keep rules small and specific to the repository convention they enforce.
- State when a rule is heuristic, especially for test evidence or branch coverage.
- Prefer parser facts and SDK helpers over ad hoc text scanning.
- Request typed fact views in the `#[polint::rule]` signature; examples are consumers of the SDK, not special internal entry points.
- Compose `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>` for higher-level quality rules instead of making rules depend on other rules.
- Do not implement `Rule` manually or write handwritten capability declarations.
- For custom config, prefer explicit fields in `[[rules.config]]` and read them through `ctx.options().settings`.
- Add the smallest real fixture that demonstrates the policy violation.
- Run the rule through the CLI before claiming it works.

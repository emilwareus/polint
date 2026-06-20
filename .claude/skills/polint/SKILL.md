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
polint test --format json
polint inspect rule --format json
polint check --format ai-friendly --fail-on none
```

Use `polint check --format ai-friendly --fail-on none` when you are an AI agent
or when a repository may have many findings. It prints counts by rule and at
most 10 example diagnostics, then saves full JSON under `.polint/output/`
(`.polint/output/latest.json` is the stable path). Do not `cat` the whole file
into your prompt; query it with bounded commands:

```bash
jq '.summary.by_rule' .polint/output/latest.json
jq '[.diagnostics[] | select(.rule_id=="local/no-raw-colors")][0:20]' .polint/output/latest.json
jq '.diagnostics[] | select(.file=="src/Button.tsx") | {rule_id, range, message}' .polint/output/latest.json | head -c 12000
```

Use `polint check --format json` when another program needs the full report on
stdout. JSON is a versioned report object with a `diagnostics` array (not a bare
array at the root); the schema lives in `docs/schemas/polint-report-v1.json` in
the polint repo. Human output uses ANSI colors on a TTY unless `NO_COLOR` is set;
use `--color never` for plain text. Use `polint check --format sarif` for CI
upload paths. Use `--fail-on warn`, `error`, or `none` to control the exit
status. Use `polint check --shortstat` or `polint check --stat` for human scan
summaries; these flags do not add prose to JSON or SARIF output.

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
into `src/main.rs`, then creates positive and negative fixture cases under
`.polint/tests/rules/<name_with_underscores>/`. See `examples/multiple-rules` in
the polint repo for several rules in one pack.

## Agent JSON

Use versioned, bounded JSON commands when deciding what a rule can request:

```bash
polint inspect rule --format json
polint test --format json
polint facts list --format json
polint facts sample --cap resolved_imports --limit 20 --format json
polint unknowns --cap references --format json
polint explain --rule local/no-raw-colors --format json
```

`facts list` reports stable and reserved fact-view dispositions. `facts sample`
requires a bounded limit and emits only public fact fields. `unknowns` reports
public setup/resolution gaps for supported facts and unsupported rows for
reserved capabilities such as dataflow. `explain` reports macro-derived fact
views and capability support; it does not expose provider execution graphs,
layer-cache internals, or eval/debug schemas.

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

## Module Relationship Facts

For architecture policies, request `ResolvedImports<'_>` to inspect resolution
status and unresolved reasons, and request `ModuleGraphFacts<'_>` to inspect
file, package, module, and dependency edges. Both views are exported by
`polint::sdk::prelude::*`; keep rules on the typed fact-view path. When
relationship rules run, `Unresolved`, `Dynamic`, and `Unsupported` statuses are
inspectable fact data. `SetupMissing` is reported as a `polint/capability`
diagnostic and blocks requesting rules until resolver setup exists.

## Symbol And Reference Facts

For identity-aware policies, request `Symbols<'_>` and `References<'_>` as typed
fact-view parameters. Use `symbols.by_name("name")` to find candidate symbols,
`symbols.definitions(symbol.id)` to inspect declarations, `references.to(symbol.id)`
to inspect resolved uses of one symbol, and `references.unresolved()` to review
names that could not be bound. Check `SymbolPrecision` and
`SymbolResolutionStatus` before treating a reference as exact.

TS/JS symbol facts use Oxc for exact local lexical facts and module-linked import
aliases where module resolution succeeds. They do not claim TypeScript
type-checker, cross-file member/property, or declaration-file precision. Go
symbol facts use typed package information when the sidecar can run, normally via
Go 1.24+ on `PATH`, and analyzed Go files belong to module roots. Monorepos are
configured in the single `.polint.toml` file with `[languages.go].module_roots`,
or inferred from nearest `go.mod` files. Setup gaps are reported as
`polint/capability` diagnostics. Symbol/reference facts are not call graph, CFG,
dataflow, coverage, or Go SSA facts.

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
- Treat raw `Cfg<'_>`, raw `CallGraph<'_>`, `Evidence<'_>`, model packs, provider extensions, and `polint eval` as reserved/preview/internal unless public docs and temp-repo tests explicitly promote them. The policy query views `Events<'_>` and `Calls<'_>` are provider-backed for Phase 56 call-event and reachable-call policies; `ControlFlow<'_>` is provider-backed for Phase 57 same-function call-event guard and cleanup policies; `DataFlow<'_>` still fails closed until bounded source/sink/barrier behavior lands.
- Document only stable, supported CLI workflows; keep debug helpers, exploratory analysis surfaces, and future/TBD behavior out of generated skills until they are intentionally promoted.
- Keep rules small and specific to the repository convention they enforce.
- State when a rule is heuristic, especially for test evidence or branch coverage.
- Prefer parser facts and SDK helpers over ad hoc text scanning.
- Request typed fact views in the `#[polint::rule]` signature; examples are consumers of the SDK, not special internal entry points.
- Compose `FileMetrics<'_>`, `FunctionMetrics<'_>`, and `ComplexityMetrics<'_>` for higher-level quality rules instead of making rules depend on other rules.
- For architecture rules, compose `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` instead of parsing import strings yourself.
- For identity rules, compose `Symbols<'_>` and `References<'_>` and inspect precision/status fields before assuming a reference is exact.
- Do not implement `Rule` manually or write handwritten capability declarations.
- For custom config, prefer explicit fields in `[[rules.config]]` and read them through `ctx.options().settings`.
- Add the smallest real fixture that demonstrates the policy violation.
- Run the rule through the CLI before claiming it works.

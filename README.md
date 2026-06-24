# polint

**Repo-local lint rules for the policies only your team knows.**

polint is a Rust framework for writing static-analysis rules that live in your
repository. It gives those rules fast file discovery, parsers, typed facts,
diagnostics, caching, CI output, and an SDK. You bring the policy.

Use it for conventions that generic linters cannot know: internal API usage,
security guardrails, migration review rules, design-token rules, test-quality
expectations, and the project-specific checks you keep repeating in prompts and
review comments.

polint is not a replacement for ESLint, Biome, Ruff, golangci-lint, or
formatters. It is the layer for rules that belong to your codebase.

![polint diagnostic for a raw-color literal in Button.tsx](https://raw.githubusercontent.com/emilwareus/polint/main/docs/img/example-no-raw-colors.svg)

## Why

Engineering teams are putting more work through AI coding agents, but agents do
not reliably remember local conventions from `AGENTS.md`, prompts, or review
comments. polint turns the parts that are statically checkable into executable
feedback.

The rule code stays in your repo, next to the code it protects. That makes the
policy reviewable, testable, versioned, and runnable locally or in CI.

polint ships no built-in policy rules.

## Start

```bash
cargo install polint --locked

polint init
polint new-rule ts no-raw-colors
polint test --format json
polint check
```

`polint init` creates a local rule pack under `.polint/rules`. Rules are normal
Rust functions that use the public SDK and ask for the fact views they need.

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
    let rule_id = ctx.rule_id().to_string();

    for literal in literals.iter() {
        if literal.value.starts_with('#') {
            ctx.report(Diagnostic::error(
                rule_id.clone(),
                ctx.file_path(literal.file),
                literal.span.diagnostic_range(),
                "Use a design token instead of a raw color literal.",
            ));
        }
    }

    Ok(())
}
```

polint derives the required analysis from the rule signature, runs the relevant
Go / TypeScript / JavaScript parsers, and emits deterministic diagnostics for
humans, agents, and CI.

## Review

`polint review <ref>` runs the same rule-as-code setup against a diff:

```bash
polint review origin/main
```

Review rules are useful for policies that should only fire on changed code:
migration ownership, risky API usage in touched files, generated-code checks, or
anything else that belongs in code review instead of a full-repo gate.

## CI

```yaml
name: polint

on: [push, pull_request]

jobs:
  polint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: emilwareus/polint@v1
        with:
          args: check --format github
```

## Docs

- [Examples](examples/)
- [Agent and CI playbook](docs/AGENT-PLAYBOOK.md)
- [Consumer setup and troubleshooting](docs/CONSUMER-SETUP.md)
- [GitHub Action](docs/GITHUB-ACTION.md)
- [Fact reference](docs/facts/)
- [Comment ignores](docs/IGNORE-COMMENTS.md)

## License

[MIT](LICENSE)

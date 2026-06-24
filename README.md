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

## Why

Engineering teams are putting more work through AI coding agents, but agents do
not reliably remember local conventions from `AGENTS.md`, prompts, or review
comments. polint turns the parts that are statically checkable into executable
feedback.

The rule code stays in your repo, next to the code it protects. That makes the
policy reviewable, testable, versioned, and runnable locally or in CI.

polint ships no built-in policy rules.

## Example

Say a PR adds or edits a GORM model:

```go
type Invoice struct {
    ID        uuid.UUID `gorm:"type:uuid;primaryKey"`
    AccountID uuid.UUID `gorm:"index:idx_invoices_account_status_created_at,priority:1"`
    Status    string    `gorm:"index:idx_invoices_account_status_created_at,priority:2"`
    CreatedAt time.Time `gorm:"index:idx_invoices_account_status_created_at,priority:3"`
}
```

Your repo can make that a review requirement:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/gorm-model-read-indexes",
    description = "GORM model changes require read-index validation.",
    severity = "error",
    kind = "review"
)]
pub(crate) fn gorm_model_read_indexes(
    ctx: &mut RuleCtx<'_>,
    changes: ChangedFiles<'_>,
) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();

    for changed in changes.iter() {
        let is_gorm_model =
            changed.path().ends_with(".go") && changed.matches_glob("internal/**/models/**");

        if changed.is_deleted() || !is_gorm_model {
            continue;
        }

        let line = changed.lines().first().map(|&(lo, _)| lo).unwrap_or(1);
        ctx.report(
            Diagnostic::error(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(line, 1),
                "GORM model changed: validate the correct read indexes for this model.",
            )
            .with_help(
                "Check the read paths for this model and add or update composite indexes in \
                 GORM tags or migrations. If no index is needed, explain why in the PR.",
            ),
        );
    }

    Ok(())
}
```

Run it during review:

```bash
polint review origin/main
```

The rule is ordinary Rust in your repo. It is a review gate, not a query-plan
verifier: it enforces the step your team cares about exactly where the model
changed.

See the checked-in [GORM review indexes example](examples/gorm-review-indexes/)
for the runnable rule pack and fixture model.

## Start

```bash
cargo install polint --locked

polint init
polint new-rule generic gorm-model-read-indexes --review
polint review origin/main
```

`polint init` creates a local rule pack under `.polint/rules`. Rules are normal
Rust functions that use the public SDK and ask for the fact views they need.

polint derives the required analysis from the rule signature, runs the relevant
Go / TypeScript / JavaScript parsers, and emits deterministic diagnostics for
humans, agents, and CI.

## CI

```yaml
name: polint-review

on: [pull_request]

jobs:
  polint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
        with:
          fetch-depth: 0
      - uses: emilwareus/polint@v1
        with:
          args: review origin/main --format github
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

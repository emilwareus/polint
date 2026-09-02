# Review Rules


`polint review <ref>` is `polint check` gated to a diff against a target branch or
commit (`origin/main`, a SHA, or `a...b`). Use it for review-time policies that
should only fire on what a change touched.

A review rule is authored exactly like a check rule, but it is marked
`kind = "review"` and reads the diff through the `ChangedFiles<'_>` fact view.
For example, say a PR adds or edits a GORM model:

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
polint new-rule generic gorm-model-read-indexes --review
polint review origin/main
```

`ChangedFiles<'_>` exposes `iter()`, `contains_path()`, `matches_glob()`, and
`lines_for()`; each entry has `path()`, `status()`, `lines()`, and
`is_added/is_modified/is_deleted/is_renamed()`. It is empty under `polint check`,
so review rules are inert there. By default `polint review` surfaces only
diagnostics intersecting the diff (changed file plus changed line ranges), so any
rule effectively becomes "check, but only on the diff"; `--no-diff-gate` shows
all review findings and `--whole-file` gates by file only.

See [the changed-files fact reference](facts/changed-files.md), the
[review-rules example](../examples/review-rules/), and the
[GORM review indexes example](../examples/gorm-review-indexes/).


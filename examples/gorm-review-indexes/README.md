# GORM Review Indexes Example

This example models a review-only policy for Go services that use GORM models.

The policy is `review/gorm-model-read-indexes`. When a PR adds or edits a Go
file under `internal/**/models/**`, the rule emits a review diagnostic asking
the author to validate the model's read indexes.

This is intentionally a review gate, not a query-plan verifier. It makes the
team's database-review step executable at the point where a model changes.

## Run It

Review rules need a git diff. From a checkout of this example as its own git
repo:

```bash
polint review origin/main
```

For a local smoke test, initialize a temporary git repo around this directory,
commit the base model, edit `internal/billing/models/invoice.go`, then run:

```bash
polint review HEAD~1 --format json --fail-on none
```

The expected finding is `review/gorm-model-read-indexes` on the changed model
file.

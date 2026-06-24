# Review rules example

`polint review <ref>` is `polint check` with the **identical rule-as-code setup**,
gated so a rule fires only against a **diff to a target branch or commit**. Review
rules are normal `#[polint::rule]` Rust functions that use the full SDK and analysis
engine; the only differences from a check rule are `kind = "review"` and that they
run under `polint review` instead of `polint check`. Nothing lives in TOML.

This pack has two review rules.

## `review/migrations` — the simple rule

Fires when any path under `db/migrations/**` changes. "Fire when this path changes"
is ordinary Rust: the rule reads the diff through the `ChangedFiles<'_>` fact view
and globs each changed path. It anchors its diagnostic on the first changed line so
the finding lands inside the diff under the default gate (see below).

See [`.polint/rules/src/migrations.rs`](.polint/rules/src/migrations.rs).

## `review/public-api-change` — the complex rule

Runs real symbol/reference analysis but restricts it to changed files: for every
exported symbol defined in a changed file that is referenced from another file, it
flags a public-API impact for review. The diff (`ChangedFiles<'_>`) narrows the work;
`Symbols<'_>` and `References<'_>` do the analysis. The heuristic is repo-local
policy, not exact API-compatibility analysis.

See [`.polint/rules/src/public_api_change.rs`](.polint/rules/src/public_api_change.rs).

## The diff gate

By default, `polint review` surfaces only diagnostics that intersect the diff — both
the changed **file** and (unless `--whole-file`) the changed **line ranges**. So a
review rule is "check, but only on the diff" for free. Opt out with `--no-diff-gate`
to see every review finding regardless of the diff, or use `--whole-file` to gate on
changed files only and ignore line ranges. Because the gate is line-aware, rules that
want to fire for a whole-file concern (like the migration watcher) should anchor their
diagnostic on a changed line via `ChangedFileRef::lines()`.

## Running it

Review rules only fire against a diff, so they need a git repo and a target ref:

```sh
# From a checkout of this example as its own git repo:
polint review origin/main          # diff HEAD against the merge-base with origin/main
polint review <commit-sha>         # diff against a specific commit
polint review <base>...<head>      # an explicit three-dot range
```

`polint check` runs only check-kind rules, so it never emits these review diagnostics.

Inside this repository the pack is a workspace member, so `cargo build` /
`cargo clippy` cover it; it is exercised against a real diff by the `polint review`
integration tests in `crates/polint/tests/cli.rs`.

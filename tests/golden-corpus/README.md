# Golden corpus - analysis targets

This directory is the **input surface** for behavioural characterization: the
repositories and rule packs polint may be run against when locking observable
output, cost, and capability.

| Kind | Source | Materialization |
|------|--------|-----------------|
| Example rule packs | `example_rule_packs` in [`inputs.toml`](inputs.toml) | Checked in under `examples/*/.polint/rules/` |
| Eval fixture trees | `eval_fixture_trees` in [`inputs.toml`](inputs.toml) | Checked in under `tests/eval-fixtures/` |
| Scale repositories | `scale_suite_manifests` in [`inputs.toml`](inputs.toml) | Cloned by `make fetch-scale-repos` into gitignored `research/evaluation-harness/repos/` at the **commit SHA** declared in each suite manifest |

`inputs.toml` is the inventory. Adding an example pack or fixture tree without
updating it fails `cargo test -p polint --test golden_corpus --locked`.

Scale checkouts are never floated on a branch tip: the fetch script reads
`source_url` and `source_commit` from the suite manifests and checks out that
exact commit.

This directory does **not** store golden diagnostic outputs or run the CLI
harness; those belong to later characterization work.

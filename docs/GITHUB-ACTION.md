# GitHub Action

The repository ships a composite GitHub Action that installs polint, restores
the documented `.polint/cache` directory, runs polint, and saves the cache after
the run.

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
          version: latest
          args: check --format github
```

`--format github` emits GitHub Actions annotations with file, line, column,
severity, rule id, and message. The command still exits according to polint's
normal `--fail-on` behavior.

## Inputs

| Input | Default | Notes |
|---|---:|---|
| `version` | `latest` | Installs a GitHub Release asset. Accepts `latest`, `vX.Y.Z`, or `X.Y.Z`. If an asset is missing, the action falls back to `cargo install polint --locked`. |
| `args` | `check --format github` | Arguments passed to `polint`. |
| `cache` | `true` | Restores and saves `.polint/cache`. |
| `cache-key-prefix` | `polint` | Prefix for the GitHub Actions cache key. |
| `working-directory` | `.` | Directory where `polint` should run. |
| `fail-on` | empty | Optional convenience value appended as `--fail-on <value>`. Leave empty if `args` already contains `--fail-on`. |

## Caching

The action caches one stable path:

```text
.polint/cache
```

That path includes analysis artifacts and the repo-local rule-host Cargo target
cache at `.polint/cache/rules-target`. The primary cache key includes:

- runner OS
- installed polint version
- `.polint.toml`
- repository `Cargo.lock`
- `rust-toolchain.toml`
- `.polint/rules/Cargo.toml`
- `.polint/rules/Cargo.lock`
- `.polint/rules/src/**/*.rs`

Repositories without `.polint/rules` still work; missing files simply do not
contribute to the hash.

Caching does not make a fully cold first scan free. The first run on a fresh key
may still compile repo-local rules and populate analysis data. Repeat CI runs
with the same relevant inputs should restore the rule-host target directory and
analysis cache.

Use `polint cache status` locally or in CI to inspect the restored cache, and
`polint cache prune` / `polint cache clean` when you need explicit cleanup.

## Versioning

The action is published from this repository as `emilwareus/polint@v1`. The
release workflow moves the `v1` tag to the reviewed release commit when
`publish_action` is enabled. Patch and minor polint releases can continue to use
`version: latest`; pin `version: vX.Y.Z` when a workflow must run an exact CLI
release.

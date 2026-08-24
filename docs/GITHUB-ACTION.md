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
| `cache` | `true` | Master switch for both cache entries below. |
| `cache-rule-builds` | `true` | Restores and saves Cargo build intermediates for repo-local rule hosts. Ignored when `cache` is `false`. |
| `rule-paths` | empty | Rule package directories the build cache covers, newline- or comma-separated. Empty means `.polint/rules`. Set this when `.polint.toml` configures a different `[rules].paths`. |
| `cache-key-prefix` | `polint` | Prefix for the GitHub Actions cache keys. |
| `working-directory` | `.` | Directory where `polint` should run. |
| `fail-on` | empty | Optional convenience value appended as `--fail-on <value>`. Leave empty if `args` already contains `--fail-on`. |

## Outputs

| Output | Notes |
|---|---|
| `version` | Installed polint version. |
| `cache-hit` | Whether the exact analysis cache key was restored. |
| `rule-build-cache-hit` | Whether the exact rule-host build cache key was restored. Empty when rule-host build caching did not run. |
| `rule-build-cache-skipped` | Why rule-host build caching did not run, or empty when it ran. |
| `exit-code` | polint process exit code. |

## Caching

The action keeps two cache entries with different jobs, different keys, and
different safety arguments. Both live under the documented `.polint/cache`
paths, which are unchanged.

### 1. Analysis cache (source-validated artifacts)

```text
.polint/cache/analysis
.polint/cache/layers
.polint/cache/derived
.polint/cache/semantic-store
```

Key: `<prefix>-analysis-<runner os>-<polint version>-<hash of>`

- `.polint.toml`
- repository `Cargo.lock`
- `rust-toolchain.toml`
- `.polint/rules/Cargo.toml`
- `.polint/rules/Cargo.lock`
- `.polint/rules/src/**/*.rs`

Fallback: the same key without the file hash, so an older entry for the same
polint version can warm-start a run. There is no fallback across polint
versions, because polint stamps its own artifact keys with the version and would
reject those entries anyway.

Restoring this entry never bypasses validation. polint keys every artifact by
source path and content, loaded config, rule/options digest, requested
capability plan, cache format, and polint version; anything that does not match
the current sources is a miss, and an artifact that cannot be decoded is
evicted. A restored entry can only save recomputation of facts about files that
are byte-identical to what is being analyzed now.

Repositories without `.polint/rules` still work; missing files simply do not
contribute to the hash.

### 2. Rule-host build cache (compiler output)

```text
.polint/cache/rules-target
```

This is the Cargo target directory `polint check` uses when it builds repo-local
rule packages (`cargo run --manifest-path .polint/rules/Cargo.toml`). Compiling
those packages means compiling the `polint` library and its dependencies, which
dominates the check phase on a cold runner.

Key: `<prefix>-rules-build-v1-<runner os>-<runner arch>-<polint version>-env-<build env digest>-deps-<dependency digest>`

The build env digest covers:

- runner OS and architecture
- `POLINT_RULES_PROFILE` (the Cargo profile used for rule hosts)
- `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`
- `CARGO_BUILD_TARGET`, `CARGO_INCREMENTAL`
- `RUSTC_WRAPPER`, `RUSTC_WORKSPACE_WRAPPER`, `RUSTUP_TOOLCHAIN`
- the resolved `rustc -vV` and `cargo -V`, read from the working directory so a
  checked-in `rust-toolchain.toml`, a pinned action toolchain, or a floating
  `stable` that has moved are all reflected

The dependency digest covers the files that decide which dependency units Cargo
resolves and builds:

- `.polint.toml`
- repository `Cargo.toml`, `Cargo.lock`
- `rust-toolchain.toml`, `rust-toolchain`
- `.cargo/config.toml`, `.cargo/config`
- `Cargo.toml` and `Cargo.lock` of every covered rule package

Fallback: exactly one key, identical up to and including the build env digest,
relaxing only the dependency digest. Every restored unit still comes from the
same compiler, flags, profile, architecture, and polint version, and Cargo
re-fingerprints each unit before reusing it. There is no broader fallback.

Rule sources are deliberately **not** part of this key. After restoring the
entry — on an exact hit as well as a fallback hit — the action makes every file
in each covered rule package newer than everything that was restored. Cargo
decides freshness for path units by comparing source timestamps against the
recorded fingerprint, so the rule packages are always recompiled from the
sources in this checkout, and only registry dependency units survive as reuse.
No cached rule-host binary can outlive the sources it was built from.

### When rule-host build caching is skipped

The action reports the reason in `rule-build-cache-skipped` and in the job log:

| Condition | Reason |
|---|---|
| `cache-rule-builds` is not `true` | opted out |
| `POLINT_CACHE_DIR` is set | the cache root moved; the action also skips the analysis cache |
| `POLINT_RULES_TARGET_DIR` is set | the target directory left the cache root |
| `.polint.toml` sets a `[rules].paths` the action cannot recognize as the default and `rule-paths` is unset | the key would not cover every rule package |
| no rule package found | nothing to build |
| no Rust toolchain, or no `sha256sum`/`shasum` | the key cannot be computed |

Skipping only forgoes the speedup: `polint check` still builds and runs the rule
hosts, and reports build failures itself.

### Notes

Caching does not make a fully cold first scan free. The first run on a fresh key
still compiles repo-local rules and populates analysis data.

The build cache entry grows as dependency sets change, because Cargo does not
remove superseded units from a target directory. GitHub evicts caches by least
recent use within the repository's 10 GB budget. To retire entries deliberately,
bump `cache-key-prefix`.

Use `polint cache status` locally or in CI to inspect the restored cache, and
`polint cache prune` / `polint cache clean` when you need explicit cleanup.

## Versioning

The action is published from this repository as `emilwareus/polint@v1`. The
release workflow moves the `v1` tag to the reviewed release commit when
`publish_action` is enabled. Patch and minor polint releases can continue to use
`version: latest`; pin `version: vX.Y.Z` when a workflow must run an exact CLI
release.

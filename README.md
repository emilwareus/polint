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

There are two ways to run your rules:

- **`polint check`** — run every rule across the repository. This is the core of
  polint and where most rules live.
- **`polint review <ref>`** — run rules gated to a diff against a target branch
  or commit, for review-time policies that should only fire on what changed. See
  [Review Rules](#review-rules).

## Why

Engineering teams are putting more work through AI coding agents, but agents do
not reliably remember local conventions from `AGENTS.md`, prompts, or review
comments. polint turns the parts that are statically checkable into executable
feedback.

The rule code stays in your repo, next to the code it protects. That makes the
policy reviewable, testable, versioned, and runnable locally or in CI.

polint ships no built-in policy rules. It gives you the SDK, parsers, facts,
diagnostics, local rule runner, config, cache, and CI output so your repo can own
the rules.

## Quick Example

Say your frontend must use design tokens instead of raw colors. A polint rule in
your repo can catch the violation and tell the AI agent exactly how to fix it:

![polint diagnostic for a raw-color literal in Button.tsx](https://raw.githubusercontent.com/emilwareus/polint/main/docs/img/example-no-raw-colors.svg)

That is the point: the rule does not just fail the code. It injects the missing
project context back into the agent at the moment it needs to repair the change.

## Try It

Install polint:

```bash
cargo install polint --locked
```

The default build includes Go and TypeScript/JavaScript analysis. Slim installs
can select one frontend without compiling the other parser family:

```bash
cargo install polint --no-default-features --features lang-go
cargo install polint --no-default-features --features lang-typescript
```

`all-languages` enables both frontends explicitly. If a repository contains a
language whose feature is disabled, polint reports a `polint/capability`
diagnostic instead of running rules against placeholder facts. Rule packs scaffolded by that binary inherit the same language-feature selection, so the local rule host does not silently restore excluded parsers. Existing rule packs are not rewritten automatically; when upgrading one, set its `polint` dependency to `default-features = false` and list the same `lang-go` and/or `lang-typescript` features explicitly.

Or from GitHub Releases:

```bash
curl -sSfL https://raw.githubusercontent.com/emilwareus/polint/main/scripts/install.sh | bash
```

Run a self-contained example:

```bash
git clone https://github.com/emilwareus/polint.git
cd polint/examples/config-denied-literal
polint check --color always --fail-on none
```

Expected output:

![polint check on examples/config-denied-literal showing a denied literal diagnostic](https://raw.githubusercontent.com/emilwareus/polint/main/docs/img/example-config-denied-literal.svg)

## Use It In Your Repo

```bash
polint init
polint add-skill
polint new-rule ts no-raw-colors
polint new-rule ts no-secret-logs --template secret-to-log
polint test --format json
polint inspect rule --format json
polint check
```

`polint init` creates `.polint.toml`, `.polint/rules/src/`, `.polint/cache/`, `.polint/output/`, `.polint/.gitignore` (ignoring `cache/` and `output/`), and root `rust-toolchain.toml` when missing.
`polint new-rule <go|ts|js|generic> <name>` adds a Rust rule module to your
local rule pack. `polint check` discovers and runs that rule pack.
Generated rules include positive and negative fixture cases under
`.polint/tests/rules/`, so `polint test --format json` can verify the local
policy loop before you run it across the workspace. Use `--template <id>` for a
repo-local policy starter that you edit to your APIs: `request-to-shell`,
`secret-to-log`, `pii-to-analytics`, `sensitive-write-guard`,
`transaction-cleanup`, `raw-reachable-api`, `ssrf`, `dangerous-html`,
`unsafe-deserialization`, and `user-file-path`. These are scaffolds, not bundled
rules enabled by polint. `polint inspect rule --format json`, `polint facts list
--format json`, `polint unknowns --cap references --format json`, and
`polint explain --rule <id> --format json` are bounded, versioned JSON surfaces
for agent workflows.

Rule packs live in your repo:

```text
.polint.toml
.polint/
  rules/
    Cargo.toml
    src/
      main.rs
      no_raw_colors.rs
```

Rules should use the public SDK (`polint::sdk::prelude::*`) and runner
(`polint::runner::run_cli`) only. Rule modules use `#[polint::rule]` functions:
the typed fact-view parameters (`StringLiterals<'_>`, `Imports<'_>`,
`GoTests<'_>`, and similar) are the facts the rule can read, and polint derives
the analysis capabilities from that function signature. Rule functions are plain
sync Rust functions with `&mut RuleCtx<'_>` first and a `RuleResult` return.
`RuleCtx` is for
reporting diagnostics, source paths, rule options, and capability/setup
metadata. The fact reference in [docs/facts/](docs/facts/) describes the raw and
derived building blocks available to rule authors: functions, reusable metric
signals, imports, branches, Go tests, TS/JS facts, literals, and JSX attributes.
Rule-specific TOML fields that are not one of the common shortcuts are available
through `ctx.options().settings`.

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

Profiles are explicit:

- `polint check` runs every discovered rule.
- `polint check --profile web` runs exactly `[profiles.web]`.
- Unknown profiles are errors.
- Profile names are arbitrary. There is no default profile.

## Cache

polint keeps local, untracked cache data under `.polint/cache` by default:

```text
.polint/cache/
  analysis/           compact parser/fact JSON artifacts     [source-validated]
  layers/             per-layer fact manifests and blobs     [source-validated]
  derived/            reserved for project-level derived facts [source-validated]
  semantic-store/     durable semantic store, when enabled   [source-validated]
  rules-target/       Cargo target dir for repo-local rule hosts [compiler-output]
  extensions-target/  Cargo target dir for repo-local extensions [compiler-output]
  review/             serialized `polint review` changesets  [scratch]
```

Source-validated data is re-validated against current sources on every read;
compiler output is Cargo's to judge; scratch is rebuilt from the current inputs.
CI has to key those roles differently — see the
[GitHub Action guide](docs/GITHUB-ACTION.md).

`--no-cache` disables analysis/fact cache reads and writes for that run. It does
not disable the `rules-target` Cargo cache, because rebuilding the local rule
host on every run is usually wasted work.
Analysis cache keys include the source path and content, rule/options digest,
loaded config, requested capability plan, cache format, and polint version.
If a cache artifact cannot be decoded, polint treats it as a miss and removes
that artifact.

Useful commands:

```bash
polint cache status
polint cache status --format json
polint cache prune --max-size-mb 512
polint cache prune --max-age-days 14 --dry-run
polint cache clean --category analysis
polint cache clean
```

Set `POLINT_CACHE_DIR` to move the whole cache root, or
`POLINT_RULES_TARGET_DIR` to move only the repo-local rule-host Cargo target
directory. Repo-local rule hosts run with Cargo's `release` profile by default;
set `POLINT_RULES_PROFILE=dev` for faster unoptimized local rule development, or
to another Cargo profile name to build with `--profile <name>`.

### Shared rule-host store

`.polint/cache` is per checkout, so every git worktree of a repository would
otherwise recompile the same rule host from the same bytes. polint shares the
compiled **rule-host binaries** — nothing else — through one machine-global
store:

```bash
POLINT_CACHE_STORE=/path/to/store polint check   # a store you name
POLINT_CACHE_STORE=off polint check              # no sharing this run
```

Unset, the store lives in the platform user cache directory:
`$XDG_CACHE_HOME/polint/store` (else `~/.cache/polint/store`) on Linux,
`~/Library/Caches/polint/store` on macOS, and `%LOCALAPPDATA%\polint\store` on
Windows. `off`, `disabled`, or `none` turns sharing off; so does any value that
is not an absolute path, because a relative one would name a different store
from every directory.

For packages eligible for sharing, each entry is keyed by the build's
**complete input surface**: the resolved `rustc` and `cargo`, the toolchain
override, the compiler-flag environment variables, the cargo profile, the OS
and architecture, the repository's
`Cargo.toml`/`Cargo.lock`/`rust-toolchain*`, every applicable ancestor and Cargo
home `.cargo/config*`, the rule package's manifests and lockfile, and the content
of every file in the package — Rust sources, `build.rs`, and data consumed by
`include_bytes!` or a build script. The fingerprint also records whether the
package tripped the checkout-path gate described below. Everything is hashed
from bytes, never modification times. A restored binary is re-hashed against
the length and digest the entry recorded before it is moved into place and run,
and any failure — a missing entry, an unreadable one, a corrupt blob — is a miss
that compiles locally instead. Committing the rule package's `Cargo.lock` is
what lets a fresh checkout compute the same key the build in another one
published under.

Three constructs make polint build locally without restoring or publishing a
machine-global host: any `path` dependency that leaves the rule package
(including an absolute path, whose contents have no lockfile checksum); a cargo
config with `patch`, `replace`, `paths`, source replacement, `include`, `[env]`,
or a target runner; and Rust source that can embed checkout-specific values
Cargo injects. The last check scans `*.rs` under `src`, `benches`, `examples`,
and `tests`, plus a root `build.rs`, for the byte tokens
`CARGO_MANIFEST_DIR`, `OUT_DIR`, and the executable environment names `CARGO`
and `RUSTC`. It is intentionally broad: even a token in a comment opts that
package out, because a false positive costs one local build while a false
negative could restore a host compiled for another checkout. `CARGO_PKG_*`
values are not gated because they come from checkout-independent package
metadata. A config or source tree polint cannot read or prove is local-only too.

The store is one user's state on one machine, at the trust level of
`~/.cargo/registry`: polint creates it `0700` on Unix and never shares it between
users or over a network. Point `POLINT_CACHE_STORE` only at local storage you
alone can write. Deleting the directory is always safe.

### Contributor build cache

The repository disables Cargo incremental compilation because its artifacts are
stored separately in every worktree and can grow much larger than the final
build outputs. For shared compile reuse across worktrees, install
[`sccache`](https://github.com/mozilla/sccache) and configure it once in
`~/.cargo/config.toml`:

```toml
[build]
rustc-wrapper = "sccache"
```

The repository caps sccache's shared local cache at 10 GiB. Each worktree keeps
its own `target` directory, avoiding Cargo lock contention and collisions
between concurrently developed branches.

## Comment Ignores

Use comment ignores for intentional, local suppressions:

```ts
// polint-ignore-next-line local/no-raw-colors -- legacy fixture
const color = "#ff00aa";
```

`polint-ignore-line`, `polint-ignore-next-line`, `polint-ignore-start` /
`polint-ignore-end`, and top-of-file `polint-ignore-file` are supported.
Selectors are required and use exact IDs, `prefix/*`, or `*`. Ignores suppress
policy diagnostics only; parser, internal, capability, and `polint/*`
diagnostics still report.

To inspect ignored debt:

```bash
polint ignores --stat --filter local/no-raw-colors
```

See [docs/IGNORE-COMMENTS.md](docs/IGNORE-COMMENTS.md).
The checked-in [comment-ignores example](examples/comment-ignores/README.md)
shows one suppressed finding and one visible finding from the same rule.

For quick human scan summaries during normal checks:

```bash
polint check --shortstat
polint check --stat
```

These flags summarize scanned files, diagnostics, and ignore suppression counts
for human output. They do not change JSON or SARIF output.

For AI agents or large repositories, use the compact agent-oriented format:

```bash
polint check --format ai-friendly --fail-on none
```

This prints counts by triggered rule plus at most 10 example diagnostics, then
saves the full report under `.polint/output/`. The stable path is
`.polint/output/latest.json`. Do not paste the whole file into an agent prompt;
query it selectively:

```bash
jq '.summary.by_rule' .polint/output/latest.json
jq '[.diagnostics[] | select(.rule_id=="local/no-raw-colors")][0:20]' .polint/output/latest.json
jq '.diagnostics[] | select(.file=="src/Button.tsx") | {rule_id, range, message}' .polint/output/latest.json | head -c 12000
```

## Baselines

Use a baseline when adopting polint in a repository that already has valid
findings. The baseline is always checked in at `.polint/baseline.yaml` as
compact YAML:

```yaml
version: 1

baseline:
  - "local/backend-context-propagation e337fbb73d44b2b7 backend/app/handler.go"
ignore:
  - "local/no-raw-colors 1b7c9a00e493aa21 frontend/Button.tsx"
```

Each entry is one string:

```text
<rule_id> <fingerprint> <file>
```

`baseline` entries are existing debt: they stay visible in human output but do
not fail the process. `ignore` entries are central accepted exceptions: they are
suppressed from output and failure. Baseline matching uses `rule_id +
fingerprint` and refreshes unambiguous moved paths; ignore matching is
file-specific to avoid suppressing unrelated findings with the same fingerprint.

```bash
polint baseline create
polint check --baseline --new-only
polint baseline update
```

`--new-only` emits and fails only on diagnostics not covered by the baseline or
central ignore list.

## Review Rules

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

See [docs/facts/changed-files.md](docs/facts/changed-files.md), the
[review-rules example](examples/review-rules/), and the
[GORM review indexes example](examples/gorm-review-indexes/).

## CI

Run `polint check` on push and pull requests:

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

For diff-gated review rules, run `polint review` on pull requests with full
history (`fetch-depth: 0`) so the target ref is available:

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

Rules that request Go symbol/reference facts use embedded Go sidecars by
default, which need Go 1.25 or newer on `PATH`. polint supports monorepos by
inferring Go module roots from discovered files, or from
`[languages.go].module_roots` in `.polint.toml`; no repository-root `go.mod` is
required. In GitHub Actions, add this before `polint check` when using those
facts:

```yaml
      - uses: actions/setup-go@v6
        with:
          go-version: "1.25.x"
```

The action caches `.polint/cache` by default as two separate entries: the
source-validated artifacts under a key scoped to the polint version and the
resolved config/rule inputs, and the Cargo target directories under a key built
from compiler inputs (runner OS/architecture, resolved toolchain, compiler
flags, manifests, and lockfiles). A restored build cache can never reuse a stale
rule host, because the action removes each rule package's own output from the
entry before saving it — so Cargo has to rebuild the rule host from the sources
in the next checkout, and only dependency compilation is reused. Entries are
saved only after a run that finished. A fully cold first run can still pay
install, build, and analysis costs. See the
[GitHub Action guide](docs/GITHUB-ACTION.md) for inputs, cache keys, and
pinning options.

## Versions

| Component | Requirement |
|-----------|-------------|
| Rust (MSRV) | 1.95 |
| Go (symbol/reference sidecars) | 1.25+ on `PATH` |

### Minimum Rust version

polint's MSRV is **1.95** (workspace `rust-version` and root `rust-toolchain.toml`).
`polint init` writes a matching root `rust-toolchain.toml` when missing so
repo-local rule packs build with a compatible compiler.

## More

- [Examples](examples/)
- [Agent & CI playbook](docs/AGENT-PLAYBOOK.md)
- [Consumer setup / troubleshooting](docs/CONSUMER-SETUP.md)
- [GitHub Action](docs/GITHUB-ACTION.md)
- [Fact reference](docs/facts/)
- [Comment ignores](docs/IGNORE-COMMENTS.md)
- [Changed-files facts](docs/facts/changed-files.md)
- [Metric facts](docs/facts/metrics.md)
- [Go test facts](docs/facts/go-tests.md)
- [Analysis roadmap](docs/ANALYSIS-ROADMAP.md)
- [Release process](docs/RELEASING.md)

## License

[MIT](LICENSE)

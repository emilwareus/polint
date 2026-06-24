# polint

**AI-agent-native, shadcn-style linting for rules you own.**

polint turns repo-specific engineering instructions into executable lint rules.
AI agents do not always follow prose in `CLAUDE.md`, `AGENTS.md`, prompts, or
review comments. polint lets you encode the parts that are actually analyzable.

Think shadcn, but for linting: you own the rule code in your repository; polint
brings the scaffolding and infrastructure to create, run, test, and ship it.

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

`polint init` creates `.polint.toml`, `.polint/rules/src/`, `.polint/cache/`, `.polint/output/`, `.polint/.gitignore` (ignoring `cache/` and `output/`), and root `rust-toolchain.toml` when missing (see [Minimum Rust version](#minimum-rust-version)).
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

## Review rules (diff-gated)

`polint review <ref>` is `polint check` with the **identical rule-as-code setup**,
gated so a rule fires only against a **diff to a target branch or commit**:

```bash
polint review origin/main      # diff HEAD against the merge-base with origin/main
polint review <commit-sha>     # diff against a specific commit
polint review <base>...<head>  # an explicit three-dot range
```

Review rules are normal `#[polint::rule]` Rust functions that use the full SDK and
analysis engine. The only differences from a check rule are the `kind = "review"`
designation and an optional `ChangedFiles<'_>` parameter that exposes the diff (the
changed paths, their status, and the changed line ranges) as a typed fact view:

```rust
#[polint::rule(id = "review/migrations", description = "Migrations changed.",
               severity = "warn", kind = "review")]
fn migrations(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    for changed in changes.iter() {
        if changed.matches_glob("db/migrations/**") {
            let line = changed.lines().first().map(|&(lo, _)| lo).unwrap_or(1);
            ctx.report(Diagnostic::warning(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(line, 1),
                "A DB owner must review this migration.",
            ));
        }
    }
    Ok(())
}
```

By default `polint review` surfaces only diagnostics that intersect the diff — both
the changed file and (unless `--whole-file`) the changed line ranges — so *any* rule
is "check, but only on the diff" for free. Use `--no-diff-gate` to surface every
review finding regardless of the diff, or `--whole-file` to gate on changed files
only. Review rules are inert under `polint check` (which runs only check-kind rules).

Scaffold one with `polint new-rule generic <name> --review`, which generates a
`kind = "review"` rule with a `ChangedFiles<'_>` parameter (review rules are
exercised with `polint review`, so no static check fixtures are generated). See
`examples/review-rules/` for a simple path-watcher and a complex rule that restricts
real symbol/reference analysis to changed code. Review-rule heuristics are
heuristic — they are repo-local policy, not exact analysis.

`ChangedFiles<'_>` is documented in [`docs/facts/changed-files.md`](docs/facts/changed-files.md).

## Cache

polint keeps local, untracked cache data under `.polint/cache` by default:

```text
.polint/cache/
  analysis/       compact parser/fact JSON artifacts
  rules-target/   Cargo target dir for repo-local rule hosts
  derived/        reserved for future project-level derived facts
```

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
to another Cargo profile name to use `cargo run --profile <name>`.

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

## Machine contract (JSON)

Stable JSON reports (`polint check --format json`) match the schema at
[docs/schemas/polint-report-v1.json](docs/schemas/polint-report-v1.json). Emitters
also set a top-level `schema` URL when using current polint. Diagnostics are
deduplicated and sorted deterministically; `--only-rule` and `--max-diagnostics`
apply after that pipeline for emitted reports. `--max-diagnostics` does not hide
failures from `--fail-on` (see [docs/AGENT-PLAYBOOK.md](docs/AGENT-PLAYBOOK.md)).

AI-friendly saved reports (`polint check --format ai-friendly`) match
[docs/schemas/polint-ai-friendly-v1.json](docs/schemas/polint-ai-friendly-v1.json).
They contain `summary`, `examples`, and `diagnostics`; stdout intentionally stays
small to avoid overloading coding-agent context.

## Minimum Rust version

Rule packs under `.polint/rules` are normal Rust crates that depend on the **`polint` library**. The published crate declares **`rust-version = "1.95"`** (MSRV). Cargo refuses to build the rule pack if the **active `rustc`** is older, even when the stub uses `edition = "2024"`.

- **`polint init`** writes **`rust-toolchain.toml` at the repository root** only when that file does **not** already exist, aligning the default toolchain with polint’s MSRV so `polint check` (which runs `cargo` with `--manifest-path .polint/rules/Cargo.toml`) succeeds.
- If your repo already pins an older toolchain, **raise `channel`** in `rust-toolchain.toml` to **1.95** or newer, or run with an override, for example:
  `RUSTUP_TOOLCHAIN=1.95 polint check`

When the rules crate fails for this reason, the CLI adds a short note on top of Cargo’s stderr.

**Semver:** generated `Cargo.toml` uses `polint = "0.1.x"` (caret). Patch updates within **0.1** are accepted automatically; a **0.2** release requires updating that dependency line.

This repository pins Rust **1.95** in [`rust-toolchain.toml`](rust-toolchain.toml) for developing polint itself.

## Versions

| Item | Where it is defined |
|------|---------------------|
| CLI and `polint` crate version | Workspace `version` in the repo root `Cargo.toml` |
| Published crate | `polint` on crates.io |
| Minimum supported Rust | `rust-version` in workspace `Cargo.toml` |
| Generated rule packs | Rust edition **2024** (`polint new-rule` template) |

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
          version: latest
          args: check --format github
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

The action caches `.polint/cache` by default, including the repo-local
rule-host Cargo target directory at `.polint/cache/rules-target`. A fully cold
first run can still pay install, build, and analysis costs; repeat runs with the
same relevant inputs should restore those caches. See the
[GitHub Action guide](docs/GITHUB-ACTION.md) for inputs, cache keys, and
pinning options.

## More

- [Examples](examples/)
- [Agent & CI playbook](docs/AGENT-PLAYBOOK.md)
- [Consumer setup / troubleshooting](docs/CONSUMER-SETUP.md)
- [GitHub Action](docs/GITHUB-ACTION.md)
- [Comment ignores](docs/IGNORE-COMMENTS.md)
- [Metric facts](docs/facts/metrics.md)
- [Go test facts](docs/facts/go-tests.md)
- [Analysis roadmap](docs/ANALYSIS-ROADMAP.md)
- [Release process](docs/RELEASING.md)

## License

[MIT](LICENSE)

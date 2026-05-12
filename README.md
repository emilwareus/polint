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
polint check
```

`polint init` creates `.polint.toml`, `.polint/rules/src/`, `.polint/cache/`, `.polint/.gitignore` (ignoring `cache/`), and root `rust-toolchain.toml` when missing (see [Minimum Rust version](#minimum-rust-version)).
`polint new-rule <go|ts|js|generic> <name>` adds a Rust rule module to your
local rule pack. `polint check` discovers and runs that rule pack.

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
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install polint --locked
      - run: polint check --format sarif > polint.sarif
```

## More

- [Examples](examples/)
- [Agent & CI playbook](docs/AGENT-PLAYBOOK.md)
- [Consumer setup / troubleshooting](docs/CONSUMER-SETUP.md)
- [Comment ignores](docs/IGNORE-COMMENTS.md)
- [Metric facts](docs/facts/metrics.md)
- [Go test facts](docs/facts/go-tests.md)
- [Analysis roadmap](docs/ANALYSIS-ROADMAP.md)
- [Release process](docs/RELEASING.md)

## License

[MIT](LICENSE)

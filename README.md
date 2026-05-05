# polint

polint is a Rust framework for repo-local static-analysis rules. It is built for engineering policies that are specific to one codebase: architecture boundaries, branch-test expectations, design-token usage, test maintainability, or conventions that are too local for generic linters.

It is not a replacement for ESLint, Biome, Ruff, golangci-lint, rustfmt, gofmt, or formatters. Keep using those tools. Use polint when the policy is local to your repository and should be executable instead of repeated in prompts.

## Why This Exists

AI-assisted coding often violates subtle local conventions. Prompting the agent again and again is unreliable. Encoding those expectations as repo-local rules gives the team a repeatable check that works locally, in CI, and in agent workflows.

Rules are code. The framework provides file discovery, Go and TypeScript/JavaScript parsing, reusable facts, diagnostics, rule testing, profiling, caching, graph output, and CI output.

## Installation

### From crates.io

With a Rust toolchain installed:

```bash
cargo install polint-cli --locked
```

This installs the `polint` binary into `~/.cargo/bin` (or the default Cargo install root).

### Prebuilt binary (GitHub Releases)

Maintainers ship **versioned** archives on each semver GitHub Release **`vX.Y.Z`** (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64 as a `.tar.gz` containing `polint` or `polint.exe`) via the **Release** workflow. **Unix:** the install script downloads from **`releases/latest`** over HTTPS, verifies SHA-256, and installs to `~/.local/bin` by default:

```bash
curl -sSfL https://raw.githubusercontent.com/emilwareus/exlint/main/scripts/install.sh | bash
```

**Windows:** use `cargo install polint-cli --locked`, or download `polint-windows-x86_64.tar.gz` from the [latest release](https://github.com/emilwareus/exlint/releases/latest), extract `polint.exe`, and place it on your `PATH`.

Override the install directory:

```bash
curl -sSfL https://raw.githubusercontent.com/emilwareus/exlint/main/scripts/install.sh | POLINT_INSTALL_DIR=/usr/local/bin bash
```

Use another GitHub repo or release tag (for example a fork or a specific **`vX.Y.Z`**):

```bash
curl -sSfL https://raw.githubusercontent.com/yourfork/exlint/main/scripts/install.sh \
  | POLINT_REPO=yourfork/exlint POLINT_RELEASE_TAG=v0.2.0 bash
```

You need `curl` or `wget`, plus `shasum` or `sha256sum`, for checksum verification.

### From a local checkout

```bash
make install
```

This installs from `crates/polint-cli` using Cargo and the checked-in lockfile, normally into `~/.cargo/bin`.

## Try to use it!

Install polint (see above), clone the repository, and run one self-contained example:

```bash
git clone https://github.com/emilwareus/exlint.git polint
cd polint/examples/config-denied-literal

polint --version
polint check --fail-on none
```

`polint check` discovers and runs the local rule host at `.polint/rules/`
(`Cargo.toml` + `src/main.rs`) for you. That is intentional: polint ships no
built-in policy rules, so examples bring their own repo-local rule code.

Human diagnostics use ANSI colors when stdout is a TTY and `NO_COLOR` is unset. For plain text (e.g. pasting into docs), run `polint check --fail-on none --color never`.

After install and clone, the final two commands should print (version line first; diagnostic layout like this when using `--color never`):

```text
polint 0.1.0
error[local/no-denied-literals]: Configured denied literal `legacy-testid` found.
  --> query.ts:4:25-4:40
  evidence literal: legacy-testid
  evidence matched: legacy-testid
  help: Replace the literal with an allowed constant or local abstraction.
  fingerprint: e337fbb73d44b2b7


```

## Quickstart

```bash
polint init
polint add-skill
polint new-rule go require-payment-error-tests
polint check --fail-on none
polint check src --fail-on none
```

`polint init` creates:

```text
.polint.toml
.polint/
  rules/
```

`polint new-rule go my-policy` adds `src/my_policy.rs` to the rule pack under
`.polint/rules/` and registers it from `src/main.rs` (creating the pack manifest
when needed).

If config is missing, `polint check` uses normal workspace defaults, no
profiles, and `.polint/rules` for local rule discovery. If no local rules are
present, it only runs parser/fact extraction and suggests `polint init`.

## AI Agent Skills

Install repo-local skill instructions so an AI coding agent knows how to use
polint and how to write local rules for the current repository:

```bash
polint add-skill
```

The command is interactive by default and lets you choose Claude Code, Codex, or
both. For automation:

```bash
polint add-skill --agent claude
polint add-skill --agent codex
polint add-skill --all
```

Claude installs to `.claude/skills/polint/SKILL.md`. Codex installs to
`.agents/skills/polint/SKILL.md` by default, or to `.codex/skills/polint/SKILL.md`
when that folder already exists in the repository. Existing skills are preserved
unless you pass `--force`.

## Example Config

```toml
[workspace]
include = ["internal/**", "apps/web/**"]
exclude = ["**/vendor/**", "**/node_modules/**", "**/*.pb.go"]

[rules]
paths = [".polint/rules"]

[[rules.config]]
id = "custom/no-raw-brand-colors"
severity = "error"
files = ["apps/web/**/*.{ts,tsx}"]
allow_files = ["apps/web/src/theme/**", "apps/web/src/design-tokens/**"]

[profiles.web]
rules = ["custom/no-raw-brand-colors"]
```

Profiles are optional named subsets. `polint check` with no `--profile` runs
every discovered rule. `polint check --profile web` runs only `[profiles.web]`;
an unknown profile is an error. Profile names are arbitrary.

## No Built-In Policy Rules

polint intentionally ships no built-in policy rules. It provides the host
infrastructure: discovery, parsing, facts, diagnostics, rule execution, config,
CI output, graph output, cache, and SDK types.

The copyable rules under `examples/*/.polint/rules/` are SDK dogfood examples,
not product defaults. Each example is shaped like a separate repository: it has
its own fixture source, `.polint.toml`, and one local Rust rule package
(`Cargo.toml` at `.polint/rules/` with one module file per rule under `src/`).

Heuristic rules say so in diagnostics. For example, Go branch-obligation diagnostics report "No nearby test evidence found"; they do not claim exact coverage.

## Analysis roadmap

polint is a **facts-first** analyzer: rules read stable extractions (literals, imports, functions, branches, tests, …) from Go and TS/JS sources today, with room to grow into a deeper analysis engine. The table lists **shipped** scenarios (with example repos that prove them) and **planned** work in rough dependency order.

| Status | Scenario | Notes | Examples |
|--------|----------|-------|----------|
| Shipped | TS/JS string & template literals | Span-backed literal facts | [basic](examples/basic/README.md), [ts-design-tokens](examples/ts-design-tokens/README.md), [config-denied-literal](examples/config-denied-literal/README.md) (Go + TS/JS), [custom-rule-ts](examples/custom-rule-ts/README.md) |
| Shipped | JSX / TSX attributes | Name / value facts | [basic](examples/basic/README.md), [ts-design-tokens](examples/ts-design-tokens/README.md), [custom-rule-ts](examples/custom-rule-ts/README.md), [multiple-rules](examples/multiple-rules/README.md) |
| Shipped | Config-driven deny lists | `[[rules.config]]` → `deny`, globs | [config-denied-literal](examples/config-denied-literal/README.md) |
| Shipped | Go import paths & boundaries | Import facts + `forbidden_imports` | [go-import-boundaries](examples/go-import-boundaries/README.md), [multiple-rules](examples/multiple-rules/README.md) |
| Shipped | Cyclomatic complexity (Go) | Per-function metric | [go-complexity](examples/go-complexity/README.md) |
| Shipped | Cyclomatic complexity (TS/JS) | Per-function metric | [ts-complexity](examples/ts-complexity/README.md) |
| Shipped | Go branch / error-path obligations | Heuristic branch facts | [go-branch-obligations](examples/go-branch-obligations/README.md) |
| Shipped | Go branch policy + test evidence | Branches + test facts (heuristic) | [custom-rule-go](examples/custom-rule-go/README.md) |
| Shipped | Go test maintainability | Test facts, assertions, thresholds via config | [go-test-quality](examples/go-test-quality/README.md) |
| Shipped | Several rules in one pack | One `.polint/rules/Cargo.toml`, module per rule | [multiple-rules](examples/multiple-rules/README.md) |
| Shipped | Minimal TSX starter | Single rule, single diagnostic | [basic](examples/basic/README.md) |
| Shipped | CLI: JSON/SARIF, cache, profiling, graph | No dedicated example per subcommand | `polint --help` |
| Planned | Scope-accurate module resolution | Path mapping, package exports, build tags / conditions | — |
| Planned | Symbol / binding resolution | Definitions, references, re-exports; stable symbol IDs | — |
| Planned | Type-aware analysis | TS semantic layer; Go `go/types` (or equivalent); syntax vs typed rule tiers | — |
| Planned | General intra-procedural CFG | First-class per-function graph, not only branch heuristics | — |
| Planned | Dataflow | Def-use / SSA-style IR; value propagation where types exist | — |
| Planned | Resolved call graph | Caller → callee symbols; approximate virtual/dynamic dispatch | — |
| Planned | Interprocedural analysis | Summaries; whole-program or scoped modes; finer-grained invalidation | — |
| Planned | Taint / source–sink tracking | On top of dataflow + configurable sources/sinks | — |
| Planned | Alias / points-to (conservative) | Stronger security-style rules when needed | — |
| Planned | Higher-level rule API | Composable queries, stability guarantees, richer diagnostics provenance | — |

Heuristic and future typed rules should **state their precision tier** in messaging so teams know what they are enforcing.

## Rule Authoring

Generated rules use the same SDK shape as the example rules. They are runnable
local rule hosts that `polint check` can discover under `.polint/rules`:

```rust
use polint_sdk::prelude::*;
use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(RequirePaymentErrorTests)])
}

pub struct RequirePaymentErrorTests;

impl Rule for RequirePaymentErrorTests {
    fn meta(&self) -> RuleMeta {
        RuleMeta {
            id: "custom/require-payment-error-tests".to_string(),
            description: "Require payment error branches to have test evidence.".to_string(),
            severity: Severity::Error,
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new().go_tests().branch_obligations()
    }

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {
        for function in ctx.functions() {
            for obligation in ctx.branch_obligations(function.id) {
                if obligation.condition_text.contains("err != nil") {
                    ctx.warn(&obligation.decision_span, "Check this error branch has test evidence");
                }
            }
        }
        Ok(())
    }
}
```

Repo-local Rust rules are compiled and executed as local Cargo rule hosts for
now. `polint check` hides that Cargo invocation from normal use, but Rust and
Cargo still need to be available when a local rule host has not already been
built.

## Capabilities

Rules declare the facts they need so the engine computes only relevant analysis:

```rust
fn capabilities(&self) -> Capabilities {
    Capabilities::new()
        .imports()
        .functions()
        .go_tests()
        .branch_obligations()
}
```

Capabilities cover facts such as files, functions, imports, Go tests, branch obligations, TypeScript components/classes, string literals, JSX attributes, and graph-oriented import/call facts where available.

## Testing Rules

Run any example to see SDK rules execute against real fixtures:

```bash
cd examples/go-complexity
polint check --format json --fail-on none
```

For a repo with multiple local policies, use one rule-pack crate and register
the rules together. `polint check` discovers the rule-pack manifest:

```bash
cd examples/multiple-rules
polint check --format json --fail-on none
```

Use `--fail-on warn|error|none` to control CI status. Exit codes are:

- `0`: no diagnostics at or above the fail threshold
- `1`: diagnostics at or above the fail threshold
- `2`: fatal tool/config/internal error

`--format json` writes a single JSON object (not a bare array): `version` (currently `1`), `tool` (`name`, `version`, from the emitting binary), and `diagnostics` in the same schema as each `Diagnostic` struct. Repo-local rule hosts emit this shape when run with `--format json`. Parse it with `diagnostics_from_json_report` in `polint-diagnostics` or read the `diagnostics` field in your own tooling.

For `--format human`, use `--color auto` (default), `always`, or `never`. Colors follow the common `NO_COLOR` convention when `auto` is selected.

`--format sarif` produces SARIF 2.1 with primary locations, `relatedLocations` for multi-span labels, `fixes` when a replacement text is set, stable fingerprints in `fingerprints.polint/v1`, and stub `rules` entries derived from result `ruleId`s. The log is still intentionally incremental and is not claimed as fully SARIF-certified.

## Examples

The top-level `examples/` directory contains copyable examples:

- `examples/basic` - smallest runnable TSX raw-color example with `local/no-raw-colors`.
- `examples/config-denied-literal` - configured denied string literal with `local/no-denied-literals`.
- `examples/custom-rule-go` - Go repo-local rule host with `local/require-error-branch-tests`.
- `examples/custom-rule-ts` - TypeScript/JS repo-local rule host with `local/no-product-hex-colors`.
- `examples/go-complexity` - Go cyclomatic complexity with `local/go-cyclomatic-complexity`.
- `examples/go-branch-obligations` - heuristic branch-test evidence with `local/go-branch-obligations`.
- `examples/go-import-boundaries` - configured Go import boundary with `local/go-import-boundaries`.
- `examples/go-test-quality` - heuristic Go test quality with `local/go-test-quality`.
- `examples/multiple-rules` - one local rule-pack crate registering both `local/no-raw-colors` and `local/go-import-boundaries`.
- `examples/ts-complexity` - TypeScript cyclomatic complexity with `local/ts-cyclomatic-complexity`.
- `examples/ts-design-tokens` - syntax-level raw color detection with `local/no-raw-colors`.

Run a fixture from that fixture directory:

```bash
polint check --format json --fail-on none
```

For `examples/multiple-rules`, the same command discovers the rule-pack
manifest:

```bash
cd examples/multiple-rules
polint check --format json --fail-on none
```

## CI

```yaml
name: polint

on: [push, pull_request]

jobs:
  polint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: make install
      - run: polint check --format sarif > polint.sarif
```

CI can run `polint check --format sarif` for parser diagnostics,
graph/fact smoke coverage, and any rules registered by a custom host. The CLI
has no bundled policy rules. SARIF output is incremental (subset of the spec); it is still not claimed as fully SARIF-certified.

## GitHub Actions

Workflows in `.github/workflows/`:

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push/PR to `main` | **`rustfmt`** on Ubuntu; on **Ubuntu, Windows, and macOS**: `clippy -D warnings`, full **`cargo test --workspace`**, then an **ignored** integration test that runs **`cargo install`** of `polint-cli` to a temp prefix and **`polint --version`** (models the crates.io install path) |
| `release.yml` | **Manual** (`workflow_dispatch` on `main`) | **Release only:** patch **`scripts/bump-workspace-version.py`**, commit and push to **`main`**, annotated tag **`vX.Y.Z`**, **crates.io** publish (optional), and **CLI archives** on that tag’s GitHub Release (both optional inputs default to on). Prebuilt installs use **`releases/latest`** (see **`scripts/install.sh`**) |

**Secrets (repository settings)**

| Secret | Required for | Notes |
|--------|----------------|-------|
| *(none)* | CI | `ci.yml` uses the default `GITHUB_TOKEN` only. |
| *(none)* | **`release.yml`** (typical) | `GITHUB_TOKEN` can push tags and manage releases if branch protection allows it. |
| `WORKFLOW_PUSH_TOKEN` | **`release.yml`** when **`main`** is protected | Optional PAT with **contents: write** (and permission to bypass or push to protected **`main`**) so the bump commit and tag push succeed. |
| `CRATES_IO_TOKEN` | **`release.yml`** with **Publish crates** on | [Create a token](https://crates.io/settings/tokens) on crates.io; add under **Secrets and variables → Actions**. Publish-scoped; never commit it. |

**Ship a version:** Run **Actions → Release → Run workflow** on **`main`**. That bumps the workspace **patch** version (via **`scripts/bump-workspace-version.py`**), pushes to **`main`**, creates **`vX.Y.Z`**, and—with both inputs left on—publishes to crates.io and uploads CLI archives to that GitHub Release. Turn off **Publish crates** or **Build CLI** if you only want bump+tag or only one publish step. To **smoke-test** the crates.io ordered publish without uploading, run **`DRY_RUN=1 ./scripts/publish-crates.sh`** locally first.

## Development

```bash
cargo fmt
cargo fmt -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
# Same release install path CI exercises (slow; compiles polint-cli in release):
cargo test -p polint-cli --test cargo_install_smoke --locked -- --ignored
```

Rust **1.94** is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); `dtolnay/rust-toolchain` picks it up in CI.

## Release Readiness

Before treating the workspace as release-ready, run:

```bash
cargo fmt -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Release readiness means the documented v1 behavior is implemented and verified. Semver-tagged releases and deeper analysis features can ship independently.

## License

This project is licensed under the [MIT License](LICENSE).

## Roadmap

Future work:

- Exact Go semantic sidecar through `go/packages` or `go/analysis`.
- Dynamic branch coverage instrumentation.
- More language adapters after Go and TypeScript/JavaScript stabilize.

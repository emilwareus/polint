# polint

polint is a Rust framework for repo-local static-analysis rules. It is built for engineering policies that are specific to one codebase: architecture boundaries, branch-test expectations, design-token usage, test maintainability, or conventions that are too local for generic linters.

It is not a replacement for ESLint, Biome, Ruff, golangci-lint, rustfmt, gofmt, or formatters. Keep using those tools. Use polint when the policy is local to your repository and should be executable instead of repeated in prompts.

## Why This Exists

AI-assisted coding often violates subtle local conventions. Prompting the agent again and again is unreliable. Encoding those expectations as repo-local rules gives the team a repeatable check that works locally, in CI, and in agent workflows.

Rules are code. The framework provides file discovery, Go and TypeScript/JavaScript parsing, reusable facts, diagnostics, rule testing, profiling, caching, graph output, CI output, and an experimental Wasm plugin boundary.

## Installation

Private main-branch install, using the GitHub CLI:

```bash
gh auth login
gh api --method GET -H "Accept: application/vnd.github.v3.raw+json" repos/emilwareus/exlint/contents/scripts/install.sh -f ref=main | bash
```

The installer downloads the latest `polint-main` release asset for your
OS/architecture, verifies its SHA-256 checksum, and installs `polint` to
`~/.local/bin` by default. Override the install directory with:

```bash
gh api --method GET -H "Accept: application/vnd.github.v3.raw+json" repos/emilwareus/exlint/contents/scripts/install.sh -f ref=main | POLINT_INSTALL_DIR=/usr/local/bin bash
```

Release assets are published by GitHub Actions only from pushes to `main`.
Because the repository is private for now, the installer requires `gh` to be
authenticated with access to `emilwareus/exlint`.

From a local checkout:

```bash
make install
```

This installs the `polint` binary from source using Cargo and the checked-in
lockfile, normally into `~/.cargo/bin`.

## Try to use it!

Install the latest private `main` build, clone the repository, and run one
self-contained example:

```bash
gh auth login
gh api --method GET -H "Accept: application/vnd.github.v3.raw+json" repos/emilwareus/exlint/contents/scripts/install.sh -f ref=main | bash

gh repo clone emilwareus/exlint polint
cd polint/examples/config-denied-literal

polint --version
polint check --fail-on none
```

`polint check` discovers and runs the local rule host in
`.polint/rules/no-denied-literals` for you. That is intentional: polint ships no
built-in policy rules, so examples bring their own repo-local rule code.

After the installer and clone messages, the final two commands should print:

```text
polint 0.1.0
query.ts:4:25-4:40 error local/no-denied-literals
  Configured denied literal `legacy-testid` found.
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

`polint new-rule go my-policy` creates a repo-local Rust rule skeleton under `.polint/rules/my-policy`.

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
its own fixture source, `.polint.toml`, and one local Rust rule crate.

Heuristic rules say so in diagnostics. For example, Go branch-obligation diagnostics report "No nearby test evidence found"; they do not claim exact coverage.

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
built. Automatic repo-local Wasm compilation/loading remains future work.

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

## Experimental Wasm Plugins

Repo-local Wasm rules are experimental. The `polint-plugin` crate currently provides the WIT rule interface, manifest validation, and optional Wasmtime component-byte validation behind the `wasmtime-host` feature.

`polint check` does not automatically compile, cache, or execute repo-local Wasm rules in v1. Rust rule hosts are supported through local Cargo manifests; the Wasm skeleton is a versionable contract for future sandboxed plugins, not a production plugin runtime.

Future plugins should query host-owned facts through stable IDs such as file IDs, function IDs, and branch IDs. Plugins should not receive full AST JSON, full source text, or large graph payloads.

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
has no bundled policy rules. The output is SARIF-like for v1 and intentionally
does not claim full SARIF certification.

## Development

```bash
cargo fmt
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Release Readiness

Before treating the workspace as release-ready, run:

```bash
cargo fmt -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Release readiness means the documented v1 behavior is implemented and verified. It does not mean crates.io publication, release tags, exact Go semantics, dynamic branch coverage, or automatic repo-local Wasm rule compilation are complete.

## Roadmap

Future work:

- Exact Go semantic sidecar through `go/packages` or `go/analysis`.
- Dynamic branch coverage instrumentation.
- Repo-local Wasm rule compilation and caching by source hash, SDK version, and target triple.
- More language adapters after Go and TypeScript/JavaScript stabilize.

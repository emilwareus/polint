# polint

polint is a Rust framework for repo-local static-analysis rules. It is built for engineering policies that are specific to one codebase: architecture boundaries, branch-test expectations, design-token usage, test maintainability, or conventions that are too local for generic linters.

It is not a replacement for ESLint, Biome, Ruff, golangci-lint, rustfmt, gofmt, or formatters. Keep using those tools. Use polint when the policy is local to your repository and should be executable instead of repeated in prompts.

## Why This Exists

AI-assisted coding often violates subtle local conventions. Prompting the agent again and again is unreliable. Encoding those expectations as repo-local rules gives the team a repeatable check that works locally, in CI, and in agent workflows.

Rules are code. The framework provides file discovery, Go and TypeScript/JavaScript parsing, reusable facts, diagnostics, rule testing, profiling, caching, graph output, CI output, and an experimental Wasm plugin boundary.

## Installation

From this repository:

```bash
cargo install --path crates/polint-cli
```

During development, run the CLI without installing it:

```bash
cargo run -p polint-cli -- check
```

## Quickstart

```bash
polint init
polint new-rule go require-payment-error-tests
polint check
polint check --profile full --format json
```

`polint init` creates:

```text
.polint.toml
.polint/
  rules/
```

`polint new-rule go my-policy` creates a repo-local Rust rule skeleton under `.polint/rules/my-policy`.

If config is missing, `polint check` still runs a minimal default and suggests `polint init`.

## Example Config

```toml
[workspace]
include = ["internal/**", "apps/web/**"]
exclude = ["**/vendor/**", "**/node_modules/**", "**/*.pb.go"]

[rules]
paths = [".polint/rules"]

[profiles.fast]
rules = ["custom/*", "examples/ts-no-raw-colors"]

[profiles.full]
rules = ["custom/*", "examples/*"]

[[rules.config]]
id = "examples/ts-no-raw-colors"
severity = "error"
files = ["apps/web/**/*.{ts,tsx}"]
allow_files = ["apps/web/src/theme/**", "apps/web/src/design-tokens/**"]
```

## Built-In Example Rules

The built-ins are SDK dogfood examples, not a comprehensive lint pack:

- `examples/go-cyclomatic-complexity`
- `examples/ts-cyclomatic-complexity`
- `examples/go-import-boundaries`
- `examples/ts-no-raw-colors`
- `examples/go-branch-obligations`
- `examples/go-test-suite-size`
- `examples/go-assertion-after-action`
- `examples/config-query-no-literal`

Heuristic rules say so in diagnostics. For example, Go branch-obligation diagnostics report "No nearby test evidence found"; they do not claim exact coverage.

## Rule Authoring

Generated rules use the same SDK shape as built-in rules:

```rust
use polint_sdk::prelude::*;

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

Generated repo-local Rust rules are scaffolded for authoring and testing, but they are not automatically compiled or dynamically loaded by `polint check` in v1. Native registration and the built-in example rules are the current executable path.

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

Use `polint test-rules` to run the same analysis path against fixtures:

```bash
polint test-rules --format json
polint test-rules --profile full --format json --fail-on none
```

Use `--fail-on warn|error|none` to control CI status. Exit codes are:

- `0`: no diagnostics at or above the fail threshold
- `1`: diagnostics at or above the fail threshold
- `2`: fatal tool/config/internal error

## Examples

The top-level `examples/` directory contains copyable examples:

- `examples/basic` - minimal init/check flow.
- `examples/custom-rule-go` - Go repo-local rule skeleton and SDK helper notes.
- `examples/custom-rule-ts` - TypeScript/JS rule skeleton and literal/JSX helper notes.
- `examples/go-branch-obligations` - heuristic branch-test evidence example.
- `examples/ts-design-tokens` - syntax-level raw color detection example.

Run examples with an installed binary or through Cargo:

```bash
polint check --profile fast --format json
cargo run -p polint-cli -- check --profile fast --format json
```

## Experimental Wasm Plugins

Repo-local Wasm rules are experimental. The `polint-plugin` crate currently provides the WIT rule interface, manifest validation, and optional Wasmtime component-byte validation behind the `wasmtime-host` feature.

`polint check` does not automatically compile, cache, or execute repo-local Wasm rules in v1. The current skeleton is a versionable contract for future sandboxed plugins, not a production plugin runtime.

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
      - run: cargo run -p polint-cli -- check --profile full --format sarif > polint.sarif
```

CI should prefer `polint check --profile full --format sarif` so the full rule profile runs and output can be uploaded or archived by the CI system. The output is SARIF-like for v1 and intentionally does not claim full SARIF certification.

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

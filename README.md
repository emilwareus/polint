# polint

polint is a Rust framework for repo-local static-analysis rules. It is built for engineering policies that are specific to one codebase: architecture boundaries, branch-test expectations, design-token usage, test maintainability, or conventions that are too local for generic linters.

It is not a replacement for ESLint, Biome, Ruff, golangci-lint, rustfmt, gofmt, or formatters. Keep using those tools. Use polint when the policy is local to your repository and should be executable instead of repeated in prompts.

## Why This Exists

AI-assisted coding often violates subtle local conventions. Prompting the agent again and again is unreliable. Encoding those expectations as repo-local rules gives the team a repeatable check that works locally, in CI, and in agent workflows.

Rules are code. The framework provides file discovery, parsing, facts, diagnostics, rule testing, profiling, caching, and CI output.

## Quickstart

```bash
cargo install --path crates/polint-cli

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

The built-ins are SDK examples, not a comprehensive ruleset:

- `examples/go-cyclomatic-complexity`
- `examples/ts-cyclomatic-complexity`
- `examples/go-import-boundaries`
- `examples/ts-no-raw-colors`
- `examples/go-branch-obligations`
- `examples/go-test-suite-size`
- `examples/go-assertion-after-action`
- `examples/config-query-no-literal`

Heuristic rules say so in diagnostics. For example, Go branch obligation diagnostics report "No nearby test evidence found"; they do not claim exact coverage.

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

Capabilities let the engine compute only the facts a rule asks for.

## Experimental Wasm plugins

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

Exit codes:

- `0`: no diagnostics at or above the fail threshold
- `1`: diagnostics at or above the fail threshold
- `2`: fatal tool/config/internal error

## Development

```bash
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Roadmap

- Exact Go semantic sidecar through `go/packages` or `go/analysis`
- Dynamic branch coverage instrumentation
- Repo-local Wasm rule compilation and caching
- Additional language adapters

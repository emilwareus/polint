# Multiple Rules Example

This example shows the recommended structure for a real project with several
repo-local policies: keep one rule-pack crate for the whole set of rules.

The local crate lives at `.polint/rules/Cargo.toml`. Its `src/main.rs` registers
both rules:

- `local/no-raw-colors`
- `local/go-import-boundaries`

Each rule still lives in its own Rust module, so the code stays easy to split
and review without creating a separate Cargo package per rule.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`Button.tsx` intentionally embeds a raw color:

```tsx
return <button data-color="#ff00aa">Pay</button>;
```

`handler.go` intentionally imports a forbidden package:

```go
import "net/http"
```

The expected output contains diagnostics from both rules. A real fix would move
the TSX color to a design token and move the Go HTTP dependency behind an
allowed package boundary.

## Why One Cargo.toml

Use one `Cargo.toml` for a group of rules when they belong to the same repo and
share the same owners, dependencies, and CI lifecycle. Use one Cargo package per
rule only when the rules need to be versioned, owned, or distributed separately.

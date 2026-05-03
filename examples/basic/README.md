# Basic Example

This is the smallest useful policy example: one TSX file, one rule, and one
diagnostic.

The policy is `local/no-raw-colors`. It catches hard-coded color literals in UI
code so a team can keep colors in design tokens or theme variables.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/no-raw-colors/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`Button.tsx` intentionally embeds a raw hex color:

```tsx
export function Button() {
  return <button data-color="#ff00aa">Pay</button>;
}
```

The expected finding is `local/no-raw-colors` on `#ff00aa`. A real fix would
replace that literal with a token such as `color.action.primary`.

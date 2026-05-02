# TypeScript Design Tokens Example

`examples/ts-design-tokens` is a TSX fixture for its local
`local/no-raw-colors` rule.

This directory is self-contained: the local rule implementation lives at
`.polint/rules/no-raw-colors/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/no-raw-colors/Cargo.toml -- check --profile fast --format json --fail-on none
```

`Button.tsx` intentionally includes raw color literals:

```tsx
const accent = "#ff00aa";
<button data-color="#00ff00" />
```

The rule detects syntax-level string and JSX color literals. It is useful for
catching obvious design-token violations, but it does not prove design-token
semantic correctness or validate that a replacement token exists.

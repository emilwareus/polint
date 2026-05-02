# TypeScript Design Tokens Example

`examples/ts-design-tokens` is a TSX fixture for the built-in
`examples/ts-no-raw-colors` rule.

Run it from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

`Button.tsx` intentionally includes raw color literals:

```tsx
const accent = "#ff00aa";
<button data-color="#00ff00" />
```

The rule detects syntax-level string and JSX color literals. It is useful for
catching obvious design-token violations, but it does not prove design-token
semantic correctness or validate that a replacement token exists.

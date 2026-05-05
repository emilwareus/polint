# TypeScript Design Tokens Example

This example models a design-system policy for TSX UI code.

The policy is `local/no-raw-colors`. It catches raw color literals in strings
and JSX attributes so contributors move colors into design tokens.

## Run It

From this directory:

```bash
polint check --format json --fail-on none
```

## What It Finds

`Button.tsx` intentionally includes raw color literals:

```tsx
const accent = "#ff00aa";
<button data-color="#00ff00" />
```

The expected finding is `local/no-raw-colors`. A real fix would replace both raw
colors with approved tokens. This is a syntax-level policy: it catches obvious
violations, but it does not prove that the replacement token exists or is
semantically correct.

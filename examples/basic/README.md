# Basic Example

Minimal TSX repository that runs one example policy against real source code.

`Button.tsx` intentionally uses a raw color literal:

```tsx
export function Button() {
  return <button data-color="#ff00aa">Pay</button>;
}
```

This directory is self-contained: the local rule implementation lives at
`.polint/rules/no-raw-colors/src/main.rs`.

Run it from this directory:

```bash
cargo run --manifest-path .polint/rules/no-raw-colors/Cargo.toml -- check --profile fast --format json --fail-on none
```

The example uses `local/no-raw-colors` because it is the smallest useful policy
to demonstrate: one TSX file, one configured local rule, one diagnostic.

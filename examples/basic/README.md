# Basic Example

Minimal TSX repository that runs one example policy against real source code.

`Button.tsx` intentionally uses a raw color literal:

```tsx
export function Button() {
  return <button data-color="#ff00aa">Pay</button>;
}
```

Run it from this directory:

```bash
cargo run --manifest-path ../rules/Cargo.toml -- check --profile fast --format json --fail-on none
```

The example uses `examples/ts-no-raw-colors` because it is the smallest useful
policy to demonstrate: one TSX file, one configured rule, one diagnostic.

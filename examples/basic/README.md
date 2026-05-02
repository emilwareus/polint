# Basic Example

Minimal TSX repository that runs one built-in policy against real source code.

`Button.tsx` intentionally uses a raw color literal:

```tsx
export function Button() {
  return <button data-color="#ff00aa">Pay</button>;
}
```

Run it from this directory:

```bash
polint check --profile fast --format json --fail-on none
```

From this repository during development, run the same commands through Cargo:

```bash
cargo run -p polint-cli -- check --profile fast --format json --fail-on none
```

The example uses `examples/ts-no-raw-colors` because it is the smallest useful
policy to demonstrate: one TSX file, one configured rule, one diagnostic.

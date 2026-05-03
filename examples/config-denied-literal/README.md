# Config Denied Literal Example

This example shows a policy driven by configuration instead of hard-coded rule
logic.

The policy is `local/no-denied-literals`. It reads the `deny` list from
`.polint.toml` and reports matching string literals.

## Run It

From this directory:

```bash
cargo run --manifest-path .polint/rules/no-denied-literals/Cargo.toml -- check --profile fast --format json --fail-on none
```

## What It Finds

`query.ts` intentionally contains `legacy-testid`, and the example config denies
that literal text:

```ts
export const selector = "legacy-testid";
```

The expected finding is `local/no-denied-literals`. A real fix would replace the
literal with an approved selector constant or remove the dependency on that
legacy value.

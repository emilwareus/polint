# TypeScript Complexity Example

This example models a simple maintainability policy for TypeScript and
JavaScript functions.

The policy is `local/ts-cyclomatic-complexity`. It reports functions whose
branch count exceeds the configured `max`.

## Run It

From this directory:

```bash
polint check --format json --fail-on none
```

## What It Finds

`label.ts` intentionally has two branches while the example config sets
`max = 1`:

```ts
if (admin) {
  return "admin";
}
if (status === "paid") {
  return "paid";
}
```

The expected finding is `local/ts-cyclomatic-complexity`. A real fix would split
the decision logic into smaller named helpers or adjust the threshold if the
team accepts the branch count.

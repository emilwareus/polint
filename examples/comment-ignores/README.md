# Comment Ignores Example

This example shows how `polint-ignore-*` comments suppress policy diagnostics
without changing rule code.

The local rule is `local/no-denied-literals`. It reports any TypeScript string
literal containing `legacy-testid`. `app.ts` contains two violations:

- one is intentionally ignored with
  `polint-ignore-next-line local/no-denied-literals -- ...`
- one remains visible in `polint check`

## Run It

From this directory:

```bash
polint check --format json --fail-on none
polint ignores --stat
polint ignores --format json --filter local/no-denied-literals
```

## What It Shows

`polint check` reports only the unignored literal. The rule still reports both
findings internally; polint applies the ignore layer after rule execution.

`polint ignores --stat` shows one active directive and one suppressed diagnostic.
Because `.polint.toml` sets `[ignores].require_reason = true`, removing the text
after `--` would make `polint check` emit `polint/ignore-missing-reason`.

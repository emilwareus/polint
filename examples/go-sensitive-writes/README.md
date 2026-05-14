# Go: Guard Sensitive Field Writes

This example models a backend package where only approved maintenance code may
write directly to sensitive account state.

The policy is `local/no-sensitive-balance-writes`. It uses `Symbols<'_>` and
`References<'_>` to find write and read-write references to the `Balance` field.
The rule allows `admin.go`, but reports writes in normal application code.

Run it with:

```bash
polint check --format json --no-cache --fail-on none
```

Run the command from this directory so polint loads this example's local rule
pack.

The expected findings are in `ledger.go`. `admin.go` shows the approved escape
hatch configured through `allow_files`.

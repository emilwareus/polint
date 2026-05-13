# TypeScript: Require Generated SDK API Calls

This example models a product codebase where service APIs should be called
through generated SDK clients instead of raw HTTP helpers.

The policy is `local/no-raw-api-calls`. It uses `Symbols<'_>` and
`References<'_>` to find calls to denied API entry points:

- `rawRequest` is a local helper, so polint can report resolved symbol/reference
  evidence for the call.
- `fetch` is a global in this tiny fixture, so polint reports it through the
  unresolved reference path instead of pretending it has an exact symbol.

Run it with:

```bash
polint check --format json --no-cache --fail-on none
```

Run the command from this directory so polint loads this example's local rule
pack.

The expected findings are in `src/user-page.ts`: one resolved call to the local
`rawRequest` helper and one unresolved global `fetch` call. The allowed pattern
is shown in `loadUserViaSdk`, which calls the generated `UsersSdk` client.

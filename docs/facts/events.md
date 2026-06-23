# Events Facts

`Events<'_>` is a preview SDK view for matching policy events. Requesting it
derives the supported `events` capability.

See [policy-queries.md](policy-queries.md) for the shared query-object style,
evidence header, precision/status vocabulary, unknown semantics, and template
starter workflow.

Call-event matching is syntax-first for performance on large repositories. When
no deeper policy view is requested, `Events<'_>` answers from syntax-derived
function call names and reports syntax precision with function-level ranges.
When another requested view already builds call/refined-call facts, Events
upgrades to those more precise call-site facts. The API remains preview because
additional event families, such as field writes and lifecycle events, will be
promoted in later phases.

```rust
#[polint::rule(id = "local/no-raw-dangerous-call", description = "Dangerous calls", severity = "error")]
pub(crate) fn no_raw_dangerous_call(ctx: &mut RuleCtx<'_>, events: Events<'_>) -> RuleResult {
    for violation in events.matching(EventPattern::call("dangerous_exec")) {
        ctx.report(violation.diagnostic(ctx.rule_id(), "dangerous call is not allowed"));
    }

    Ok(())
}
```

## Pattern Vocabulary

- `EventPattern::call("target")` matches one call target string. On the
  syntax-first path, exact names and member-property suffixes match, so
  `EventPattern::call("post")` can match `client.post(...)`. On the upgraded
  provider-backed path, Events also checks canonical labels such as symbol
  qualified names, symbol names, function names, synthetic targets, and
  syntactic callee labels.
- `EventPattern::write_field("field")` is preview vocabulary. It currently
  returns no provider-backed matches until write-event facts are promoted.

Results are returned as `PolicyViolation`s with the common policy evidence
header documented in [evidence.md](evidence.md), plus event-specific keys such
as `event`, `target`, `function`, `event_source`, `call_status`,
`call_precision`, and `confidence` when available. They do not expose raw AST,
MIR, CFG, solver, call-graph node, or provider IDs.

Matching intentionally starts with exact strings and explicit lists. It does not
include regex matching, closure filters, raw graph traversal, or parser-specific
selectors.

## Realistic Example

Use `Events<'_>` for cheap architectural policies where a direct call name is
the useful signal and whole-program reachability is unnecessary.

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-frontend-network",
    description = "Route frontend network calls through typed API clients.",
    severity = "error"
)]
pub(crate) fn no_raw_frontend_network(ctx: &mut RuleCtx<'_>, events: Events<'_>) -> RuleResult {
    for target in ["fetch", "post", "rawRequest"] {
        for violation in events.matching(EventPattern::call(target)) {
            ctx.report(violation.diagnostic(
                ctx.rule_id(),
                "route network calls through the generated API client",
            ));
        }
    }

    Ok(())
}
```

Analyzed application code:

```ts
import { usersApi } from "./generated/users-api";
import { rawRequest } from "./http";

export async function loadUserViaClient(id: string) {
  // Allowed by local convention: typed generated API client.
  return usersApi.getUser({ id });
}

export async function loadUserWithFetch(id: string) {
  // Violation: bypasses the generated client and calls the network directly.
  return fetch(`/api/users/${id}`);
}

export async function loadUserWithRawHelper(id: string) {
  // Violation: rawRequest is the local low-level HTTP helper.
  return rawRequest(`/api/users/${id}`);
}
```

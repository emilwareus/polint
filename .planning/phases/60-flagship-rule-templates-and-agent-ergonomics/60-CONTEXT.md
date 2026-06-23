# Phase 60 Context: Flagship Rule Templates and Agent Ergonomics

## Goal

Turn the v1.4 policy-query API into realistic scaffolds users and agents can
copy immediately, without presenting polint as a bundled ruleset.

## Requirements

- TPL-01: `polint new-rule` can generate a request-to-shell template using
  `DataFlow<'_>`, `FlowQuery`, `SourcePattern::http_request`,
  `SinkPattern::call`, and validation barriers.
- TPL-02: Generated templates cover secret-to-log and PII-to-analytics policies
  with explicit heuristic wording and redaction/barrier examples.
- TPL-03: Generated templates cover auth/validation-before-sensitive-write,
  transaction cleanup, and raw reachable API policies using `ControlFlow<'_>`
  and `Calls<'_>`.
- TPL-04: Generated templates cover SSRF, dangerous HTML sinks, unsafe
  deserialization from request data, and user-controlled file path policies
  using the same query-object style.
- TPL-05: README, examples, generated agent skill text, and docs show the
  flagship templates as repo-local policy examples without presenting polint as
  a bundled ruleset.

## Design Decision

Use a single optional selector on the existing command:

```bash
polint new-rule ts no-secret-logs --template secret-to-log
```

The default `polint new-rule <lang> <name>` behavior remains intact. The
template selector is a closed enum of public template IDs. Generated rules still
import only `polint::sdk::prelude::*`, request one typed policy view in the
signature, build one query object, and report each returned `PolicyViolation`.

## Constraints

- Do not add a second DSL, string query language, or separate template command.
- Do not invent backed public pattern constructors for Phase 60. Broader
  categories such as PII, SSRF, HTML, deserialization, analytics, and file paths
  should be honest scaffolds over current backed primitives: HTTP request
  sources, explicit secret-like name sources, exact call sinks, logger sinks,
  and call barriers.
- Fixtures must compile and, where the underlying Phase 58/59 facts are backed,
  produce expected diagnostics through `polint test --format json`.


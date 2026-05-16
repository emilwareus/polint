# Polint Rule SDK And Agent Authoring Design

## Current Strengths

Polint already has the right base direction:

- repo-local `.polint/rules` crates;
- `#[polint::rule]` functions;
- typed fact-view parameters;
- macro-derived capabilities;
- public examples using `polint::sdk::prelude::*`;
- capability diagnostics for unsupported/setup-missing facts;
- `RuleResult` for rule failures.

The next work is not to replace this. It is to make it inspectable, testable,
and powerful enough for agent-generated policies.

## Module/API Work

Recommended internal additions:

```text
crates/polint/src/rule_manifest.rs
crates/polint/src/rule_test/
  mod.rs
  case.rs
  fixture.rs
  markers.rs
  snapshots.rs
  runner.rs
  normalize.rs
crates/polint/src/cli/inspect.rs
crates/polint/src/cli/facts.rs
crates/polint/src/cli/test.rs
```

Potential SDK additions:

```text
crates/polint/src/sdk/diagnostic.rs
crates/polint/src/sdk/options.rs
crates/polint/src/sdk/query/
  imports.rs
  modules.rs
  symbols.rs
  references.rs
  future_calls.rs
  future_flow.rs
```

Keep unstable future modules hidden or preview-gated.

## Rule Manifest CLI

Add:

```text
polint inspect rule <rule-id> --format json
```

Output:

```json
{
  "id": "local/no-raw-colors",
  "description": "Raw color literals must use design tokens",
  "severity": "warning",
  "stability": "preview",
  "sdk_version": "0.1.0",
  "fact_views": ["StringLiterals"],
  "capabilities": [
    {
      "name": "literals",
      "precision": "syntactic",
      "required": true
    }
  ],
  "options_schema": null,
  "fixability": "none",
  "limitations": []
}
```

The manifest should be available even if analysis setup fails.

## Facts And Unknowns CLI

Add:

```text
polint facts list --format json
polint facts sample --cap imports --path src/app.ts --limit 50 --format json
polint unknowns --cap calls --format json
```

Agents need bounded samples. Never dump every fact by default.

Fact output should include:

- stable ID;
- source span;
- fact family;
- precision;
- status;
- provenance;
- relevant fields.

Unknown output should include:

- unknown kind;
- source span;
- reason;
- affected capability;
- suggested artifact type;
- related setup diagnostics if any.

## `polint test` Implementation

Case manifest:

```toml
kind = "rule"
rule = "local/no-raw-colors"
expect = "inline-and-snapshot"
format = "json"

[fixture]
root = "."

[config]
# optional `.polint.toml` overlay
```

Marker parser should support line comments for Go, TS, JS, Python, Java, and
block comments later:

```text
polint-expect <rule-id>: <regex>
polint-ok <rule-id>
polint-todo-expect <rule-id>: <regex>
polint-todo-ok <rule-id>
```

Snapshot normalization:

- sort diagnostics deterministically;
- normalize temp paths;
- include rule id, severity, message, primary span, related spans, evidence
  summary, capability diagnostics, and fix applicability;
- omit unstable runtime/cache stats unless explicitly requested.

## Scaffolding

`polint new-rule ts no-raw-colors` should generate:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-raw-colors",
    description = "Raw color literals must use design tokens",
    severity = "warning",
    stability = "preview"
)]
fn no_raw_colors(ctx: &mut RuleCtx<'_>, literals: StringLiterals<'_>) -> RuleResult {
    for literal in literals.iter() {
        if literal.text.starts_with("#") {
            ctx.diagnostic(literal.span.clone(), "raw color literal")
                .emit();
        }
    }
    Ok(())
}
```

And one fixture with expected/ok markers.

## Query Builder Rules

Query builders should:

- live on fact views;
- return iterators or bounded query result handles;
- carry precision/status where relevant;
- require explicit limits for path queries;
- avoid cloning source strings;
- never expose parser ASTs.

Example:

```rust
for edge in modules.import_edges().from_layer("ui").to_layer("server") {
    ctx.diagnostic(edge.span(), "UI imports server layer").emit();
}
```

Future:

```rust
let sink = calls.named("exec.Command").argument(0);
for path in flow.sources(SourceKind::UserInput).to(sink).paths().limit(5) {
    ctx.diagnostic(path.sink_span(), "user input reaches command execution")
        .with_evidence_path(path)
        .emit();
}
```

## Model Pack Implementation

Model pack manifest:

```toml
name = "local/acme-http"
version = "0.1.0"
targets = { "polint/data-flow" = "^0.1" }
models = ["models/**/*.toml"]
```

Example model:

```toml
[[source]]
language = "ts"
symbol = "Request.query"
access_path = "return"
kind = "user-input"
precision = "modeled"
provenance = "agent-generated"

[[sink]]
language = "go"
symbol = "os/exec.Command"
access_path = "arg[0]"
kind = "process-exec"
precision = "modeled"
provenance = "handwritten"
```

Model rows should become validated candidate facts, not direct facts.

## Provider Extension Implementation

Provider extensions should use a process protocol:

```text
host -> extension: handshake
host -> extension: input snapshot and requested fact families
extension -> host: candidate facts + provenance + dependencies
host: validates and merges
```

Do not run provider extensions in-process first.

## Public Documentation Gates

Do not promote a new fact view or query builder until:

- it has a docs page under `docs/facts/`;
- it states precision and limitations;
- `#[polint::rule]` can request it;
- unsupported/setup-missing behavior is tested;
- a temp-repo style outside-user test exists;
- `polint inspect rule` reports the capability.

## Implementation Order

1. Rule manifest data type and macro output.
2. `polint inspect rule`.
3. `polint facts list` and `polint capabilities`.
4. `polint test` fixture runner.
5. Improve `polint new-rule` to generate fixtures.
6. Diagnostic builder/message IDs.
7. Query helpers on existing fact views.
8. Preview gates for new views/builders.
9. Model pack parser/validator.
10. Provider test runner.
11. Structured fixes.

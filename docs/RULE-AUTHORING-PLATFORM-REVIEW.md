# Rule Authoring Platform Review

This review checks whether polint is really giving users the building blocks
needed to write their own repo-local rules, or whether the examples only work
because they are close to polint's internal code.

## Bottom Line

The examples are mostly clean at the Rust import boundary: they use
`polint::sdk::prelude::*` and `polint::runner::run_cli`, not private parser or
core adapter functions.

But the current proof is not strong enough yet. The examples are checked into
the polint workspace, use workspace dependencies, and fit the current fixed
configuration shapes. That means they prove "our examples work here" more than
they prove "a normal user can build new rules from public primitives."

The main risk is not that examples call hidden internals. The main risk is that
the public rule-authoring contract is still under-proven and partly shaped around
the examples.

## Fix Status

This branch has started addressing the review:

- Added `#[polint::rule]` and typed fact views so normal rule capabilities are
  derived from function parameters instead of handwritten declarations.
- Added `RuleOptions::settings` so arbitrary `[[rules.config]]` fields reach
  repo-local rules.
- Added a temp-repo integration test proving a generated external rule can use
  SDK facts, settings, and diagnostics through `polint check`.
- Added cache digest coverage so custom rule settings, newer config sections,
  and ambiguous string-list boundaries invalidate deterministic hashes.
- Added fact reference docs under `docs/facts/`.
- Updated AGENTS guidance so future work treats rule packs as external SDK
  consumers.
- Made capability docs clearer that the current host may harvest a superset and
  that unavailable fact families must not be advertised as provided.

## What Good Looks Like

A repo-local rule should be able to live in a user's repository under
`.polint/rules`, depend only on the published `polint` crate, import only:

```rust
use polint::sdk::prelude::*;
```

and register itself through:

```rust
polint::runner::run_cli(vec![...])
```

From there, the user should have enough raw facts and helpers to build policies
that are not already represented by the example rules.

## Finding 1: The Examples Are SDK-Clean, But The Tests Are Workspace-Coupled

### Plain Statement

The example rule code uses the right public API, but the way we test examples is
too close to the polint repository. We are not yet proving that an outside user
can build the same kind of rule in their own repo.

### Evidence

The public crate root intentionally exposes only `sdk`, `runner`, `run_main`,
and the internal benchmark facade:

- [`crates/polint/src/lib.rs`](../crates/polint/src/lib.rs#L5-L21)

The SDK prelude explicitly re-exports the intended rule-authoring surface:

- [`crates/polint/src/sdk/mod.rs`](../crates/polint/src/sdk/mod.rs#L26-L40)

Example rules import the SDK prelude, for example:

- [`examples/ts-design-tokens/.polint/rules/src/no_raw_colors.rs`](../examples/ts-design-tokens/.polint/rules/src/no_raw_colors.rs)
- [`examples/go-branch-obligations/.polint/rules/src/go_branch_obligations.rs`](../examples/go-branch-obligations/.polint/rules/src/go_branch_obligations.rs)
- [`examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs`](../examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs)

That part is good.

The weak part is the manifests. The checked-in examples are workspace members:

- [`Cargo.toml`](../Cargo.toml#L1-L15)

and their rule crates use workspace inheritance:

```toml
version.workspace = true
edition.workspace = true
polint = { workspace = true }
lints.workspace = true
```

Example:

- [`examples/basic/.polint/rules/Cargo.toml`](../examples/basic/.polint/rules/Cargo.toml#L1-L11)

The integration test helper runs those example manifests directly from the
polint repository:

- [`example_rule_cmd`](../crates/polint/tests/common/mod.rs#L99-L110)

### Why This Matters

This can hide problems that users would hit outside the workspace:

- dependency resolution with `polint = "0.1.x"`
- missing workspace metadata
- missing workspace lints
- generated manifest behavior
- published-crate API visibility
- rule-pack compilation from a normal consumer repo

### Fix Direction

Add an integration test that creates a temporary user repo, runs `polint init`,
runs `polint new-rule`, writes a real rule using only `polint::sdk::prelude::*`,
and then runs `polint check`.

That rule should:

- consume real facts from `RuleCtx`
- read at least one config option
- emit a diagnostic
- compile outside the polint workspace

This is the most important proof to add.

## Finding 2: Rule Configuration Is Too Example-Shaped

### Plain Statement

Rules can only read a small fixed set of config fields. That works for the
current examples, but it is not enough for arbitrary user policies.

### Evidence

`RuleOptions` currently exposes only these fixed fields:

- `severity`
- `files`
- `allow_files`
- `allow`
- `max`
- `deny`
- `forbidden_imports`

Code:

- [`RuleOptions`](../crates/polint/src/core/mod.rs#L690-L699)

The runner maps config into that fixed shape:

- [`rule_options_from_config`](../crates/polint/src/runner/mod.rs#L222-L235)

The examples fit this shape well:

- complexity rules use `max`
- denied literal rules use `deny` and `allow`
- import boundary rules use `forbidden_imports`
- path scoping uses `files` and `allow_files`

Examples:

- [`ts_complexity.rs`](../examples/ts-complexity/.polint/rules/src/ts_complexity.rs#L21-L49)
- [`no_denied_literals.rs`](../examples/config-denied-literal/.polint/rules/src/no_denied_literals.rs#L25-L67)
- [`go_import_boundaries.rs`](../examples/go-import-boundaries/.polint/rules/src/go_import_boundaries.rs#L21-L56)

### Why This Matters

A real team may need rule-specific config like:

- required package prefixes
- allowed naming patterns
- service ownership maps
- architectural layer names
- file-to-file pairing names
- custom thresholds by path
- lists of APIs that require wrappers

Today, users must either hard-code those values in Rust or overload generic
fields like `allow`, `deny`, and `max`. That makes the SDK feel narrower than it
should.

### Fix Direction

Implemented direction: arbitrary rule-owned TOML fields are preserved in
`RuleOptions::settings` and documented in
[`docs/CONSUMER-SETUP.md`](CONSUMER-SETUP.md).

## Finding 3: Capabilities Over-Promise What They Actually Control

### Plain Statement

Rules declare capabilities, but the host does not use those capabilities to
decide what facts to collect. Some capability names also suggest facts that are
not really available yet.

### Evidence

`Capabilities` includes fields like:

- `syntax`
- `imports`
- `cfg`
- `call_graph`
- `coverage_facts`
- `test_suite_metrics`

Code:

- [`Capabilities`](../crates/polint/src/core/mod.rs#L593-L607)

But the runner always loads files and runs Go and TS analysis before running
rules:

- [`analyze_and_run`](../crates/polint/src/runner/mod.rs#L138-L176)

The Go adapter extracts a fixed set of facts unconditionally:

- [`parse_go_file`](../crates/polint/src/go/adapter.rs#L159-L184)

The TS adapter does the same for TS/JS facts:

- [`parse_ts_file`](../crates/polint/src/ts/adapter.rs#L159-L220)

### Why This Matters

The API says "declare what facts your rule needs," but the implementation mostly
treats that declaration as metadata. A user may believe that setting
`.coverage_facts()` or `.call_graph()` gives them those underlying models, when
that is not true in the same way as `.string_literals()` or `.imports()`.

### Fix Direction

Either:

- make capabilities real and use them to drive analysis, or
- rename/document them as descriptive metadata only.

Also split unavailable or placeholder capability names out of the public SDK
until they have real facts behind them.

## Finding 4: The SDK Has Real Primitives, But The Fact Semantics Need Better Docs

### Plain Statement

Users do get real underlying facts, but many fact fields are not explained well
enough. That forces users to learn by reading examples or adapter code.

### Evidence

`RuleCtx` exposes useful raw facts:

- files
- packages
- functions
- imports
- branch obligations
- Go tests
- TS components
- TS classes
- string literals
- JSX attributes
- path-context related paths

Code:

- [`RuleCtx` fact accessors](../crates/polint/src/core/mod.rs#L733-L940)

The fact structs are public and field-based:

- [`FunctionFact`, `ImportFact`, `BranchObligation`, `TestFact`, TS/JS facts](../crates/polint/src/core/mod.rs#L107-L190)

`TestFact` is documented clearly in:

- [`docs/facts/go-tests.md`](facts/go-tests.md)

But most other fact families do not have equivalent docs. For example, a user
has to infer the exact meaning and limits of:

- `FunctionFact.calls`
- `FunctionFact.cyclomatic_complexity`
- `ImportFact.package`
- `BranchObligation.edge_label`
- `BranchObligation.condition_text`
- `TsComponentFact`
- `TsClassFact.is_component_like`
- `StringLiteralFact.value`
- `JsxAttributeFact.value`

### Why This Matters

The product promise depends on users understanding the facts well enough to
compose new rules. If the examples are the only clear guide, users will copy the
examples instead of building their own policies confidently.

### Fix Direction

Implemented direction: fact documentation now lives under
[`docs/facts/`](facts/):

- [`docs/facts/functions.md`](facts/functions.md)
- [`docs/facts/imports.md`](facts/imports.md)
- [`docs/facts/branches.md`](facts/branches.md)
- [`docs/facts/ts-js.md`](facts/ts-js.md)
- [`docs/facts/literals.md`](facts/literals.md)

Each page should explain:

- what the fact means
- when it is produced
- what is heuristic
- what is not guaranteed
- one small rule example using the fact

## Finding 5: `polint new-rule` Generates A No-Op Rule

### Plain Statement

The generated rule skeleton compiles, but it does not show a user how to emit a
real diagnostic from real facts.

### Evidence

The template loops over facts and counts them, then discards the counts:

- [`rule_module_template`](../crates/polint/src/cli/mod.rs#L497-L550)

Generated Go rules count tests and branches:

```rust
for branch in branches.iter() {
    let related_test_count = tests.related_for_file(branch.file).len();
    let _ = related_test_count;
}
```

Generated TS rules count literals and JSX attributes:

```rust
let literal_count = literals.iter().count();
let attribute_count = jsx.iter().count();
let _ = (literal_count, attribute_count);
```

The smoke test proves only that `init`, `new-rule`, and `check` complete:

- [`init_new_rule_and_check_json_smoke`](../crates/polint/tests/cli.rs#L1352-L1373)

### Why This Matters

The first generated rule is a user's first contact with the SDK. A no-op rule
does not prove or teach the main product loop:

1. request typed fact views
2. inspect facts
3. apply repo-specific logic
4. report diagnostics
5. use config

### Fix Direction

Keep the generated rule safe, but make it more instructive. For example, generate
a commented diagnostic pattern or a disabled sample branch like:

```rust
// Example:
// if literal.value == "TODO" {
//     ctx.warn(&literal.span, "Replace TODO placeholder.");
// }
```

Even better, add a `polint new-rule --example` mode that creates a working sample
rule with a real diagnostic.

## Finding 6: External Rule-Pack Tests Are Too Shallow

### Plain Statement

Before the fix, tests checked that generated rules compiled and that repo
examples worked. They did not check that a generated external rule could use
real facts to report a real diagnostic.

### Evidence

The skeleton tests assert strings in generated source:

- [`new_rule_go_creates_sdk_oriented_skeleton`](../crates/polint/tests/cli.rs#L318-L341)
- [`new_rule_ts_creates_sdk_oriented_skeleton`](../crates/polint/tests/cli.rs#L343-L363)
- [`new_rule_generic_uses_sdk_query_helpers`](../crates/polint/tests/cli.rs#L365-L384)

For example, they check that generated modules contain:

- `use polint::sdk::prelude::*;`
- `tests.related_for_file(branch.file)`
- `literals.iter().count()`
- no `crate::core::`

The stronger integration tests use checked-in example rule crates through
`example_rule_cmd`:

- [`example_rule_cmd`](../crates/polint/tests/common/mod.rs#L99-L110)

### Why This Matters

String assertions are useful, but they do not prove the rule-author experience.
They can pass even if the generated external rule is not useful in practice.

### Fix Direction

Implemented direction: added a test named:

```rust
external_generated_rule_uses_sdk_facts_and_reports_diagnostic
```

The actual test name includes settings too:
`external_generated_rule_uses_sdk_facts_settings_and_reports_diagnostic`.

That test creates a temp repo and writes a rule that:

- imports only `polint::sdk::prelude::*`
- requests `StringLiterals<'_>` in a `#[polint::rule]` signature
- reads `literals.iter()`
- reports a diagnostic through `ctx.warn` or `ctx.report`
- runs through parent `polint check`
- asserts JSON output contains the diagnostic

This directly tests the product promise.

## Finding 7: Some SDK Helpers Are Canned, But Not Fatally So

### Plain Statement

The SDK includes a few convenience helpers, such as path scoping and related Go
test lookup. These are acceptable, but they should not become the only way to
write meaningful rules.

### Evidence

Path-scope helpers live in:

- [`sdk/scope.rs`](../crates/polint/src/sdk/scope.rs#L31-L79)

Go related-test lookup is exposed through:

- [`GoTests::related_for_file`](../crates/polint/src/sdk/facts.rs)
- [`collect_go_tests`](../crates/polint/src/sdk/mod.rs#L16-L19)

Examples use these helpers:

- [`examples/go-branch-obligations/.polint/rules/src/go_branch_obligations.rs`](../examples/go-branch-obligations/.polint/rules/src/go_branch_obligations.rs)
- [`examples/custom-rule-go/.polint/rules/src/require_error_branch_tests.rs`](../examples/custom-rule-go/.polint/rules/src/require_error_branch_tests.rs)

### Why This Matters

Helpers are good when they remove boilerplate. They become a problem if the SDK
only supports rules that look like the helpers. Users need both:

- raw facts for custom logic
- helpers for common patterns

### Fix Direction

Keep the helpers, but document them as optional convenience APIs. The fact docs
and external tests should show users building logic directly from raw facts too.

## Recommended Priority

1. Keep extending external temp-repo tests as new SDK facts are added.
2. Consider making capabilities drive analysis work, or keep documenting them as
   descriptive declarations.
3. Add more small rule examples to the new fact docs when new fact fields are
   promoted.
4. Consider adding `polint new-rule --example` for users who want a working
   sample diagnostic instead of a neutral skeleton.

## Release Readiness Note

This does not block the claim that polint has a real rule SDK. It does.

It previously blocked the stronger claim that the examples fully proved the SDK
as a general rule-authoring platform. The new external temp-repo test and
`RuleOptions::settings` close the biggest proof gap, while capability-driven
analysis remains future hardening.

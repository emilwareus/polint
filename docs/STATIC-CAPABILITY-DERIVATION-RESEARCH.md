# Static Capability Derivation Research

## Purpose

This document researches whether polint can avoid manual capability declarations
like this:

```rust
fn capabilities(&self) -> Capabilities {
    Capabilities::new().imports().jsx_attributes()
}
```

The concern is valid: a handwritten declaration is brittle. A rule author can
forget a capability, over-declare one, or call broad escape hatches that are not
reflected in `capabilities()`. Rust will still compile the rule, so the engine
cannot fully trust the declaration.

The goal is not to remove capability planning. The goal is to make capability
planning mechanically trustworthy.

## Short Conclusion

Do not infer capabilities from arbitrary Rust rule bodies.

Instead, change the rule authoring API so rules are written in a restricted,
analyzable shape where the function signature is the capability source of
truth. A procedural macro can then generate the `Rule` implementation and its
`capabilities()` method from typed fact-view parameters.

Implementation status on the current static-capability branch: this is now the
normal rule-authoring path. `#[polint::rule]` generates the `Rule`
implementation, examples and `polint new-rule` use typed fact views, and broad
fact access moved off `RuleCtx`.

In other words:

- Bad target: scan arbitrary Rust and guess what facts the rule uses.
- Good target: make rules request facts through typed parameters, then generate
  capabilities from those parameter types.

## Why Capability Enumeration Still Matters

polint needs capabilities before it runs analysis. That lets the engine:

- plan expensive or setup-dependent analysis work before parsing
- validate missing setup before producing misleading diagnostics
- include the analysis shape in cache identity
- explain what a ruleset requires without reading source code
- keep Go, TS/JS, Python, Java, and future languages behind one public fact
  model

The question is not whether capabilities should exist. They should. The question
is how the engine obtains them reliably.

## Previous Model

The previous public rule shape was roughly:

```rust
pub trait Rule: Send + Sync {
    fn meta(&self) -> RuleMeta;
    fn capabilities(&self) -> Capabilities;
    fn run(&self, ctx: &mut RuleCtx<'_>) -> RuleResult;
}
```

`RuleCtx` exposed many fact accessors, and also had broad access through
`ctx.db()`. That meant `capabilities()` and actual fact usage were separate
things.

That worked as a planning contract, but it was not a checked contract.

## Why Arbitrary Source Inference Is The Wrong Path

Inferring capabilities from arbitrary Rust would mean analyzing the rule crate
and answering questions like:

- Did the rule call `ctx.imports()` directly?
- Did it call a helper that calls `ctx.imports()`?
- Did it use a trait method, macro, type alias, generic helper, closure, or
  re-export?
- Did it call `ctx.db()` and manually inspect facts?
- Did it conditionally read a fact only under some config?

Doing this correctly requires Rust-aware semantic analysis, not text matching.
Rust has compiler lints and Clippy-style late lint passes that can inspect typed
code, but building an external custom compiler/lint driver depends on rustc
internals and toolchain setup. That is too heavy for the normal rule-authoring
path.

Source scanning would give a false sense of safety. It would catch simple cases
and miss exactly the abstractions users will naturally write.

## Recommended Design: Typed Fact Views

Make the rule signature carry the dependency information.

Conceptual rule:

```rust
#[polint::rule(
    id = "local/no-raw-colors",
    description = "Require design tokens instead of raw TSX color literals.",
    severity = "error"
)]
fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    strings: StringLiterals<'_>,
    jsx: JsxAttributes<'_>,
) -> RuleResult {
    for literal in strings.iter() {
        // ...
    }

    for attribute in jsx.iter() {
        // ...
    }

    Ok(())
}
```

The macro sees `StringLiterals<'_>` and `JsxAttributes<'_>` and generates:

```rust
fn capabilities(&self) -> Capabilities {
    Capabilities::new().string_literals().jsx_attributes()
}
```

The rule author no longer writes `capabilities()` separately. The requested fact
views are the declaration.

## What This Makes Safer

This approach removes the main brittleness:

- Under-declaration becomes impossible for normal rules because a rule cannot
  access `StringLiterals` unless it asks for `StringLiterals` in the signature.
- Over-declaration becomes visible because unused fact-view parameters trigger
  ordinary Rust warnings or can be linted by the macro/test harness.
- The engine can still build an `AnalysisPlan` before source analysis because
  the generated `Rule` implementation exposes capabilities normally.
- Helpers remain possible, but helpers should accept narrow fact views instead
  of a broad `AnalysisDb`.

Example helper:

```rust
fn report_denied_literals(
    ctx: &mut RuleCtx<'_>,
    strings: StringLiterals<'_>,
) -> RuleResult {
    // ...
}
```

This is analyzable because the dependency still appears in the helper's type
signature and in the calling rule's parameter list.

## Required API Split

To make this model real, `RuleCtx` should be split:

| Surface | Purpose |
|---|---|
| `RuleCtx` | reporting diagnostics, reading options, source lookup, support/status metadata |
| `Imports<'a>` | import facts only |
| `StringLiterals<'a>` | string and regex literal facts only |
| `JsxAttributes<'a>` | JSX attribute facts only |
| `GoTests<'a>` | Go test facts only |
| `BranchObligations<'a>` | branch obligation facts only |
| future `Cfg<'a>` | CFG facts only |
| future `Coverage<'a>` | coverage facts only |
| future `Symbols<'a>` | symbol/reference facts only |

The stable rule-authoring prelude should prefer these narrow views. Broad
database access should either become internal, explicitly advanced/unstable, or
excluded from the recommended macro path.

If `ctx.db()` remains a normal public escape hatch, capability correctness is
not enforceable.

## Rule Shape Restrictions

For this to stay analyzable, the macro path should intentionally restrict what
counts as a fact dependency:

- Fact parameters must use concrete polint fact-view types.
- Type aliases should not be accepted for fact parameters in v1.
- Generic fact-view parameters should not be accepted in v1.
- Macros inside the rule body are allowed only because they cannot grant new
  facts; they can only use the fact views already passed in.
- Conditional use still declares the superset of possible facts.
- Direct manual `impl Rule` stays possible only as an advanced/internal escape
  hatch, not the default documented path.

These restrictions are a feature. They are what make the rule analyzable.

## Alternatives Considered

| Approach | Verdict | Reason |
|---|---|---|
| Keep handwritten `capabilities()` | Not enough | Simple, but users can lie accidentally and Rust will not catch it. |
| Text scan rule source for `ctx.*()` calls | Reject | Breaks on helpers, aliases, macros, traits, generics, and `ctx.db()`. |
| Runtime record actual accessor usage | Useful adjunct | Good for `polint test-rules`, but too late for planning and cache identity. |
| Custom rustc/Clippy lint driver | Defer | Powerful, but heavy and tied to unstable compiler internals/toolchain setup. |
| Type-state `RuleCtx<HasImports, HasStrings>` | Possible later | Strong but complex with dynamic rule registration and trait objects. |
| Macro-derived typed fact views | Recommended | Removes duplicate declarations while keeping planning cheap and explicit. |

## Implementation Plan

### Step 1: Introduce Fact View Types

Add narrow borrowed fact views under the SDK, for example:

```rust
pub struct Imports<'a> { /* private */ }
pub struct StringLiterals<'a> { /* private */ }
pub struct JsxAttributes<'a> { /* private */ }
```

Each view wraps `&AnalysisDb` or a pre-filtered slice/iterator and exposes only
the corresponding fact family.

Add an internal trait similar to:

```rust
trait FactView<'a>: Sized {
    const CAPABILITY: CapabilityName;
    fn build(ctx: &'a RuleRuntimeCtx<'a>) -> Self;
}
```

The exact trait can stay internal or sealed. Rule authors should normally use
the concrete view types, not implement the trait.

### Step 2: Split `RuleCtx`

Reduce the normal public `RuleCtx` role to:

- diagnostics
- options
- source lookup
- path-context helpers that do not expose arbitrary facts
- capability support/status metadata

Move fact-family accessors from `RuleCtx` to typed fact views.

Decision needed: whether `ctx.db()` is removed from the stable prelude,
feature-gated, renamed to an advanced escape hatch, or kept temporarily with a
clear warning that macro-derived capability safety does not apply when used.

### Step 3: Add A Proc-Macro Crate

Rust procedural macros must live in a separate `proc-macro` crate and operate on
syntax token streams. Add a small crate such as:

- `crates/polint-macros`

Expose the macro through the SDK:

```rust
pub use polint_macros::rule;
```

The macro should parse:

- `id`
- `description`
- `severity`
- function name
- function parameters
- return type

It should recognize only approved fact-view parameter types and map them to
capabilities.

### Step 4: Generate A Normal `Rule`

The macro should generate a normal implementation of the existing internal
`Rule` trait so the runner and `AnalysisPlan` do not need a large rewrite.

Conceptually:

```rust
#[polint::rule(...)]
fn no_raw_colors(ctx: &mut RuleCtx<'_>, strings: StringLiterals<'_>) -> RuleResult {
    // user body
}
```

expands into:

- a generated rule struct
- `meta()`
- generated `capabilities()`
- `run()` that builds the requested fact views and calls the user's function
- a small factory function that can be passed to `runner::run_cli`

This keeps the runtime model compatible with `Vec<Arc<dyn Rule>>` while making
capabilities generated.

### Step 5: Make The Macro Path The Documented Path

Rewrite examples to use the macro/fact-view style first. Manual `impl Rule`
should either disappear from docs or be explicitly labeled as advanced.

The target user experience should be:

```rust
fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        no_raw_colors(),
        go_import_boundaries(),
    ])
}
```

where each generated function returns an `Arc<dyn Rule>` or equivalent rule
registration object.

### Step 6: Add Compile-Fail Tests

Use compile-fail tests to prove invalid rule shapes are rejected:

- unknown fact-view parameter
- type alias used as a fact-view parameter, if disallowed
- missing `RuleCtx` parameter
- unsupported return type
- duplicate fact views if the API chooses to reject duplicates

Add normal integration tests proving:

- generated capabilities appear in `polint explain plan`
- generated capabilities affect cache identity
- checked-in examples compile and run through the macro path
- unsupported future views such as `Cfg<'_>` produce an unsupported capability
  diagnostic until their owning phase is implemented

### Step 7: Add Runtime Validation As Defense In Depth

Even with typed fact views, runtime validation is still useful for advanced
paths:

- during `polint test-rules`
- when a rule manually implements `Rule`
- when an advanced escape hatch is used

This validation can record fact-view construction or broad DB access and compare
it to generated/declared capabilities. It should be treated as a safety net, not
the primary model.

## Migration Plan For This PR

Because this PR is not shipped and the rule-authoring surface is not used by
external users yet, backwards compatibility should not constrain this rewrite.
It is acceptable to break existing example rules, generated scaffolds, and the
current `Rule` API if doing so produces a simpler and more trustworthy model.

Do not preserve the handwritten `capabilities()` API out of habit. If the macro
and typed fact-view design is the right product shape, prefer replacing the old
shape completely before release over carrying compatibility shims.

Recommended migration order:

1. Keep the current `AnalysisPlan` concept.
2. Replace handwritten rule capability declarations in examples with
   macro-derived capabilities.
3. Keep `Capabilities` as the internal plan representation, but stop making
   normal users write it directly.
4. Split public fact access into typed fact views.
5. Update `polint new-rule` scaffolds to generate macro-style rules.
6. Update docs to say capabilities are derived from fact-view parameters.
7. Decide whether manual `impl Rule` remains public advanced API or becomes
   internal before release.

Compatibility posture:

- Breaking example rule code is fine.
- Breaking generated `polint new-rule` scaffolds is fine.
- Breaking the current public `Rule` trait is fine before release.
- Removing or hiding `ctx.db()` from the normal SDK path is fine.
- Renaming types is fine if it makes the API easier to explain.
- Keeping temporary adapters is only useful when it reduces implementation risk;
  it should not shape the released API.

This keeps the good part of Phase 11, which is the engine planning model, while
replacing the brittle part, which is the handwritten declaration.

## Open Decisions

1. Should manual `impl Rule` remain a supported public API?
   - Keeping it preserves flexibility but weakens the capability guarantee.
   - Removing or hiding it makes the product promise cleaner before release.

2. Should `ctx.db()` remain in the stable SDK prelude?
   - Keeping it is convenient.
   - Removing it is the clearest way to make capability derivation trustworthy.

3. Should the macro accept type aliases for fact views?
   - Accepting aliases is ergonomic.
   - Rejecting aliases keeps derivation simple and auditable.

4. How should generated rules be registered?
   - Factory functions are explicit and fit the current `run_cli` model.
   - Automatic inventory-style registration is more magical and not necessary.

## Research Notes

- Rust procedural macros can transform syntax token streams and generate new
  items at compile time, which fits the proposed rule macro approach:
  <https://doc.rust-lang.org/reference/procedural-macros.html>
- Rust compiler lints and Clippy passes can inspect Rust code, including typed
  code in late lint passes, but this is a lint/tooling path rather than a stable
  public API design:
  <https://doc.rust-lang.org/stable/clippy/development/adding_lints.html>
- External rustc drivers can run compiler callbacks, but this relies on rustc
  internals and is too heavy for the normal rule authoring workflow:
  <https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html>
- `rustc_private` exposes unstable compiler internals such as `rustc_driver` and
  requires extra toolchain components, which reinforces that custom compiler
  analysis should not be the main product path:
  <https://doc.rust-lang.org/unstable-book/language-features/rustc-private.html>

## Final Recommendation

Rewrite the rule-authoring layer around macro-derived, typed fact-view
parameters.

This gives polint the static enumeration it needs without asking users to write
duplicate declarations. It also makes rules intentionally analyzable: the facts
available to the rule are visible in the function signature, the macro turns
those types into capabilities, and the runner can still build an `AnalysisPlan`
before doing expensive work.

The product claim should become:

> A rule's capabilities are derived from the fact views in its signature.

That is stronger, easier to explain, and much less brittle than requiring rule
authors to manually keep `capabilities()` in sync with their code.

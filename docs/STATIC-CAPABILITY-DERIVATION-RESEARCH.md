# Static Capability Derivation Research

## Purpose

This document explains why polint rule capabilities are derived from typed rule
function parameters instead of handwritten declarations.

The old shape asked rule authors to keep a declaration like this in sync with
the code they wrote:

```rust
Capabilities::new().imports().jsx_attributes()
```

That is brittle. A rule author can forget a capability, over-declare one, or
move fact usage into a helper without updating the declaration. Rust still
compiles the rule, so the engine cannot fully trust the declaration.

The goal is not to remove capability planning. The goal is to make capability
planning mechanically trustworthy.

The broader agent-facing static-analysis rationale is captured in
[`research/STATIC-ANALYSIS-FOR-AI-AGENTS.md`](research/STATIC-ANALYSIS-FOR-AI-AGENTS.md).
That note explains why typed fact views, setup diagnostics, and machine-readable
rule output matter for coding-agent repair loops.

## Short Conclusion

Do not infer capabilities from arbitrary Rust rule bodies.

Instead, polint rules are written in a restricted, analyzable shape where the
function signature is the capability source of truth. The `#[polint::rule]`
macro reads typed fact-view parameters and generates the opaque `Rule` value
that the runner executes.

In other words:

- Bad target: scan arbitrary Rust and guess which facts the rule uses.
- Good target: make rules request facts through typed parameters, then generate
  capabilities from those parameter types.

## Why Capability Enumeration Matters

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

The previous public rule shape had a public trait with separate `meta`,
`capabilities`, and `run` methods. `RuleCtx` also exposed broad fact access.

That made the capability contract a promise written by the user, not something
checked by the rule shape. It also encouraged examples to call internal polint
surfaces directly because the easiest way to write a rule was to implement the
same trait that the engine used internally.

That model has been removed for normal rule authors. `Rule` is now an opaque
value returned by generated rule factory functions.

## Why Arbitrary Source Inference Is The Wrong Path

Inferring capabilities from arbitrary Rust would mean analyzing the rule crate
and answering questions like:

- Did the rule call a fact accessor directly?
- Did it call a helper that reads facts?
- Did it use a trait method, macro, type alias, generic helper, closure, or
  re-export?
- Did it conditionally read a fact only under some config?

Doing this correctly requires Rust-aware semantic analysis, not text matching.
Rust has compiler lints and Clippy-style late lint passes that can inspect typed
code, but building an external custom compiler/lint driver depends on rustc
internals and toolchain setup. That is too heavy for the normal rule-authoring
path.

Source scanning would give a false sense of safety. It would catch simple cases
and miss exactly the abstractions users naturally write.

## Current Design: Typed Fact Views

The rule signature carries dependency information:

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

The macro sees `StringLiterals<'_>` and `JsxAttributes<'_>` and generates the
rule capabilities from those types. The rule author no longer writes
`capabilities()` separately. The requested fact views are the declaration.

## What This Makes Safer

This approach removes the main brittleness:

- Under-declaration is not part of the normal rule shape because a rule cannot
  access `StringLiterals` unless it asks for `StringLiterals` in the signature.
- Over-declaration becomes visible because unused fact-view parameters trigger
  ordinary Rust warnings and can be tightened further by macro tests.
- The engine can still build an `AnalysisPlan` before source analysis because
  the generated opaque `Rule` exposes generated capabilities internally.
- Helpers remain possible, but helpers accept narrow fact views instead of a
  broad database.

Example helper:

```rust
fn report_denied_literals(
    ctx: &mut RuleCtx<'_>,
    strings: StringLiterals<'_>,
) -> RuleResult {
    // ...
}
```

This is analyzable because the dependency still appears in the calling rule's
parameter list.

## API Split

The public rule-authoring surface is intentionally split:

| Surface | Purpose |
|---|---|
| `RuleCtx` | diagnostics, options, source lookup, and capability/setup metadata |
| `Imports<'a>` | import facts only |
| `StringLiterals<'a>` | string and regex literal facts only |
| `JsxAttributes<'a>` | JSX attribute facts only |
| `GoTests<'a>` | Go test facts only |
| `BranchObligations<'a>` | branch obligation facts only |
| `Functions<'a>` | function facts only |
| `FileMetrics<'a>` | derived file-size metrics only |
| `FunctionMetrics<'a>` | derived function-size metrics only |
| `ComplexityMetrics<'a>` | derived complexity metrics only |
| `Packages<'a>` | package facts only |
| `TsComponents<'a>` | TS component facts only |
| `TsClasses<'a>` | TS class facts only |
| future `Cfg<'a>` | CFG facts only |
| future `CallGraph<'a>` | call graph facts only |
| future `DataFlow<'a>` | dataflow facts only |
| future `CoverageFacts<'a>` | coverage facts only |

`RuleCtx` is not the normal fact surface. Fact families live behind typed views
so capability derivation stays visible at the rule boundary.

## Rule Shape Restrictions

For this to stay analyzable, the macro path intentionally restricts what counts
as a fact dependency:

- The first parameter must be a simple mutable `RuleCtx<'_>` binding.
- Rule functions must be plain non-generic sync functions and return
  `RuleResult` or `RuleResult<()>`.
- Fact parameters must use concrete polint fact-view types exported by the SDK
  prelude or written as canonical `polint::sdk::facts::*` paths, with the
  placeholder lifetime form such as `Imports<'_>`.
- The macro constructs canonical `polint::sdk::facts::*` views; arbitrary
  qualified lookalike paths are rejected, and local unqualified lookalike types
  fail Rust type checking instead of becoming user-defined fact sources.
- Type aliases with different names are not accepted as fact parameters in v1;
  rule authors should write the canonical view name directly.
- Generic fact-view parameters are not accepted in v1.
- Conditional use still declares the superset of possible facts.
- If the resolved plan marks a requested hard capability as unsupported or
  setup-missing, polint reports the capability problem and does not execute the
  rule with placeholder facts.
- Macros inside the rule body are allowed only because they cannot grant new
  facts; they can only use the fact views already passed in.
- Manual `impl Rule` is not a supported authoring path. The `Rule` type is
  opaque, and user-facing examples/scaffolds must not reintroduce trait
  implementation shims.

These restrictions are a feature. They are what make the rule analyzable.

## Alternatives Considered

| Approach | Verdict | Reason |
|---|---|---|
| Keep handwritten capabilities | Reject | Users can lie accidentally and Rust will not catch it. |
| Text scan rule source for fact calls | Reject | Breaks on helpers, aliases, macros, traits, generics, and re-exports. |
| Runtime record actual accessor usage | Useful adjunct | Good for future rule tests, but too late for planning and cache identity. |
| Custom rustc/Clippy lint driver | Defer | Powerful, but heavy and tied to unstable compiler internals/toolchain setup. |
| Type-state `RuleCtx<HasImports, HasStrings>` | Possible later | Strong but complex with dynamic rule registration and opaque rules. |
| Macro-derived typed fact views | Chosen | Removes duplicate declarations while keeping planning cheap and explicit. |

## Implementation Scope

The implementation follows this product contract:

1. Add narrow SDK fact views with private construction and public iterators.
2. Keep `RuleCtx` focused on diagnostics, options, source paths, and
   capability/setup metadata.
3. Add `polint-macros` and expose `#[polint::rule]`.
4. Generate opaque `Rule` values from annotated functions.
5. Register rules as `Vec<Rule>` through `polint::runner::run_cli`.
6. Derive capabilities from concrete fact-view parameter types.
7. Update examples and `polint new-rule` scaffolds to use the macro style.
8. Test the outside-user path with temp repos that import only the public SDK
   and assert diagnostics through `polint check --format json`.

Because this branch has not shipped, backwards compatibility does not constrain
the rewrite. It is acceptable to break old examples, generated scaffolds, and
the previous public `Rule` trait shape to keep the released API small and
trustworthy.

No compatibility shims should be added just to preserve handwritten rule
implementations during beta development.

## Closed Decisions

1. Manual rule trait implementation is not supported.
   `Rule` is an opaque value, not a public trait.

2. Fact capabilities are derived from typed parameters.
   The rule function signature is the source of truth.

3. `RuleCtx` is not a broad fact database.
   Facts are read through narrow typed views.

4. Rule registration stays explicit.
   Generated functions return `Rule`, and rule packs pass those values to
   `polint::runner::run_cli`.

## Research Notes

- Rust procedural macros can transform syntax token streams and generate new
  items at compile time, which fits the rule macro approach:
  <https://doc.rust-lang.org/reference/procedural-macros.html>
- Rust compiler lints and Clippy passes can inspect Rust code, including typed
  code in late lint passes, but this is a lint/tooling path rather than a stable
  public API design:
  <https://doc.rust-lang.org/stable/clippy/development/adding_lints.html>
- External rustc drivers can run compiler callbacks, but this relies on rustc
  internals and is too heavy for normal rule authoring:
  <https://rustc-dev-guide.rust-lang.org/rustc-driver/intro.html>
- `rustc_private` exposes unstable compiler internals such as `rustc_driver` and
  requires extra toolchain components, which reinforces that custom compiler
  analysis should not be the main product path:
  <https://doc.rust-lang.org/unstable-book/language-features/rustc-private.html>

## Final Recommendation

Keep the rule-authoring layer centered on macro-derived, typed fact-view
parameters.

The product claim is:

> A rule's capabilities are derived from the fact views in its signature.

That is stronger, easier to explain, and much less brittle than requiring rule
authors to manually keep capabilities in sync with their code.

# OSS Implementation Comparison

## Summary Table

| System | Extension Unit | Can Improve Engine Facts? | Runs User Code? | Isolation | Main Lesson For Polint |
|---|---|---:|---:|---|---|
| Dylint | Rust dynamic lint library | Mostly diagnostics over rustc facts | Yes, Rust | In-process inside rustc driver | Copy Rust-code ergonomics and version handshake; avoid dylib ABI. |
| Clippy/rustc | Compiled lint passes | Yes, but only inside compiler | No external runtime plugin | In-process | Copy phased passes and UI tests; avoid rustc-private public coupling. |
| Error Prone | Java `BugChecker` | Mostly diagnostics over javac semantics | Yes, Java | In-process compiler plugin | Copy matcher interfaces, semantic state, compile fixture tests. |
| OpenRewrite | Java `Recipe`/`ScanningRecipe` | Yes for transformation/search metadata | Yes, Java | In-process | Copy scan -> accumulate -> generate/emit lifecycle and validation. |
| ESLint | JS rule/plugin | Mostly diagnostics/fixes | Yes, JS | In-process Node | Copy simple rule shape and `RuleTester`; avoid dynamic string API. |
| TypeScript LS plugins | JS plugin around language service | Editor diagnostics/navigation only | Yes, JS | In-process tsserver | Copy host-version injection; do not use as CI-analysis model. |
| CodeQL MaD | YAML data extensions | Yes, data-flow models | No executable code in model files | N/A | Copy model taxonomy and provenance, but implement in Rust code. |
| Pysa | `.pysa` models and Python generators | Yes, taint models | Generator code can run | Process/script dependent | Copy pre-analysis model generation and expected/unexpected validation. |
| Semgrep | YAML rules with taint clauses | Yes, per-rule taint behavior | Mostly declarative | Engine controlled | Copy source/sink/propagator vocabulary, not config-first design. |
| Joern | CPG passes and Scala queries | Yes, graph overlays/layers | Yes, Scala | In-process JVM | Copy overlay/layer idea; avoid exposing raw graph mutation first. |

## Dylint

Dylint is the closest Rust-code precedent. It lets users write lint libraries as Rust dynamic libraries. These libraries use `rustc_private`, implement rustc lint passes, and are loaded by a custom rustc driver. Dylint manages toolchain-qualified artifact names and uses a wrapper/linking strategy to find the right libraries.

**Strengths:**

- Rust code as extension unit.
- Clear discovery and listing patterns.
- Version/toolchain handshake before execution.
- Good local authoring story for Rust lints.

**Weaknesses for polint:**

- Exact compiler/toolchain coupling.
- In-process dynamic library crash risk.
- Public dependency on compiler internals.
- Rust-only fact model.

**Polint decision:** do not use dynamic Rust libraries as the first extension runtime. Use a Rust executable protocol.

## Error Prone

Error Prone checkers extend `BugChecker`, implement specific matcher interfaces, and receive `VisitorState` with javac semantic state. Plugin checks are loaded through Java's `ServiceLoader` on the annotation processor path.

This is a strong semantic-checker design, but it intentionally exposes compiler internals. That is acceptable in a compiler plugin ecosystem; it would be a liability for polint because polint must preserve a stable multi-language SDK.

**Polint decision:** copy semantic state access through typed views, not raw compiler internals.

## OpenRewrite

OpenRewrite's `Recipe` and `ScanningRecipe` model is the best lifecycle precedent. A scanning recipe can first collect repository-wide information, then optionally generate files, then perform transformations. Recipes also have metadata, options, descriptors, validation, and tests.

For polint, the equivalent is not transformation. The equivalent is:

```text
scan base facts -> extension accumulates domain model -> extension emits typed facts -> host validates -> downstream analyses run
```

**Polint decision:** copy the scan/accumulator lifecycle and validation, but apply it to fact emission rather than code modification.

## ESLint

ESLint shows how to make custom rules easy: metadata plus `create(context)` returning visitors, with `RuleTester` for valid/invalid fixtures. It also exposes code-path events and scope services.

The limitation is that ESLint's plugin surface is dynamic and stringly. It is excellent for JavaScript; it is not the right shape for a Rust SDK whose goal is analyzability, capability planning, and cache determinism.

**Polint decision:** keep the current `#[polint::rule]` typed-view model and add an equally typed extension macro.

## CodeQL Models-as-Data

CodeQL custom library models are the best source/sink/summary/barrier taxonomy. They let users model libraries and frameworks that are not represented in source code, and the tuples include provenance.

For polint, the important idea is not YAML. It is the controlled output vocabulary:

- source model;
- sink model;
- summary model;
- barrier model;
- barrier guard model;
- type/access-path model;
- provenance.

**Polint decision:** expose Rust builders that emit these model facts, with validation against symbols/references and access paths.

## Pysa Model Generators

Pysa explicitly recognizes that some models are too numerous or too dynamic to hand-write. Generators can inspect project source before analysis, then emit taint models. Pysa's model DSL also supports expected and unexpected generated model checks.

This matches polint's agent-era thesis. An agent can inspect the repo, then generate a Rust extension that produces the exact project-specific model.

**Polint decision:** make extension fixtures first-class: extensions must prove expected emitted facts, unexpected facts, and before/after precision deltas.

## Joern

Joern uses code property graphs and overlays/layers. An overlay records that a layer has been applied and prevents repeated application. This is directly useful for polint's derived fact families.

**Polint decision:** copy the concept of typed layers with dependencies and applied-layer metadata, but do not expose raw graph mutation as the first public extension API.

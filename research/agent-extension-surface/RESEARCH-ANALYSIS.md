# Research Analysis

## Product Framing

polint already has the first user story:

```text
users write repo-local rules in Rust
```

This research is about the second user story:

```text
users and AI agents write repo-local Rust extensions that improve the analysis engine itself
```

That is a different product surface. A rule observes facts and reports diagnostics. An analysis extension produces or corrects facts so later analysis becomes more accurate.

The old static-analysis assumption is that the analyzer must infer almost everything generically. polint's assumption is different. The analysis engine should have sane defaults and visible uncertainty, then allow agents to add codebase-specific semantics with Rust code.

## Critical Split: Rules Versus Extensions

The split should be non-negotiable:

| Surface | Purpose | Inputs | Outputs | Can Affect Later Analysis? |
|---|---|---|---|---|
| Rule | Report policy diagnostics | Typed fact views | Diagnostics and fixes | No |
| Analysis extension | Improve analysis precision | Typed fact views and lifecycle context | Typed facts/models | Yes |
| Language adapter | Parse and harvest base facts | Source files and language setup | Native base facts | Yes, but polint-owned |

This keeps rule authoring simple while still allowing maximum capability for agents that need deeper integration.

## Why Rust Code, Not Configuration

Configuration can name patterns. Rust code can inspect a repository, run multi-pass logic, bind to symbols, generate summaries, and decide when not to model something because confidence is too low.

This matters for the intended user. An AI agent can:

- inspect a custom router implementation;
- read tests and framework bootstrap code;
- identify actual source/sink APIs;
- generate a repo-local extension;
- run fixture tests;
- measure unresolved-call and data-flow deltas;
- iterate until the extension is accurate.

Static TOML/YAML config is not enough for that. It becomes either too weak or an accidental programming language. The extension should be real Rust code.

## But Rust Code Does Not Require Rust Dynamic Libraries

Dylint proves Rust dynamic lint libraries can work, but its architecture is shaped by rustc:

- lint libraries are compiled as dynamic libraries;
- the artifact name encodes toolchain compatibility;
- the driver loads libraries in-process;
- the public authoring surface relies on `rustc_private`;
- a plugin failure can compromise the driver process.

polint should not inherit those constraints. The recommended runtime is:

```text
.polint/extensions/<extension>/Cargo.toml
  -> cargo build/run cached by extension digest
  -> versioned stdin/stdout protocol
  -> host validates emitted facts
  -> downstream analyses consume accepted facts
```

The extension is still Rust code. The boundary is a protocol, not a Rust ABI.

## Accuracy Model

Extensions improve accuracy by reducing unknowns and adding high-confidence domain facts.

### Call Graph Accuracy

Default call graph analysis can resolve:

- direct calls;
- imported function calls;
- method calls with known receiver type;
- language-native dispatch where type information exists.

Repo-local extensions can add:

- framework entrypoints;
- route handler edges;
- job scheduler edges;
- callback registrations;
- dependency injection bindings;
- generated-code call targets;
- project-specific dynamic dispatch.

The extension does not say "this graph is complete." It emits edges with precision, confidence, and provenance.

### Data-Flow Accuracy

Default data flow can handle:

- local variable flow;
- field/access-path flow where supported;
- direct function summaries;
- built-in sources/sinks for common language primitives.

Repo-local extensions can add:

- true application sources;
- domain-specific sinks;
- sanitizers and barriers;
- wrapper function summaries;
- additional framework flow steps;
- storage/retrieval summaries;
- API-specific effect models.

This is where polint can outperform universal tools: the agent can model the customer's actual trust boundaries instead of relying on global heuristics.

## Complexity Model

Extensions add cost in three places:

1. Building the extension crate.
2. Serializing input fact views to the extension process.
3. Validating and merging emitted facts.

The right complexity target is:

```text
host planning: O(R + E + F)
extension execution: extension-declared, measured and budgeted
fact validation: O(output facts * binding lookup cost)
merge/dedupe: O(output facts log output facts)
downstream invalidation: dependency graph over fact families
```

Where:

- `R` = rules;
- `E` = extensions;
- `F` = requested fact families.

The extension API should force authors to declare:

- input fact families;
- output fact families;
- whether the extension is per-file, per-package, or whole-repo;
- whether it is deterministic;
- a maximum runtime budget;
- whether it needs filesystem or command execution privileges.

## Failure Modes

| Failure | Risk | Required Host Behavior |
|---|---|---|
| Extension does not compile | Rules silently lose precision | Emit `polint/capability` setup diagnostic for dependent facts. |
| Extension panics | Host crash | Process isolation; convert to controlled diagnostic. |
| Extension emits malformed facts | Bad analysis | Reject facts with validation diagnostics. |
| Extension emits unsupported symbol IDs | False certainty | Require binding to existing facts or explicit synthetic-symbol declaration. |
| Extension over-models dynamic behavior | False positives or false negatives | Confidence/provenance plus fixture and delta validation. |
| Extension reads network or environment | Non-determinism/supply chain risk | Off by default; explicit unsafe capability. |
| Extension changes without cache invalidation | Stale facts | Digest extension source, Cargo.lock, SDK/protocol version, options, and output schema. |

## What To Copy

### From ESLint

Copy the small authoring model and fixture discipline. For simple rules, polint already has the better Rust equivalent: `#[polint::rule]` plus typed fact-view parameters.

### From Error Prone

Copy semantic-state richness, but only through typed views. `VisitorState` is powerful because it exposes types, symbols, paths, suppression state, and compiler context. polint should expose `Symbols<'_>`, `References<'_>`, `Calls<'_>`, `DataFlow<'_>`, `Cfg<'_>`, and future `Types<'_>` views, not raw parser internals.

### From OpenRewrite

Copy scanning recipes. Analysis extensions need scan/accumulate/emit phases:

```text
scan base facts -> build repo model -> emit typed facts -> validate -> use downstream
```

### From CodeQL

Copy the model taxonomy:

- source;
- sink;
- summary;
- barrier;
- barrier guard;
- access path;
- provenance.

Do not copy config-first YAML as the primary surface.

### From Pysa

Copy model generators and expected/unexpected generated-model tests. The important lesson is that model coverage is a first-class path to analysis coverage.

### From Joern

Copy overlay/layer dependency metadata. Extension-emitted fact families should be named layers with dependencies and one-time application semantics.

## What To Avoid

- Do not expose `AnalysisDb` mutation to extension authors.
- Do not expose raw ASTs as the normal extension API.
- Do not add a generic "run arbitrary hook at every internal phase" API.
- Do not make extensions replace native language adapters.
- Do not trust extension facts just because they compiled.
- Do not make configuration the primary model language.
- Do not advertise a capability until a rule or extension can consume/produce real typed facts.

## Recommended Extension Categories

### 1. Entrypoint Providers

Emit facts for externally reachable code:

- HTTP routes;
- RPC handlers;
- CLI commands;
- scheduled jobs;
- queue consumers;
- serverless handlers;
- MCP tools;
- test harness entrypoints.

### 2. Call Model Providers

Emit or refine call graph edges:

- callback registration;
- dependency injection;
- generated clients;
- dynamic dispatch conventions;
- framework route dispatch.

### 3. Data-Flow Model Providers

Emit:

- sources;
- sinks;
- sanitizers;
- barriers;
- summaries;
- additional flow steps;
- access-path transformations.

### 4. Effect Providers

Emit function-level effects:

- reads/writes filesystem;
- network;
- process execution;
- database access;
- logging;
- secrets access;
- authorization checks;
- mutation/purity.

### 5. Type/Alias/Value Hint Providers

Emit high-confidence hints:

- framework injection binding;
- type narrowing beyond parser/default typechecker;
- alias facts;
- generated symbol facts;
- shape facts.

## Public SDK Implication

The SDK should split:

```rust
polint::sdk::prelude::*              // simple rule authors
polint::extension::prelude::*        // analysis extension authors
```

The extension prelude can be more complex. Its audience includes agents that can generate Rust code and tests. It should still be typed, documented, and stable enough for repo-local code.

## Precision Labels

Every emitted extension fact needs precision metadata:

```rust
enum ExtensionPrecision {
    Exact,
    ConservativeOverApprox,
    Heuristic,
    UserAsserted,
    GeneratedUnvalidated,
    ValidatedByFixture,
}
```

Rules should be able to filter or explain by precision. Diagnostics should show when an important edge or flow segment came from a repo-local extension.

## Final Research Judgment

The path to maximum capability is not a bigger config file and not a black-box universal engine. It is a typed analysis extension lifecycle where agents can write Rust code that improves the engine's facts under strict validation.

# Recommended Implementation

## Goal

Make polint's authoring surface excellent for both humans and AI agents while
preserving the current architecture discipline:

- public rules use `polint::sdk::prelude::*`;
- rules request facts through typed parameters;
- capabilities are derived, not hand-written;
- `RuleCtx` stays narrow;
- advanced analysis improvements go through models or provider extensions;
- every generated artifact has fixtures.

## Phase 0: Tighten The Existing Rule Macro Contract

The current macro already validates plain non-generic sync rule functions and
derives capabilities from fact-view parameters. Keep that direction.

Add metadata fields incrementally:

```rust
#[polint::rule(
    id = "local/no-raw-colors",
    description = "Raw color literals must use design tokens",
    severity = "warn",
    docs = "docs/polint/no-raw-colors.md",
    tags = ["design-system", "frontend"],
    precision = "syntactic",
    stability = "preview"
)]
fn no_raw_colors(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    Ok(())
}
```

Do not require every metadata field immediately, but reserve the schema now.
Fields that affect rule behavior or output must enter cache digests.

## Phase 1: Generated Rule Manifest

Add a manifest type:

```rust
pub(crate) struct RuleManifest {
    pub(crate) id: RuleId,
    pub(crate) description: String,
    pub(crate) docs: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) severity: Severity,
    pub(crate) stability: RuleStability,
    pub(crate) fact_views: Vec<FactViewRequirement>,
    pub(crate) capabilities: Vec<CapabilityRequirement>,
    pub(crate) option_schema: Option<OptionSchema>,
    pub(crate) messages: Vec<MessageDescriptor>,
    pub(crate) fixability: Fixability,
    pub(crate) sdk_version: String,
    pub(crate) rule_code_digest: Digest,
    pub(crate) limitations: Vec<String>,
}
```

Expose it through:

```text
polint inspect rule local/no-raw-colors --format json
polint capabilities --format json
```

This is the key bridge between Rust compile-time inference and agent debugging.

## Phase 2: Keep RuleCtx Narrow

`RuleCtx` should support:

```rust
impl<'a> RuleCtx<'a> {
    pub fn rule_id(&self) -> &str;
    pub fn options(&self) -> &RuleOptions;
    pub fn path_context(&self) -> &PathContext;
    pub fn diagnostic(&mut self, span: Span, message: impl Into<String>) -> DiagnosticBuilder<'_>;
    pub fn diagnostic_with_message_id(
        &mut self,
        span: Span,
        message_id: &'static str,
    ) -> DiagnosticBuilder<'_>;
}
```

Later:

```rust
impl<'a> DiagnosticBuilder<'a> {
    pub fn with_related(self, span: Span, message: impl Into<String>) -> Self;
    pub fn with_evidence_path(self, path: EvidencePath<'_>) -> Self;
    pub fn with_fix(self, fix: Fix) -> Self;
    pub fn emit(self);
}
```

Do not add `ctx.analysis_db()`, `ctx.facts("...")`, or parser AST access.

## Phase 3: Agent Inspect Commands

Add these before advanced providers become public:

```text
polint facts list --format json
polint facts sample --cap imports --path src/file.ts --limit 50 --format json
polint unknowns --cap references --format json
polint explain --rule local/foo --format json
polint inspect rule local/foo --format json
```

The JSON should include:

- supported fields;
- precision/status enums;
- setup gaps;
- capability derivation;
- provider chain;
- cache inputs where available;
- sample facts with stable IDs and spans;
- unknowns with suggested artifact type: rule, model, summary, provider, or
  fixture.

## Phase 4: `polint test`

Implement a real fixture runner.

Default case format:

```toml
kind = "rule"
rule = "local/no-raw-colors"
language = "ts"
format = "json"
expect = "inline-and-snapshot"

[options]
# rule-owned settings
```

Test layout:

```text
.polint/tests/rules/no_raw_colors/basic/
  polint-test.toml
  src/example.ts
  expected.snap.json
```

Inline markers:

```text
polint-expect <rule-id>: <regex>
polint-ok <rule-id>
polint-todo-expect <rule-id>: <regex>
polint-todo-ok <rule-id>
```

Runner steps:

```text
compile rule pack once
for each case:
  create temp repo
  copy fixture
  run real polint check --format json
  normalize output
  assert inline markers
  compare snapshot
  optionally bless
```

Support:

```text
polint test
polint test --rule local/no-raw-colors
polint test --case basic
polint test --bless
polint test --keep-temp
polint test --jobs 8
polint test --no-cache
```

## Phase 5: Improve `polint new-rule`

Every generated rule should include:

- a rule module;
- registration in `.polint/rules/src/main.rs`;
- one positive fixture;
- one negative fixture;
- `polint-test.toml`;
- README stub with precision and limitation notes;
- compileable code using only `polint::sdk::prelude::*`.

Example generated command:

```text
polint new-rule ts no-raw-colors
```

Generated layout:

```text
.polint/rules/src/no_raw_colors.rs
.polint/tests/rules/no_raw_colors/basic/polint-test.toml
.polint/tests/rules/no_raw_colors/basic/src/example.ts
```

## Phase 6: Domain Query Builders

Add convenience methods to fact views instead of exposing raw graphs.

Start with current facts:

```rust
imports.by_path("@server/db")
imports.for_file(file)
modules.import_edges()
symbols.by_name("authorize")
references.to_symbol(symbol)
functions.named("handler")
literals.string_like()
metrics.functions().over_complexity(10)
```

Future preview views:

```rust
calls.named("exec.Command")
call_graph.possible_targets(call_site).limit(20)
flow.sources(source_kind).to(sink).paths().limit(5)
effects.for_function(function).contains(EffectKind::WritesFile)
evidence.for_diagnostic(diagnostic_id).thin_slice()
```

Rules:

- methods that can be expensive must be named honestly;
- unbounded queries should require explicit `.unbounded()` or an explicit limit;
- all path queries should carry precision/status/evidence.

## Phase 7: Model Packs

Add model packs after the relevant analysis families exist.

Recommended layout:

```text
.polint/models/acme-http/
  polint-model-pack.toml
  models/http.toml
  models/auth.toml
  tests/basic/
    polint-test.toml
    src/app.ts
    facts.snap.json
    expected.snap.json
```

Model kinds:

```text
source
sink
sanitizer
barrier
barrier_guard
summary
propagator
entrypoint
framework_dispatch
generated_symbol
```

Every model row should include:

```text
language
package/module matcher
symbol/call matcher
access path
kind
precision
provenance
applicability version range
evidence
```

Validation rejects unknown symbols, invalid access paths, impossible argument
indexes, stale package versions, and unsupported model kinds.

## Phase 8: Provider Extensions

Provider extensions should remain separate from normal rules.

Use provider extensions when:

- analysis needs Rust code;
- a new fact family is emitted;
- lifecycle/framework recovery is required;
- declarative models are insufficient;
- transfer or merge semantics change.

Provider tests must include:

- handshake/protocol test;
- declared fact families;
- deterministic fact IDs;
- emitted fact snapshot;
- consumer rule test;
- failure diagnostics;
- cache invalidation by source/config/provider/model digest.

## Phase 9: Structured Fixes

Reserve API now, implement later.

```rust
pub struct Fix {
    pub message: String,
    pub edits: Vec<TextEdit>,
    pub applicability: Applicability,
}

pub enum Applicability {
    MachineApplicable,
    Safe,
    Unsafe,
    MaybeIncorrect,
    DisplayOnly,
}
```

Fixes must be:

- deterministic;
- conflict-checked centrally;
- dry-run previewable;
- snapshot-tested with before/after fixtures;
- never free-form LLM patches unless compiled down to explicit edits.

## Phase 10: Preview Gates

Add stability labels:

```text
stable
preview
experimental
internal
```

Use preview for:

- new fact views;
- new query builders;
- new model kinds;
- new provider extension sinks;
- new fix APIs;
- behavior expansions that may increase findings.

This copies Ruff's useful stability pattern without hiding incomplete features
behind preview.

## Acceptance Criteria

The first implementation is good enough when:

- a generated rule compiles without manual edits;
- a generated fixture has one positive and one negative case;
- `polint test` exercises the real rule-host path;
- `polint inspect rule` shows derived capabilities and fact precision;
- setup-missing capabilities produce actionable diagnostics;
- rule options are schema-checked and cache-keyed;
- snapshots are deterministic;
- no public API exposes raw internal `AnalysisDb` or parser ASTs.

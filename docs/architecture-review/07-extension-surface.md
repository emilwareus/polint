# 07 — Extension Surface: SDK, Rule Authoring, Distribution, Agent Integration

**Scope:** what a third party (human or AI agent) can extend without forking, how extensions are
distributed, and how well the engine explains itself.

**Verdict:** polint has the *cleanest rule-authoring front door* of any static-analysis tool I have
reviewed — and almost no *platform* behind it. The extension surface is a well-guarded funnel into a
closed engine. Three specific holes are load-bearing for the "most capable" goal: **no shareable rule
distribution at all**, **evidence/provenance is built but deliberately stripped before it reaches the
user**, and **no way to answer "why did my rule not fire."**

---

## (a) The extension surface as it exists

### The public API is genuinely small and well-policed

`crates/polint/src/lib.rs:7-8` exports exactly two modules: `runner` and `sdk`. Everything else —
`analysis`, `analysis_kernel`, `core`, `go`, `ts`, `module_graph`, `symbol_graph`, `cache`,
`diagnostics`, `cli` — is `pub(crate)`. This is enforced mechanically by
`crates/polint/tests/public_surface_leak.rs`, which compiles a probe crate *outside* the workspace
and asserts an exact allowed-prelude item count (currently 99, per
`docs/API-VISIBILITY-PLAN.md:165-185`). That test is one of the best architectural guardrails in the
repo and should be preserved through any refactor.

The rule contract is:

```rust
use polint::sdk::prelude::*;

#[polint::rule(id = "local/no-raw-colors", description = "...", severity = "error")]
pub(crate) fn no_raw_colors(ctx: &mut RuleCtx<'_>, literals: StringLiterals<'_>) -> RuleResult {
    for literal in literals.iter() { /* ... */ }
    Ok(())
}
```

The key mechanic is that **capabilities are derived from the typed fact-view parameters in the
signature** — the signature *is* the capability request. This is the single best design decision in
the product. It makes the analysis plan a compile-time consequence of the rule text, which is exactly
what you want when an LLM is writing the rule: there is no second place to keep in sync, and the
planner can refuse to run a rule whose capability is unsupported instead of feeding it empty facts.
Rationale and rejected alternatives are recorded in
`docs/STATIC-CAPABILITY-DERIVATION-RESEARCH.md:200-207`.

### Extensibility matrix

| Surface | Extensible without forking? | Mechanism | Evidence |
|---|---|---|---|
| **Diagnostic rules** | **Yes** | `#[polint::rule]` in `.polint/rules`, compiled as a child cargo binary | `crates/polint/src/cli/mod.rs:4008-4030` |
| **Rule config** | Yes | `RuleOptions::settings` free-form map | `docs/RULE-AUTHORING-PLATFORM-REVIEW.md:133-195` |
| **Analysis facts (providers)** | **Barely** — host side only | `.polint/extensions/<name>` subprocess over `polint-extension-handshake-v1` JSON | `crates/polint/src/analysis/extensions/host.rs:130-161` |
| **Framework / API models** | **No** (private) | `.polint/models/*.toml` loader exists but is `pub(crate)` and undocumented | `crates/polint/src/analysis/adaptation/mod.rs:1-5` |
| **New language** | **No — requires forking** | Must edit `Language` enum, `AnalysisDb`, `LanguageScope`, kernel `run()` | see `03-frontend-ir-and-language-scaling.md` |
| **New interprocedural analysis** | **No — requires forking** | No `Analysis`/`Provider` trait exists; the pipeline is straight-line | `analysis_kernel/mod.rs:92-968` |
| **Query language** | **No, by explicit decision** | Typed Rust views only | `docs/facts/policy-queries.md:17` |
| **Output consumption** | Yes | 9 versioned JSON schemas in `docs/schemas/` | — |

### The extension protocol is real but the author-side is unbuilt

`crates/polint/src/analysis/extensions/` is 9 modules (~34 KB of validation logic alone) implementing
discovery, digesting, subprocess hosting with a 30 s timeout and 1 MiB stdout cap, sink validation,
and cache keying. It is wired into the kernel behind `run_full_refinement_pipeline`
(`analysis_kernel/mod.rs:668-676`).

But the only working extension in the repo —
`tests/eval-fixtures/extension/real-sink/repo/.polint/extensions/demo/` — **has zero dependencies and
hand-writes the JSON protocol as raw string literals** (`src/main.rs:14-23`). There is no
`polint::extension` module, no `#[polint::extension]` macro, no public `EntrypointSink` /
`CallGraphSink` / `DataFlowModelSink` type, and no `polint extension new/test` CLI command. The
host-side validation boundary exists; the author-side ergonomics do not.

**Assessment:** this is the right half to have built first (validation is the hard, security-relevant
half), but until the author side lands, "provider extensions" is not a capability — it is a protocol
spec with a fixture.

---

## (b) Ergonomics — where the API is good and where it fights back

**Good.** Scaffolding is about as low-friction as Rust allows: `polint new-rule` emits **8 lines of
boilerplate before rule logic**, auto-wires `main.rs`, and generates matched positive/negative
fixtures. There are **10 working policy templates** (`request-to-shell`, `secret-to-log`, `ssrf`,
`dangerous-html`, `unsafe-deserialization`, …) each shipping a rule body *plus* fixtures, so
`polint new-rule ts x --template ssrf && polint test` is a genuine green-then-edit loop
(`crates/polint/src/cli/mod.rs:207-232`, `:1268-1275`). This is the best agent-facing feature in the
product.

**Bad — a scaffolding bug that will burn every first-time author.** `crates/polint/src/cli/mod.rs:1013-1036`
generates a directory named `positive/` with `expect_diagnostic = false` and `negative/` with
`expect_diagnostic = true`. So `positive` means "code that passes" — the opposite of the universal
lint-testing convention (ESLint's `valid`/`invalid`; "positive test case" = the case that triggers).
`SKILL.md:98` says only "creates positive and negative fixture cases" and never defines which is
which. Rename to `clean/` and `violating/`.

**Fact views are iteration, not query.** All 27 views (`crates/polint/src/sdk/facts.rs`) expose
`.iter()` over flat structs. There are no joins, no traversals, no predicates. A rule asking "is this
sink reachable from an HTTP handler without passing a sanitizer" cannot express that over `Calls<'_>`
— which is precisely why `sdk/policy.rs` exists as a parallel, higher-level surface
(`FlowQuery`, `ReachQuery`, `GuardQuery`, `LifecycleQuery`). Those are the *real* analysis API, and
they are hard-coded query shapes rather than a composable algebra. Adding an 11th policy shape means
editing the engine.

---

## (c) Distribution and sandboxing — the biggest structural gap

### There is no way to share a rule between two repositories

This is not an oversight to fix later; it is currently a hole with **no plan behind it**:

- Every generated and example rule pack sets `publish = false` (`cli/mod.rs:893`).
- Grepping all markdown for "rule registry", "rule marketplace", "shared rule", "across repos"
  returns **zero relevant hits**.
- The only forward-looking sentence is `research/agent-rule-authoring/RESEARCH-ANALYSIS.md:257-272`
  ("Later shareable packaging *can* split into rule pack / model pack / …") — with no ADR, roadmap
  phase, or requirement behind it.
- The "registry" that *is* deferred in `research/local-semantic-store/` and
  `research/static-analysis-2.0/03-summary-store.md` is a **package-summary** registry (precomputed
  facts for third-party dependencies) — a completely different artifact.

The only sharing mechanism that exists is copy-paste, or making the pack a member of a shared Cargo
workspace (what this repo does for its own examples).

The product framing defends this: `README.md:33` — "polint ships no built-in policy rules; every
policy belongs to the repository that needs it." That is a coherent *v1* position. It is not a
coherent position for "world's most capable," because capability compounds through shared libraries.
CodeQL's power is not its evaluator, it is `codeql/java-all`. Semgrep's power is the registry.

### The versioning story is unsound today

- A consumer's generated manifest pins `polint = "0.1.17"` — a caret requirement, so cargo accepts
  any `0.1.z >= 0.1.17`. Since polint is pre-1.0, **every patch release is semver-permitted to break
  the SDK**, and `docs/RELEASING.md:37` says every release *is* a patch bump. There is no mechanism
  to signal a breaking SDK change.
- `--locked` is never passed to any child cargo invocation (`grep '--locked' crates/polint/src/`
  returns nothing), so a consumer's rule-pack dependency graph drifts on rebuild.
- No `#[deprecated]` anywhere in `crates/`. No deprecation policy.
- `#[non_exhaustive]` is used 17 times, all on internal types — **none on the SDK types rule authors
  actually match on**.
- Two doc pointers on exactly this topic are broken: `polint init` writes a `rust-toolchain.toml`
  citing `README.md#minimum-rust-version` (no such heading), and `docs/CONSUMER-SETUP.md:205` cites a
  README "Versions table" that does not exist.

### No sandboxing, and the compile cost is unmeasured

Rules run as an ordinary cargo-built child process with **full ambient privileges** — filesystem,
network, environment. Grepping for sandbox/wasm/wasi/rhai/lua/starlark/CEL across the repo returns
**one** hit: a single table row in `research/abstract-interpretation/implementation/EXTENSION-DOMAIN-CONTRACT.md:134`
noting WASM as a "future option if safety dominates expressiveness." Embedded scripting was never
considered. That is fine while rules are repo-local and authored by the repo owner. It becomes
disqualifying the moment rules are shared — which is the direction capability requires.

And the cost: a rule pack's only declared dependency is `polint`, which pulls **273 transitive
packages** including bundled SQLite (C), tree-sitter (C), and six `oxc_*` crates — built in
**release profile by default** (`cli/mod.rs:4184-4186`). **No measurement of cold build time exists
anywhere in the repo**; the only statements are qualitative hedges (`README.md:424-426`,
`docs/GITHUB-ACTION.md:61-64`). This is the single largest unmeasured latency in the consumer story,
and the project's own ADR already lists compile time as a revisit trigger
(`research/agent-rule-authoring/decisions/001-typed-rust-rules-not-dsl-first.md:86`).

---

## (d) Agent/LLM integration — strong at planning, blind at execution

polint is unusually thoughtful about being *consumed* by an agent. `--format ai-friendly` prints
counts by rule plus at most 10 examples, writes full JSON to `.polint/output/latest.json`, and the
skill teaches bounded `jq` queries instead of dumping the file into context
(`.claude/skills/polint/SKILL.md:25-35`). That context-budget discipline is rare and correct.

But the debugging loop has a hole shaped exactly like the hardest question:

| Agent question | Answerable today? |
|---|---|
| Did my signature derive the capabilities I meant? | **Yes** — `polint explain --rule <id> --format json` |
| Is a capability unsupported / setup-missing here? | **Yes** — `polint/capability` diagnostics, `analysis_plan.rs:539-579` |
| Where did the *engine* give up? | **Yes** — `polint inspect unknowns`, and it works even when the rule pack does not compile |
| My rule over-matched — which findings and why? | **Partly** — counts and examples, but `polint test`'s `ObservedDiagnostic` (`rule_test.rs:92-100`) drops evidence entirely |
| **My rule matched nothing — why?** | **No.** |
| **Why did this specific diagnostic fire?** | **No.** |

### The evidence system is built, excellent, and unreachable

`crates/polint/src/analysis/evidence/` is **4,335 lines** implementing exactly the provenance model a
credible engine needs: `EvidenceNodeFact` binding a node to file/function/MIR-op/CFG-node/place/
symbol/call-site with `status`, `precision`, `provenance`, `validation`, `confidence`;
`EvidenceBundleFact` keyed by `diagnostic_stable_key` with selected paths, slices, and a `replay_key`;
`EvidenceUnknownFact` with `reason ∈ {DynamicCall, UnsupportedEdge, SetupMissing, BudgetExceeded,
OpaqueSummary}`; `EvidenceOmittedRegionFact` with truncation reasons; and a renderer emitting a
bounded `evidence_v1` envelope with `total_*` vs `rendered_*` limits.

None of it reaches a user. Three independent confirmations:

1. `Diagnostic::with_structured_evidence_v1` (`diagnostics/mod.rs:936-938`) has **five call sites, all
   after `#[cfg(test)] mod tests`** at line 2303.
2. It is *actively erased* at the rule-host boundary — `diagnostics/mod.rs:1131-1142`:
   ```rust
   pub(crate) fn diagnostics_from_public_json_report(s: &str) -> Result<Vec<Diagnostic>, _> {
       let mut diagnostics = diagnostics_from_json_report(s)?;
       for diagnostic in &mut diagnostics {
           diagnostic.evidence_v1 = None;
           diagnostic.evidence_bundle = None;
       }
       Ok(diagnostics)
   }
   ```
   Every repo-local rule returns through this function.
3. The fields are `pub(crate)` on the serialized struct (`diagnostics/mod.rs:775-778`).

What the agent actually receives is `Vec<{label: String, value: String}>` — whatever the rule author
hand-attached via `.with_evidence(...)`. **The engine contributes nothing.** The one exception is
policy-query rules, which auto-attach five header fields (`sdk/policy.rs:152-165`) — but even there,
a taint `path` arrives as a single flattened string, not a structured chain.

`docs/facts/evidence.md:1-10` is honest about this ("Evidence is not a public SDK fact view in this
phase"). The problem is not dishonesty; it is that 4,335 lines of the best differentiator in the
codebase are switched off.

### The "silent rule" failure mode

`AiFriendlySummary.rules_triggered` is `by_rule.len()` (`diagnostics/mod.rs:1207`) — the count of
rules that produced ≥1 diagnostic. There is **no `rules_evaluated`, no zero-finding list, no
per-rule file-in-scope count**. A rule that matched nothing is indistinguishable, from the output
alone, from a rule that was never planned or whose `files` scope excluded everything. `facts sample`
returns only `{file, span, status, precision, stable_id}` — not the field values the rule matches on.
The practical fallback is `eprintln!` in your own rule, which works but is not a supported loop and
is not mentioned in the skill.

### Agent doc coherence is drifting

`polint add-skill` generates the skill from a hardcoded `format!` string
(`crates/polint/src/cli/skill.rs:182-459`), but the **checked-in `.claude/skills/polint/SKILL.md` has
already diverged from it** despite a shared last-touching commit: the checked-in copy omits
`polint inspect unknowns` entirely and states Go 1.24+ where the generator says 1.25+. The guard test
(`crates/polint/tests/cli.rs:598-620`) only asserts both contain a shared substring list — it never
asserts equality. Meanwhile `AGENTS.md:73` says "No project skills found" while the skill exists, and
`SKILL.md:279` names an `Evidence<'_>` fact view that does not exist.

---

## (e) What a "most capable" extension surface needs

Comparison of the four models, and where polint sits:

| System | Author surface | Distribution | Sandboxing | Interprocedural power | Iteration cost |
|---|---|---|---|---|---|
| CodeQL | QL (Datalog-ish, typed, OO) | `codeql/*-all` packs, versioned | N/A (declarative, no side effects) | Full — the library *is* the product | Slow DB build, fast requery |
| Semgrep | YAML patterns + taint mode | Public registry, thousands of rules | N/A (declarative) | Shallow interfile; Pro adds more | Very fast |
| ESLint | JS plugin, npm | npm, huge ecosystem | None (runs in process) | None (single-file AST) | Fast |
| Joern | CPGQL / Scala | Scripts, no registry | None | Full graph traversal | Medium |
| **polint** | **Typed Rust fn signature** | **None** | **None** | **Fixed policy-query shapes** | **Slow (release cargo build, unmeasured)** |

The decision to reject a DSL was made deliberately and is documented as ADR
`research/agent-rule-authoring/decisions/001-typed-rust-rules-not-dsl-first.md`, with five options
considered and explicit revisit criteria. **I think the decision was right and should be kept** — for
one reason the ADR undersells: an LLM writes Rust better than it writes a bespoke DSL it has never
seen, because the type checker gives it a verifier. A wrong QL query returns wrong results silently;
a wrong Rust rule fails to compile. That is a real, defensible advantage, and it is the strongest
argument for the current shape.

But the ADR's own revisit criteria are being approached from a direction it did not anticipate. It
lists "if compile times make iteration unacceptable" as a trigger — and compile time is *unmeasured*.
Measure it before the decision gets made for you.

**What must be added regardless of the DSL question:**

1. **A distribution unit.** Rule packs as ordinary crates.io/git crates, with an exactly-pinned
   `polint` dependency and `--locked`. This is mostly a policy + tooling change, not an architectural
   one, and it is the single highest-leverage unlock for capability compounding.
2. **Semver discipline before it matters.** Move to 1.0 for the SDK surface, or adopt an explicit
   `polint-sdk` crate with its own version line, so a breaking analysis change can be *expressed*.
   Add `#[non_exhaustive]` to SDK enums now, while it is free.
3. **Provenance as a product surface.** Stop stripping `evidence_v1` for the query families that
   already produce it. Every finding should carry a replayable path. This is the differentiator that
   incumbents structurally cannot match at low latency, and it is already written.
4. **Rule-execution telemetry.** For every planned rule emit
   `{rule_id, planned, capabilities_ok, files_in_scope, diagnostics_emitted}`. One field set converts
   the worst failure mode (silent rule, no signal) into a one-command diagnosis.
5. **A real provider/analysis trait**, so the extension protocol has something to plug into beyond
   four hard-coded sinks. See `01-layering-and-boundaries.md` and `04-analysis-core-capabilities.md`.
6. **Sandboxing, before distribution — not after.** If rule packs become shareable, arbitrary Rust
   with ambient authority is a supply-chain vector. The research already names the answer (subprocess
   first, WASM later); the sequencing matters more than the choice.

---

## Bottom line

polint's extension surface is a **beautifully disciplined front door on a building with no other
entrances.** The rule contract, the capability-from-signature derivation, the public-surface leak
gate, and the agent output discipline are all better than the incumbents'. But you cannot share a
rule, you cannot add an analysis, you cannot add a language, you cannot see why a finding fired, and
you cannot see why one didn't.

The three fixes with the best capability-per-effort ratio, in order:

1. **Un-strip evidence** (`diagnostics/mod.rs:1136-1139`) — turns 4,335 already-written lines into the
   product's headline differentiator.
2. **Rule-execution telemetry** — closes the worst agent-loop failure mode.
3. **Shareable rule packs with pinned versioning** — the only path by which capability compounds
   instead of being re-typed per repository.

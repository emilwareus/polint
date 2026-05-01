# Phase 9: Plugin Skeleton - Context

**Gathered:** 2026-05-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 9 delivers a clean, explicitly experimental Wasm plugin boundary for future repo-local rules. It should add or harden WIT interface files, a Wasmtime host-loading skeleton, manifest/component validation, documentation, and focused tests. It must not block v1 usefulness by attempting full repo-local rule compilation, automatic plugin execution, broad runtime scheduling, or full AST transfer into plugins.

</domain>

<decisions>
## Implementation Decisions

### Experimental Scope

- **D-01:** `[auto]` Keep Phase 9 explicitly experimental. The goal is a credible skeleton and boundary, not a production-ready plugin runtime.
- **D-02:** `[auto]` Do not add automatic repo-local Rust rule compilation, source-hash artifact caching, plugin execution in `polint check`, or dynamic registration in this phase.
- **D-03:** `[auto]` Preserve the v1 user path through built-in SDK examples and native scaffolding. Plugin work should be additive and should not change existing CLI behavior unless tests require a small validation helper.

### WIT and Host Query Surface

- **D-04:** `[auto]` Shape WIT around the Wasm Component Model, rule metadata, capability declarations, diagnostic reporting, and host fact queries by stable IDs.
- **D-05:** `[auto]` Host APIs should expose narrow facts such as file path, function name, branch condition, and diagnostic report calls. Do not serialize full ASTs, source files, or graph payloads into plugin memory.
- **D-06:** `[auto]` Keep the WIT contract small and versionable. Prefer a `rule.wit` foundation that can grow later over a broad speculative API.

### Loader and Manifest Validation

- **D-07:** `[auto]` `polint-plugin` should own the manifest model and host skeleton. Manifest validation should check required metadata and component path existence, with structured typed errors where practical.
- **D-08:** `[auto]` Wasmtime component-byte validation should remain behind the existing optional `wasmtime-host` feature so normal workspace builds stay lightweight.
- **D-09:** `[auto]` The skeleton may validate component bytes, but it does not need to instantiate, invoke, or schedule plugin rules in Phase 9.

### Documentation and Truthfulness

- **D-10:** `[auto]` Documentation must clearly mark repo-local Wasm rules as experimental and future-facing.
- **D-11:** `[auto]` Docs should explain the intended stable-ID host API and why plugins must query host facts instead of receiving large AST JSON blobs.
- **D-12:** `[auto]` Any README or module docs must avoid claiming that Wasm plugins are run by `polint check` or that repo-local rules are automatically compiled in v1.

### Test Proof

- **D-13:** `[auto]` Add focused unit tests for WIT contents, manifest parsing, missing component paths, experimental gating, and component-byte validation where the optional feature is enabled.
- **D-14:** `[auto]` Keep tests structured and deterministic. Prefer asserting typed errors and known WIT strings over broad substring-only checks.
- **D-15:** `[auto]` Full Phase 9 verification should include `cargo fmt -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. If feature-specific Wasmtime tests are added, run the relevant feature test explicitly.

### the agent's Discretion

- The exact WIT function names and package/world organization, as long as metadata, capabilities, diagnostics, and stable-ID host fact queries are represented.
- Whether to add a tiny example Wasm rule artifact or defer it if it would require brittle build tooling or distract from the skeleton.
- Whether docs live in README, crate docs, or a dedicated example README, provided the experimental status and stable-ID host API are clear.
- How to split plans between WIT, loader, docs, tests, and verification.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project and Requirements

- `.planning/PROJECT.md` — Project value, constraints, and out-of-scope boundaries for repo-local rule support.
- `.planning/REQUIREMENTS.md` — Defines `PLUG-01` and `PLUG-02`.
- `.planning/ROADMAP.md` — Phase 9 goal and success criteria.
- `docs/INITIAL_PROMPT.md` §Plugin architecture target — Requires Wasm Component Model, WIT, stable-ID host APIs, sandboxing, and no huge AST JSON payloads.

### Existing Plugin Skeleton

- `crates/polint-plugin/src/lib.rs` — Current manifest model, experimental `PluginHost`, and optional Wasmtime component-byte validation.
- `crates/polint-plugin/src/rule.wit` — Current WIT package/world skeleton.
- `crates/polint-plugin/Cargo.toml` — Existing optional `wasmtime-host` feature and dependencies.

### Research Baseline

- `.planning/research/STACK.md` — Confirms `wasmtime` and `wit-bindgen` as the intended plugin stack and warns to keep the skeleton experimental.
- `.planning/research/PITFALLS.md` — Calls out plugin scope creep as a pitfall and recommends experimental WIT/host skeleton only.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `crates/polint-plugin/src/lib.rs`: Already exposes `RULE_WIT`, `WasmRuleManifest`, `PluginHost::experimental`, `load_manifest`, and optional `validate_component_bytes`.
- `crates/polint-plugin/src/rule.wit`: Already defines a `polint:rule` package, `host` interface, stable ID types, metadata/capability exports, and a `run` export.
- `crates/polint-core/src/lib.rs`: Existing `Capabilities`, stable fact IDs, `AnalysisDb`, and `RuleCtx` shape should guide host query names and stable-ID semantics.
- `README.md`: Already lists repo-local Wasm rule compilation and caching as roadmap/future work.
- `docs/INITIAL_PROMPT.md`: Contains the strongest source-of-truth wording for the plugin target and out-of-scope first implementation.

### Established Patterns

- Keep features honest and bounded; previous phases explicitly avoided claiming dynamic plugin loading or full semantic coverage.
- Use typed errors and structured tests where behavior matters.
- Optional heavyweight dependencies should stay feature-gated when normal workspace checks do not need them.
- Diagnostics and facts should flow through stable IDs and host-owned state rather than copying large source or AST payloads.

### Integration Points

- `polint-plugin` is already a workspace crate and dependency of `polint-cli`, so tests can validate the crate without changing `polint check`.
- Future plugin execution should eventually connect to the same `Rule`, `RuleMeta`, `Capabilities`, `RuleCtx`, and diagnostic concepts established in `polint-core`/`polint-sdk`.
- Plugin docs should connect to the existing `polint new-rule` native scaffolding by explaining that Wasm repo-local rules are experimental and not the v1 default path.

</code_context>

<specifics>
## Specific Ideas

- Keep the skeleton small enough that Phase 10 can document it honestly.
- Prefer "experimental Wasm boundary" wording over "plugin system complete".
- Treat the current `rule.wit` as a starting point, but harden it if it does not clearly represent metadata, capabilities, diagnostics, and host queries.

</specifics>

<deferred>
## Deferred Ideas

- Automatic repo-local Rust rule compilation to Wasm and artifact caching by source hash, SDK version, and target triple — future `SEM-03` work.
- Running Wasm plugins as part of `polint check` — future plugin runtime phase after the skeleton is stable.
- A large host query API, full AST transfer, semantic graph resolution, or cross-language plugin SDK generation — future work only after v1 boundaries are proven.

</deferred>

---

*Phase: 09-plugin-skeleton*
*Context gathered: 2026-05-01*

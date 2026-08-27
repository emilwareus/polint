Create an extensive, implementation-grade plan for emilwareus/polint's code-preserving build architecture.

**Later product decision (2026-08-27):** supersedes the compatibility parts of
this brief. The migration ships directly in the proposed 0.3.0 unstable minor:
no legacy backend, automatic legacy fallback, multi-release compatibility
window, or deprecation period. Users upgrade polint and migrate the rule-pack
manifest/build contract; rule `.rs` source and the typed Rust API remain stable.

Repository: /workspace/polint, main/origin/main at b272b378 (v0.2.1), clean working tree. Existing research report: /opt/data/research/polint-builds-code-preserving-2026-08-25/report.md. Read it fully and verify its claims against the repository before planning.

Core product invariant (non-negotiable): polint rules remain real, expressive, typed Rust code. Do NOT make a DSL/declarative policy the primary model. Existing rule source should remain byte-identical if technically possible: #[polint::rule], polint::sdk::prelude::*, typed fact-view parameters, RuleCtx, RuleResult, run_cli registration, normal Rust helpers/control flow. Optional code-generating conveniences are allowed only as secondary features.

Problem: today a customer scan with repo-local rules shells out to cargo run and recompiles the whole ~238k-line polint engine and dependency closure. Repository evidence reports ~223 compiled units, ~185.4s cold build, ~537MB target retention. The proposed direction is a prebuilt polint engine host plus a thin SDK and a small per-repo Rust rule binary communicating through a fact-snapshot protocol. The host performs parsing/providers/fact production; the rule binary links only the thin SDK, deserializes a snapshot once, and uses borrowed typed views. Add fingerprinting so unchanged rule artifacts run directly without Cargo. Add prebuilt artifacts for non-authors/CI and WASM later as a portable sandboxed distribution backend, not as the primary authoring path.

Produce the plan at exactly:
/opt/data/research/polint-builds-code-preserving-2026-08-25/implementation-plan.md
Create parent directories if needed. Do not modify /workspace/polint. Do not implement code, create branches, commits, PRs, or edit repository files. This is planning/documentation only.

The plan must be detailed enough for approved coding agents to implement without architectural guessing. It must contain:

1. Executive goal and invariants, including what must remain unchanged for rule authors and what is explicitly out of scope (DSL-first, native cdylib ABI, remote-first execution).
2. Current architecture and exact call/data/build path with repository paths and line ranges.
3. Target package graph. Resolve the Cargo cycle question explicitly: likely polint-sdk (thin), polint-engine (current heavy runtime), polint facade, and polint-macros. Explain alternatives and choose one. Track package count/public API changes.
4. Exact boundary design: what types/modules move to polint-sdk; what stays in engine; how AnalysisDb becomes/produces an owned FactSnapshot; lifetime/borrow preservation; serialization format and schema/version/digests; capability support metadata; review changesets; options; diagnostics; stable keys; fact-family sections; avoiding whole fact DB serialization.
5. Rule protocol design: manifest handshake, run request, snapshot transfer, stdout/stderr limits, timeouts, exit/error protocol, version negotiation, determinism, cancellation, one process vs two processes, and compatibility with current report/inspect JSON schemas.
6. Build and artifact lifecycle: source fingerprint inputs, current-artifact detection, direct binary execution bypassing Cargo, single build for all fixture cases, Cargo flags, offline/locked operation, vendored SDK, target directory location/cleanup, user-level cache, disk ceilings/LRU, prebuilt native artifacts, signing/digests, and explicit native trust mode.
7. Security threat model and controls: customer-controlled Cargo.toml, build.rs, proc macros, dependencies, rule binary, snapshot files, path traversal, untrusted fresh repos, artifact signatures, sandbox boundaries, and what native mode can/cannot guarantee. Include default behavior for owned repositories, shared artifacts, and arbitrary untrusted repositories.
8. Detailed phased implementation plan. Use phases and bite-sized tasks with exact files likely to change/create, dependencies, sequencing, prerequisites, development rollback strategy, the direct breaking migration, and acceptance criteria. Include at minimum:
   - measurement/baseline harness;
   - dependency-closure and feature-leak guard;
   - SDK extraction;
   - FactSnapshot and serialization;
   - host/rule protocol;
   - rule build fingerprint/cache and direct execution;
   - runner/CLI integration;
   - test harness/golden equivalence;
   - docs/action/release updates;
   - optional prebuilt artifact path;
   - later WASM backend decision gate.
9. Testing/verification matrix with exact commands and expected assertions. Include unit, integration, golden, public API leak, capability, determinism, corruption/version mismatch, offline/no-Cargo, cross-platform, security/failure, and performance tests. Make no unsupported claims about tests passing.
10. Performance budgets and experiment design. Separate measured existing data, targets, and kill criteria. Include 2 vCPU/4GB clean machine and normal developer machine; cold/warm; small/medium/large repos; bytes downloaded/written/retained, CPU/RSS, rule compile time, host analysis time, snapshot serialization/deserialization, startup, repeated scans, and number of Cargo invocations.
11. Direct migration and release plan: proposed 0.3.0 version, generated and existing manifests, a one-shot rewrite aid, rebuild/test instructions, actionable errors for unmigrated packs, release gates, and proof that no legacy backend or auto-fallback ships. The all-at-once manifest/build migration is intentional; preserve rule `.rs` source/API instead of the 0.2 package/execution contract.
12. Documentation plan: files and claims to update, including README, consumer setup, architecture, action docs, generated skill, schemas, examples, and troubleshooting.
13. Risks, unresolved decisions, and an implementation decision log template.
14. Final recommended order of execution and what the first implementation PR should be.

Use exact paths and symbols from the repository. Do not write pseudo-code where an API shape/field list is needed. Keep the plan focused on implementation decisions, not generic project-management prose. Clearly distinguish decisions already made by this brief from decisions that require benchmark evidence. Include a traceability table mapping each user requirement to plan phases and acceptance tests.

Before finishing, verify the plan exists, is UTF-8 Markdown, is extensive, and /workspace/polint remains unchanged. Return the exact path, byte count, section list, and repository git status.

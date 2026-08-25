Research-only task for emilwareus/polint at /workspace/polint, currently main/origin/main commit b272b378 (v0.2.1). Emil rejected the previous recommendation to make repo policies primarily a DSL/declarative format. The core product value is that policies are real code: rules are expressive Rust code, typed against polint facts, understandable and controllable by engineers and AI agents. We must preserve that value.

Investigate whether there is a better build/distribution/execution architecture that keeps rules as code essentially as they are today, while eliminating or drastically reducing customer scan-time Cargo compilation, target directories, binaries, and cold-start latency. The user wants initial scans very fast and reliable on low-powered computers with minimal customer disk use.

Write a rigorous research report at exactly:
/opt/data/research/polint-builds-code-preserving-2026-08-25/report.md
Create the parent directory if needed. Do not modify /workspace/polint or any repository files. Do not create commits, branches, PRs, or implementation code. Research and report only.

Use Claude Code Max / Opus maximum reasoning effort. Start from repository evidence. Inspect current CLI subprocess path, rule-pack scaffolding, polint runner/SDK/macro, Cargo manifests/features/toolchain, cache layout, release artifacts, extension protocol, architecture decisions, tests, and docs. Then investigate external primary sources as available. Do not assume the previous report's declarative-first or WASM-first conclusion is correct. Challenge it.

The report must explicitly investigate and compare code-preserving options, including:
- Ship the engine as a prebuilt binary and compile only a tiny per-repo rule crate against a stable lightweight SDK, rather than linking the whole engine. Analyze whether polint can split into a thin SDK + host protocol and what the actual Cargo closure would be.
- Precompile/cache the polint engine as a reusable Cargo dependency or artifact while compiling only customer rule code; distinguish compiler cache reuse from final target retention and cold-start behavior.
- Build a generic precompiled rule-host executable once, and load repo rules as code through a stable external protocol, subprocess, or generated registration/IPC mechanism.
- Compile rules as native plugins, cdylibs, or platform-specific executables, including ABI/versioning and security.
- Compile rules as WASM while preserving Rust code as the authoring model; assess whether this can maintain the current typed rule API and whether it is actually the best answer.
- Use Rust incremental compilation, sccache, cargo-binstall/cargo-dist, prebuilt per-version SDK artifacts, PGO/LTO/profile changes, or a server/daemon to reduce initial cost without changing rule semantics.
- A hybrid where rules remain Rust code but are built in CI/author environments, then shipped as signed portable code artifacts; clearly distinguish what the customer must do versus the author.
- Any alternatives from ecosystems such as rust-analyzer, Clippy/Dylint, cargo-geiger, golangci-lint plugins, CodeQL packs, Buck/Bazel remote cache, Nix, or similar, only where relevant.

Answer:
1. What exactly must be compiled today and why.
2. Which parts of the current polint crate force the large closure and which can be split out.
3. Whether preserving rules-as-Rust is compatible with no scan-time Cargo at all. Be precise: native Rust code cannot run without being compiled for the target somehow, so identify where compilation moves and what portability/security costs result.
4. Whether the actual optimal product is: prebuilt host + small code artifact, host-managed rule process, WASM Rust code, prebuilt native rule packs, or another design.
5. Keep typed facts, capability derivation, diagnostics, rule tests, determinism, and AI-friendly authoring. Do not recommend a DSL as the primary rule model. A declarative subset may be discussed only as optional convenience, not the core product.
6. Recommend the smallest viable architecture and migration path that preserves today's user experience as much as possible.
7. Propose concrete experiments and budgets for cold scan latency, bytes downloaded/written/retained, CPU/RAM, rule startup, offline operation, cross-platform behavior, and compilation frequency. Do not invent benchmark results.
8. Include security/trust-boundary treatment of customer-controlled Rust, build.rs, proc macros, native plugins, and WASM.

Important nuance: distinguish these product scenarios:
A. A team authors and reviews its own Rust rules, then scans locally/CI.
B. A customer receives a prebuilt/shared rule pack from another party.
C. An agent scans an arbitrary fresh repository that may contain untrusted rules.
The best choice may differ by scenario. State the recommended default for each.

Use exact repository paths/line ranges for internal evidence. Use primary external URLs and label any external claim that could not be fetched or verified. Include a decision matrix, explicit rejection reasons, and kill criteria.

Report structure:
- Executive recommendation
- User correction and product invariant: rules remain code
- Current-state evidence
- Product scenarios and constraints
- Code-preserving alternatives comparison
- Recommended target architecture
- What changes in the repository and what stays
- Phased migration plan
- Experiment plan, budgets, and kill criteria
- Security and trust boundaries
- Risks and unresolved decisions
- Conclusion
- Sources

Before finishing, verify the report exists at the exact path, is readable UTF-8 Markdown, contains all required sections, and /workspace/polint remains unchanged. Return the absolute report path, size, and verification details.
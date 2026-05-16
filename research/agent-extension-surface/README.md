# Agent Extension Surface Research

Date: 2026-05-15

This track researches how polint should let users and AI agents extend the analysis engine with Rust code, not only write diagnostic rules.

The product goal is higher than generic linting:

```text
repo-local Rust rules
  + repo-local Rust analysis extensions
  + typed engine facts
  + explicit uncertainty
  + validation and provenance
  = tailored scan accuracy
```

The key conclusion is that polint should keep the existing rule-authoring path for diagnostics, then add a second, more powerful extension path for analysis facts. Rules should read final facts and report diagnostics. Extensions should improve the facts that later analyses and rules consume.

## Core Recommendation

Add repo-local Rust **analysis extension crates** under `.polint/extensions/`. Compile and run them as process-isolated executables using a versioned host protocol. They should emit typed, validated, provenance-labeled facts into controlled sinks:

- entrypoints and framework lifecycle facts;
- call graph edges and call-resolution models;
- data-flow sources, sinks, sanitizers, barriers, additional steps, and summaries;
- effect summaries;
- type, alias, and value hints;
- framework and generated-code semantics.

Do not load arbitrary Rust dynamic libraries into the polint process as the first extension mechanism. Dylint proves dynamic Rust lint libraries are possible, but it also shows the cost: exact toolchain coupling, unstable compiler internals, in-process crash risk, and public leakage of low-level APIs. polint should copy Dylint's repo-local Rust ergonomics, not its ABI shape.

## Files

- `FINAL-REPORT.md`: synthesis and main conclusions.
- `RECOMMENDED_IMPLEMENTATION.md`: concrete product and implementation path.
- `RESEARCH-ANALYSIS.md`: detailed analysis of the extension systems and accuracy implications.
- `REPO-INDEX.md`: cloned repositories and exact commits inspected.
- `PAPER-INDEX.md`: research and documentation sources used.
- `STANDARD.md`: comparison structure for extension systems.
- `VALIDATION.md`: how to validate extensions and this research.
- `algorithms/extension-lifecycle.md`: lifecycle algorithms in Python-ish pseudocode.
- `implementation/polint-extension-surface-path.md`: staged implementation plan for polint.
- `oss/implementation-comparison.md`: OSS implementation comparison.

## Relationship To Existing Research

This track depends on the call graph and data-flow research:

- `research/call-graphs/AGENT-EXTENSIBLE-CALL-GRAPHS.md`
- `research/data-flow/AGENT-EXTENSIBLE-DATA-FLOW.md`

The extension surface is the way those analyses become repo-aware. Generic algorithms provide defaults; repo-local extensions provide the missing framework, lifecycle, and domain semantics.

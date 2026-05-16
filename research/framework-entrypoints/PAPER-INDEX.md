# Paper Index

Papers were downloaded into `research/framework-entrypoints/papers/`.

## Core Framework Modeling Papers

### F4F: Taint Analysis of Framework-based Web Applications

- Local file: `papers/f4f-oopsla-2011.pdf`
- Authors: Manu Sridharan, Shay Artzi, Marco Pistoia, Salvatore Guarnieri, Omer Tripp, Ryan Berg.
- Venue: OOPSLA 2011.
- Public page: <https://2011.splashcon.org/details/oopsla-2011-papers/61/F4F-Taint-Analysis-of-Framework-based-Web-Applications>
- DOI: <https://doi.org/10.1145/2048066.2048145>
- PDF source: <https://manu.sridharan.net/files/oopsla11-f4f-preprint.pdf>

Key lesson:

F4F separates framework-behavior specifications from the core taint engine. It processes application code and configuration to generate WAFL specifications, then taint analysis consumes those specifications. The paper reports 525 additional issues across nine benchmarks and a harmonic mean of 2.10x more issues per benchmark with framework support.

For polint:

- Strong support for a separate framework fact/model layer.
- Strong support for config/code evidence and generated specs.
- Strong warning against hardcoding every framework into core analysis.
- Do not overclaim: F4F is Java web taint analysis, not a universal multi-language solution.

### AUTOWEB: Automatically Inferring Web Framework Semantics via Configuration Mutation

- Local file: `papers/autoweb-2024.pdf`
- PDF source: <https://leehaofeng.github.io/papers/2024-AutoWeb.pdf>

Key lesson:

AutoWeb treats framework behavior as relations introduced by configuration: entrypoints, points-to relations, and call relations. It infers minimal sufficient and necessary configuration sets by mutating configuration and observing runtime relations. The paper reports an 8.2% false-negative rate and no false positives in its studied Java web frameworks.

For polint:

- Strong support for representing framework behavior as typed relations/facts.
- Strong support for config/decorator/annotation evidence and validation.
- Good long-term idea for agent-assisted model validation.
- Do not make mutation-based dynamic inference the default. It requires runnable apps, representative executions, and has scope limits.

### Dynamically Generating Callback Summaries For Enhancing Static Analysis

- Local file: `papers/callback-summaries-ecoop-2024.pdf`
- Public page: <https://drops.dagstuhl.de/entities/document/10.4230/LIPIcs.ECOOP.2024.4>
- Source/artifact link from paper: <https://github.com/Fraunhofer-SIT/DynamicCallbackSummaries/>

Key lesson:

CGMiner dynamically generates callback summaries for libraries. The summary captures which API calls trigger which callbacks. Integrated with FlowDroid, callback summaries uncover more data flows than the baseline without summaries. The paper reports more than 94% correct edges in summary generation and 40% more data flows with CGMiner summaries plus data-flow mappings in one evaluation.

For polint:

- Model callback registration and invocation as summaries/facts.
- Callback facts need both control-flow edges and data-flow mappings.
- Dynamic summaries are under-approximations; label them `RuntimeDerived`.
- This supports framework dispatch overlays, not direct base call graph mutation.

### BackDroid

- Local file: `papers/backdroid-2020.pdf`

Key lesson:

BackDroid is relevant for sink-directed Android analysis and avoiding unnecessary whole-app analysis. It reinforces that targeted, demand-driven analysis can beat blanket whole-program analysis for specific queries.

For polint:

- Useful support for demand-driven/path-directed future data flow.
- Do not generalize its Android-specific performance claims to Go/TS/JS without benchmarks.

## Agent And Protocol Boundary Papers

### Unsafe by Flow: Uncovering Bidirectional Data-Flow Risks in MCP Ecosystem

- Local file: `papers/mcp-biflow-2026.pdf`
- arXiv: <https://arxiv.org/abs/2605.07836>

Key lesson:

MCP-BiFlow frames MCP server security as bidirectional trust-boundary data flow. It recovers MCP-specific entrypoints, models request-side and return-side taint semantics, and performs interprocedural propagation. The paper reports 30/32 confirmed MCP vulnerability cases detected and real-world findings across 15,452 repositories.

For polint:

- MCP tools/resources/prompts are first-class entrypoints.
- Return-side outputs are security boundaries, especially for AI agents.
- Protocol-specific entrypoint recovery is mandatory for MCP.
- Be careful with real-world precision claims; candidate clusters and confirmed paths are not a simple precision denominator.

### TaintP2X

- Local file: `papers/taintp2x-icse-2026.pdf`

Key lesson:

This paper is relevant to AI-era source/sink and prompt/tool-flow modeling. It supports treating prompts, tool inputs/outputs, and agent boundaries as security-relevant data-flow surfaces.

For polint:

- Add prompt/tool/agent boundaries to the future source/sink vocabulary.
- Treat generated models as heuristic until validated.

### Taint-AWI

- Local file: `papers/taint-awi-2026.pdf`

Key lesson:

This paper is relevant to workflow and automation boundaries. It supports the broader point that modern codebases have non-HTTP entrypoints and trust boundaries: CI workflows, automation scripts, agent actions, and generated dispatch.

For polint:

- Include workflow/automation entrypoints in the long-term boundary model.
- Cite as current research direction, not settled engineering evidence.

## What The Papers Support

Supported strongly:

- Framework behavior must be modeled outside ordinary direct call graph construction.
- Entrypoints, call relations, points-to/injection relations, sources, sinks, and callbacks should be typed facts.
- Configuration/decorators/annotations/manifests are primary evidence, not side data.
- Validation and provenance are required because framework models can be wrong.
- Callback/lifecycle semantics should feed reachability and data flow.
- MCP and AI-agent tool protocols introduce new first-class boundaries.

Supported with qualifications:

- Automatic model inference can help, but is not solved generally.
- Dynamic summaries can improve precision, but are under-approximate.
- Strong benchmark recall in one ecosystem does not prove broad multi-language accuracy.
- AI-generated models can improve coverage, but must remain labeled until validated.

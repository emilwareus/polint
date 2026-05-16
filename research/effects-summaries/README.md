# Function Effects And Summaries Research

Date: 2026-05-16

This folder researches function effects and summaries as the scaling boundary for
polint's multi-language analysis engine.

The core conclusion is that summaries should be an internal typed analysis
substrate, not a flat "effect bag" and not the first public SDK primitive.
Call graphs, data flow, alias queries, framework overlays, and agent-authored
extensions should all consume summary domains through stable typed views.

## Files

- [FINAL-REPORT.md](FINAL-REPORT.md): main synthesis and recommendations.
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md): implementation path for polint.
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md): algorithms, accuracy, complexity, and rejected paths.
- [STANDARD.md](STANDARD.md): standardized vocabulary and report schema for summary/effect research.
- [REPO-INDEX.md](REPO-INDEX.md): cloned implementation repositories and inspected source paths.
- [PAPER-INDEX.md](PAPER-INDEX.md): papers, docs, and downloaded artifacts.
- [SUBAGENT-FINDINGS.md](SUBAGENT-FINDINGS.md): consolidated parallel research findings.
- [VALIDATION.md](VALIDATION.md): reference-validation and benchmark plan.
- [algorithms/SUMMARY-ALGORITHMS.md](algorithms/SUMMARY-ALGORITHMS.md): pseudo-code for the core algorithms.
- [tools/IMPLEMENTATION-REPORTS.md](tools/IMPLEMENTATION-REPORTS.md): detailed tool-by-tool implementation notes.

## Ignored Clones

Reference implementations were cloned under `research/effects-summaries/repos/`.
That path is gitignored by the existing `research/*/repos/` rule.

# Validation

## Research Inputs

Subagents researched:

- CodeQL query ergonomics, model packs, path queries, and tests.
- Semgrep rule syntax, taint mode, fix behavior, and tests.
- Joern/CPGQL, Kythe, SCIP, LSIF, and evidence paths.
- Pysa/Pyre, Infer/Pulse, CodeQL/Semgrep modeling systems.
- ESLint, typescript-eslint, Ruff, Clippy, Go `analysis`, OpenRewrite,
  ArchUnit, and mainstream lint SDK ergonomics.
- AI-agent rule/model/provider authoring loops and recent LLM static-analysis
  papers.
- Adversarial review of the draft conclusions.
- Rule testing, packaging, and fixture layout.

## Local Repositories

The following local repositories were cloned under the ignored
`research/agent-rule-authoring/repos/` folder:

- CodeQL
- Semgrep
- ESLint
- Go tools
- Joern
- Pyre/Pysa
- Ruff
- OpenRewrite

The directory is ignored by `.gitignore` via:

```text
research/*/repos/
```

## Downloaded Papers

Downloaded PDFs:

- `papers/qlcoder-2025.pdf`
- `papers/knighter-2025.pdf`
- `papers/iris-llm-inferred-taint-specs-2024.pdf`
- `papers/semtaint-2026.pdf`
- `papers/rulellm-2025.pdf`

## Validation Checks To Run

```sh
file research/agent-rule-authoring/papers/*
git check-ignore -v research/agent-rule-authoring/repos/codeql
git check-ignore -v research/agent-rule-authoring/repos/eslint
git diff --check -- research/README.md research/ROADMAP.md research/agent-rule-authoring
LC_ALL=C rg -n "[^\\x00-\\x7F]" research/agent-rule-authoring
git status --short --untracked-files=all
```

## Findings Validated Against Current polint

The research was checked against the current codebase:

- `crates/polint-macros/src/lib.rs` already validates plain `#[polint::rule]`
  function shape and derives capabilities from fact-view parameters.
- `crates/polint/src/sdk/facts.rs` already exposes typed fact views.
- `docs/facts/capability-plans.md` already documents capability derivation and
  unsupported future capabilities.
- Examples under `examples/*/.polint/rules` already use repo-local rule packs.

Therefore the recommendation extends the existing direction rather than
replacing it.

## Validation Result

Validation performed on 2026-05-16:

- `file research/agent-rule-authoring/papers/*` confirmed all downloaded papers
  are PDFs.
- `git check-ignore -v research/agent-rule-authoring/repos/...` confirmed the
  cloned implementation repositories are ignored by `.gitignore`.
- `git diff --check -- research/README.md research/ROADMAP.md research/agent-rule-authoring`
  completed with no whitespace errors.
- `LC_ALL=C rg -n "[^\\x00-\\x7F]" research/agent-rule-authoring` returned no
  non-ASCII text in the new research folder.
- `git status --short --untracked-files=all` listed only research docs and
  downloaded PDFs, not ignored cloned repositories.

# AGENTS.md - Research Instructions

These instructions apply to all work under `research/`.

polint research is groundwork for a native, multi-language static-analysis
engine that AI agents can extend with repo-local Rust code. The job is not to
produce plausible prose. The job is to build a traceable evidence base that
humans and future agents can inspect, challenge, validate, and turn into
implementation.

## Default Standard

Work very hard.

Do not stop after the first plausible answer, the first popular OSS repository,
or the first clean summary. Research until the question is answered with
adequate evidence, or until the remaining uncertainty is explicit and
documented.

For every non-trivial topic:

- Define the research question before collecting sources.
- Explain why the question matters for polint's engine, SDK, extensions, or
  roadmap.
- Search broadly enough to avoid one-source or one-language bias.
- Prefer primary sources: papers, official language/tool docs, specifications,
  and implementation source code.
- Clone and inspect relevant OSS implementations when they materially implement
  the algorithms being studied.
- Download relevant papers/docs/source material when legally and operationally
  appropriate.
- Separate facts from interpretation and product recommendations.
- Record source metadata, cloned repo commits, inspected paths, and access
  dates.
- Preserve uncertainty, contradictions, rejected approaches, and open questions.
- Summarize results for both humans and future AI agents.
- Promote only validated conclusions into the research roadmap or root docs.

No major claim about algorithms, accuracy, complexity, language semantics,
implementation architecture, extension safety, or benchmark quality should exist
without a source trail or an explicit confidence label.

## GSD And Research Edits

Research-only work under `research/` may be done directly when the user asks for
research or asks to update research documents. Do not start a GSD workflow for
research unless the user explicitly asks for it.

If research leads to product code changes outside `research/`, follow the
repository-level workflow and Rust/API instructions.

## Standard Topic Structure

Use this topic layout unless the existing folder already has a stronger local
structure:

```text
research/<topic-slug>/
  README.md
  FINAL-REPORT.md
  RECOMMENDED_IMPLEMENTATION.md
  RESEARCH-ANALYSIS.md
  STANDARD.md
  REPO-INDEX.md
  PAPER-INDEX.md
  VALIDATION.md
  SUBAGENT-FINDINGS.md
  algorithms/
  benchmarks/
  decisions/
  domains/
  implementation/
  languages/
  papers/
  repos/        # gitignored
  tools/
```

`README.md` states the question, why it matters, current status, scope, and
short summary.

`FINAL-REPORT.md` is the main synthesis. It should lead with the practical
conclusion, then show the evidence and tradeoffs.

`RECOMMENDED_IMPLEMENTATION.md` turns research into an implementation path. It
must be specific enough to prevent obvious architectural dead ends.

`RESEARCH-ANALYSIS.md` covers algorithms, accuracy, complexity, failure modes,
and rejected paths.

`STANDARD.md` defines vocabulary and report structure for that research family.

`REPO-INDEX.md` records every cloned implementation, commit, URL, reason for
inspection, and key source paths.

`PAPER-INDEX.md` records downloaded papers, official docs, specs, access dates
where useful, and source status.

`VALIDATION.md` records how the findings should be checked: fixtures,
benchmarks, differential tests, law checks, determinism checks, cache checks, and
open validation gaps.

`SUBAGENT-FINDINGS.md` consolidates intermediate agent findings, critiques, and
second-pass validation.

`implementation/` stores implementation-ready contracts, bootstrap sequences,
kernel specs, API sketches, cache/invalidation details, and extension contracts.

`papers/` stores downloaded papers/docs that are legal and appropriate to keep.

`repos/` stores cloned reference implementations. This path is intentionally
gitignored by `research/*/repos/`; do not commit cloned repositories.

## Research Protocol

Use research deliberately, not casually.

1. Start with a protocol.

   Write down the research question, scope, inclusion criteria, exclusion
   criteria, target languages, expected source types, search terms, and expected
   deliverables.

2. Search in layers.

   Start with primary sources: official docs/specs, papers, and source code.
   Then use high-quality secondary references for orientation or comparison.

3. Inspect implementations.

   For state-of-the-art implementation research, clone the relevant OSS code
   into `research/<topic>/repos/`, record the exact commit, inspect the actual
   source files, and cite key paths in `REPO-INDEX.md`.

4. Read the research.

   Download and index the papers/docs that define the algorithms. Do not rely
   only on blog posts, README claims, or library marketing pages.

5. Compare by language and by algorithmic family.

   polint targets multiple languages. Always ask what differs for Go, TS/JS,
   Python, JVM/Java, Rust, and future adapters.

6. Record the search path.

   Keep enough source metadata that another agent can reconstruct the evidence
   trail: URLs, local paths, cloned commits, inspected source files, access
   dates for web material, and rejected leads.

7. Validate and revise.

   Do a second pass after synthesis. Look for broad unsupported claims,
   stale links, missing citations, contradicted recommendations, circular
   dependencies, and implementation traps.

## Source Priority

Prefer sources in this order when the question allows it:

1. Foundational and recent peer-reviewed research papers.
2. Official language specs, compiler/toolchain docs, and standards.
3. Source code for mature implementations that actually implement the
   algorithms under study.
4. Official project documentation for those implementations.
5. Benchmark suites, test suites, and conformance corpora.
6. Vendor/project blogs used only as supporting context unless they expose
   implementation detail not available elsewhere.
7. News, forum posts, social media, and AI-generated material only as leads,
   never as final authority.

For current tools, repositories, package managers, language specs, and benchmark
sets, verify current facts rather than relying on memory.

## Source Capture

Every important source entry should be recoverable.

For papers/docs in `PAPER-INDEX.md`, include:

```text
File:
Source URL:
Publisher / project:
Authors / responsible organization:
Publication or revision date:
Access date:
Source type: paper | official docs | spec | benchmark | vendor docs | other
Local file:
Status: candidate | downloaded | summarized | extracted | rejected
Short note:
```

For repos in `REPO-INDEX.md`, include:

```text
Repo:
Commit:
URL:
Why inspected:
Key source paths:
Relevant algorithms/domains:
Notes and caveats:
```

Use short excerpts only when necessary and legally safe. Prefer summaries and
links to local files/source paths.

## Downloading And Cloning

Download relevant source material when it materially improves traceability and
when it is legal and appropriate to store it in the repo.

Good candidates:

- Public research PDFs.
- Official tool documentation.
- Official language/spec documentation.
- Benchmark papers and benchmark documentation.
- Small public HTML docs needed for reproducibility.

Be cautious with:

- Large binary files.
- Paywalled or redistribution-restricted material.
- Full website mirrors.
- Generated docs with unclear reuse terms.

Do not commit:

- Credentials, secrets, private material, or personal data.
- Cloned repositories under `repos/`.
- Material whose license or terms clearly prohibit redistribution.

When cloning:

- Use `research/<topic>/repos/`.
- Prefer partial or sparse clones for large repos when practical.
- Record exact commit hashes in `REPO-INDEX.md`.
- Record inspected source paths.
- Verify the clone path is ignored before committing:

```text
git check-ignore -v research/<topic>/repos/<repo-name>
```

## Subagents

Use many subagents for substantial research when current tool policy and the
user request allow parallel agent work. Parallel research is expected for large
topics, but split by angle rather than duplicating effort.

Useful subagent angles for polint research:

- Theory scout: papers, algorithms, complexity, precision limits.
- Production tool analyst: mature analyzers and implementation architecture.
- Language specialist: Go, TS/JS, Python, JVM/Java, Rust, or future adapters.
- Extension architect: how agent-authored Rust models safely alter analysis.
- Benchmark/evaluation analyst: external suites, metrics, ground truth, gates.
- Kernel/cache analyst: scheduling, provenance, cache keys, invalidation.
- Skeptical verifier: challenge claims, check references, find contradictions.
- Synthesizer: merge evidence into final reports and implementation path.

Subagents should return structured findings with source paths and confidence
labels. If the subagent cannot write files directly, the main agent should
preserve important findings in `SUBAGENT-FINDINGS.md` or `INSIGHTS_XX_*.md`.

## INSIGHTS File Format

For very large research tasks, preserve intermediate outputs as:

```text
INSIGHTS_XX_<angle>.md
```

Use this format:

```text
# INSIGHTS_XX - <angle>

Agent / angle:
Question answered:
Date:
Status: draft | reviewed | superseded

## Sources Reviewed

| Source | Type | Local path | URL | Access date | Notes |
| --- | --- | --- | --- | --- | --- |

## High-Confidence Findings

Facts strongly supported by sources.

## Medium-Confidence Findings

Likely conclusions that need more confirmation, context, or implementation
validation.

## Low-Confidence Leads

Potentially useful leads that should not be treated as facts yet.

## Contradictions Or Tensions

Conflicting sources, unclear terminology, circular dependencies, or questionable
assumptions.

## Implications For polint

What this means for engine design, SDK design, language adapters, extension
surface, evaluation, roadmap order, or implementation risk.

## Recommended Next Steps

Concrete follow-up tasks.
```

The main report must synthesize these files. Subagent insights are evidence
inputs, not final truth.

## Evidence And Confidence

Label claims by confidence when the strength matters:

- `High`: directly supported by primary sources or source-code inspection.
- `Medium`: supported by credible sources but needs more confirmation,
  implementation validation, or benchmark evidence.
- `Low`: plausible lead or inference; do not promote.
- `Unknown`: explicitly unresolved.

Distinguish:

- Fact: what a source says or what implementation code directly does.
- Interpretation: what we think it means for polint.
- Recommendation: what the research suggests polint should do.
- Decision: what the project has committed to do.
- Open question: what still needs validation.

Avoid unqualified comparative claims such as "best", "state of the art",
"proven", or "complete" unless the report explains the comparison set and
evidence. Prefer "strong reference", "recommended for polint", "mature", or
"inspected implementation shows".

## Algorithm And Implementation Analysis

For static-analysis research, every serious report should cover:

- algorithmic model;
- soundness or precision claims and their assumptions;
- time and memory complexity;
- scalability boundaries;
- failure modes and unsupported constructs;
- cache and invalidation implications;
- provenance and explainability implications;
- deterministic scheduling implications;
- public SDK implications;
- agent-extension implications;
- default mode versus agent-extended mode;
- benchmark and validation strategy.

When writing pseudo-code, keep it stripped down and Python-ish unless Rust types
are necessary to show ownership, trait, or API shape.

## Product Thesis

polint is not a sealed black-box analyzer that must infer every convention in
every codebase by itself.

polint should provide:

```text
sane native analysis defaults
  + typed facts
  + explicit uncertainty
  + repo-local rules
  + repo-local analysis models
  + agent-authored Rust extensions
  + validation and benchmark feedback
```

This changes the research lens:

1. What should the native engine infer by default across many repositories?
2. What can an AI agent accurately add after inspecting this specific
   repository?

Unknowns are not just failures. They are integration tasks. Research should
prefer architectures that make unknowns explicit and allow agents to add
validated, provenance-labeled models.

## Implementation Recommendations

Recommendations should be concrete enough to implement.

A good `RECOMMENDED_IMPLEMENTATION.md` includes:

- first vertical slice;
- internal module boundaries;
- public/private API boundary;
- data model and stable IDs;
- extension points and merge policies;
- cache keys and invalidation inputs;
- validation gates;
- benchmark gates;
- explicit non-goals;
- sequencing dependencies;
- risks that must be decided before coding.

Be especially careful about circular dependencies. When research tracks depend
on each other, split them into bootstrap tiers such as:

```text
direct/syntactic facts first
local analysis next
minimal summaries next
refined global analysis later
```

## Validation Pass

Before finalizing research:

- Check that all important claims have sources or confidence labels.
- Check `REPO-INDEX.md` commit hashes against local clones.
- Check that `repos/` is gitignored and not staged.
- Check that downloaded sources are indexed.
- Run a claim scan for overbroad language such as "best", "proven",
  "complete", "mandatory", and "state of the art".
- Verify that recommendations do not contradict earlier research or the roadmap.
- Have a skeptical verifier agent review the report when the user requested
  deep research and current tool policy allows subagents.
- Record corrections in `SUBAGENT-FINDINGS.md` or `VALIDATION.md`.

Useful local checks:

```text
git check-ignore -v research/<topic>/repos/<repo>
git diff --cached --check
rg -n "best|proven|complete|mandatory|state of the art" research/<topic> --glob '!papers/**'
```

## Promotion Rules

Do not promote raw research automatically.

Promote only:

- durable research-track status and sequencing to `research/ROADMAP.md`;
- accepted cross-topic research overview to `research/README.md`;
- product or architecture decisions to the appropriate decision log;
- public SDK or fact claims to docs only after validation.

Every promoted item must point back to the research topic and source material
that supports it.

## Final Deliverable Expectations

When finishing a research task:

- Make the folder navigable from `research/README.md`.
- Update `research/ROADMAP.md` with completed research and next steps.
- Commit the research docs and downloaded source material when appropriate.
- Do not commit cloned repositories.
- State what was validated and what was not.
- State the recommended next step plainly.

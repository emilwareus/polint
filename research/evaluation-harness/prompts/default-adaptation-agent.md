# Default Polint Benchmark Adaptation Agent Prompt

You are adapting polint to one benchmark target repository.

## Goal

Improve polint scanner accuracy on this target by writing repo-local polint rules,
model facts, or provider extensions that honestly describe the repository's
frameworks, lifecycle, APIs, sources, sinks, sanitizers, barriers, entrypoints,
summaries, dispatch behavior, and trust boundaries.

This is an adaptation benchmark. The output must show whether a code-aware agent
can improve polint by modeling the target repository, not whether it can learn an
answer key.

## Allowed Context

- Read the target repository source, build files, config files, tests, docs, and
  lockfiles.
- Use the polint skill at `.claude/skills/polint/SKILL.md`.
- Use polint rule-authoring, SDK, extension, model, and evaluation documentation
  available in this repository.
- Run the polint baseline and inspect diagnostics, unknowns, unresolved calls,
  missing entrypoints, setup gaps, false-positive examples surfaced by baseline
  output, and unsupported framework behavior.
- Run tests and polint eval iterations within the declared budget.
- Inspect benchmark harness manifests for suite metadata, selected tier, source
  commit, language support status, and run configuration.

## Forbidden Context And Behavior

- Do not read benchmark expected labels before writing the adaptation.
- Do not read answer keys, expected-results CSV data, suite-native truth files,
  or vulnerability labels before writing the adaptation.
- Do not hardcode benchmark case IDs.
- Do not hardcode benchmark case IDs, generated filenames, expected-result CSV
  data, suite names, source commit strings, or benchmark-specific path patterns
  as detection logic.
- Do not add broad facts or rules only to increase recall if they create
  unjustified false positives.
- Do not bypass extension validation, precision labels, provenance, cache inputs,
  capability checks, or public rule-authoring constraints.
- Do not edit the benchmark source repository or expected-label artifacts.

## Adaptation Process

1. Map the target language, package layout, framework/lifecycle entrypoints,
   request/job/CLI boundaries, generated dispatch, and test fixtures.
2. Identify where baseline polint is uncertain: unresolved calls, unknown flows,
   missing summaries, missing framework entrypoints, missing source/sink/barrier
   models, unsupported lifecycle, or noisy evidence.
3. Choose the narrowest repo-local mechanism:
   - normal `#[polint::rule]` diagnostics for repository policy checks;
   - model or provider extension facts for analysis semantics such as
     entrypoints, source/sink/sanitizer/barrier models, call edges, summaries, or
     trust boundaries.
4. Implement the rule or extension using public or explicitly approved polint
   authoring surfaces. Keep internals private.
5. Add small fixture tests or documented checks that prove the adaptation models
   real repository behavior rather than benchmark labels.
6. Run polint eval in baseline and adapted modes, inspect the delta, and iterate
   only to fix real modeling mistakes.
7. Produce an adaptation note listing explored files, assumptions, changed
   rules/extensions/models, validation results, unresolved limitations, and
   precision/runtime/cache tradeoffs.

## Deliverables

- Repo-local rule, model, config, or extension files changed.
- Adaptation note path.
- Commands run.
- Any fixture or smoke tests added.
- Final default-vs-adapted eval report path.
- Changed rule/extension file digests.
- Declared budget used:
  - `wall_time_minutes`: `<fill in>`
  - `max_iterations`: `<fill in>`

## Required Record Fields

- Prompt path: `research/evaluation-harness/prompts/default-adaptation-agent.md`
- Prompt hash.
- Allowed inputs actually used.
- Forbidden inputs declared as not used.
- Changed files or an explicit no-change reason.
- Rule and extension digests for changed artifacts.
- Adaptation notes and final adapted report path.

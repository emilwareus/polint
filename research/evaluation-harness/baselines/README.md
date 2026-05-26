# Evaluation Baseline Artifact Policy

Evaluation baselines support promotion gates and public benchmark comparison
tables. They must be small, sourceable, and reproducible.

## May Be Committed

- Small normalized baseline summaries generated from `EvaluationRun` JSON.
- Pinned suite manifest metadata, including suite id, source URL, source commit,
  language support, tier, selector, and deterministic seed.
- Markdown summaries that state product name, product version, command or source
  citation, retrieved date, metrics, and limitations.
- Adapter-only summaries for supported-suite dry runs, clearly labeled as not
  real polint scanner analysis.

## Do Not Commit

- Do not commit cloned benchmark repositories or large benchmark corpora.
- Do not commit raw third-party scanner outputs without license review.
- Do not commit machine-local paths, temporary directories, access tokens,
  absolute cache paths, or environment-specific package-manager state.
- Do not commit expected labels or answer keys into adaptation-agent context
  artifacts.

## Required Metadata

Every committed baseline summary must record:

- suite id and source commit or suite version;
- product name and product version;
- whether the result is imported from a published source, locally reproduced, a
  polint baseline run, a polint agent-adapted run, or adapter-only validation;
- command, config path, and artifact path for local reproductions;
- source name, source URL, and retrieved date for imported results;
- metric names and limitations.

Local generated reports belong under ignored output directories such as
`target/polint-eval/`. Commit only the normalized summary needed for regression
gates or benchmark tables.

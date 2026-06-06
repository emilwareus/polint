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

## Phase 54 Closeout Note

The final BENCH-01 audit is recorded at
`.planning/phases/54-benchmark-promotion-gate-extension/54-AUDIT.md`.

Local promotion verification passed for precision-floor enforcement, F0.5/F1
reporting, per-language deltas, false-positive trap flooding, the polyglot
canary, public-surface leak gate, determinism, clippy, rustfmt, and whitespace
checks.

External Go x/tools RTA and Jelly corpus final recall values are marked
limited/skipped in that audit because benchmark clones and generated full-corpus
outputs are not committed under this policy. Do not use the local Phase 54 audit
alone to claim a measured recall lift against those full external suites.

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

## Store-Disabled Performance Artifacts

`store-disabled-check.json` and `store-disabled-review.json` use the
`polint-store-disabled-baseline-1` schema. In addition to the measured metrics,
each artifact records the fixture version and digest, product version and source
revision, result kind, generation command, profile/features/lockfile settings,
target OS and architecture, Rust and Cargo identity, artifact path, metric
names, and portability limitations. Regeneration rejects tracked or untracked
workspace changes so the recorded clean-source claim is meaningful; ignored
build outputs remain ignored.

Regenerate both artifacts from a clean committed tree with a POSIX shell (or
Git Bash on Windows):

```console
CARGO_PROFILE_TEST_DEBUG=0 POLINT_WRITE_STORE_DISABLED_BASELINE=1 cargo test -p polint --lib --all-features --locked eval::baseline::tests::regenerate_committed_store_disabled_baselines -- --exact
```

The equivalent PowerShell invocation is:

```powershell
$env:CARGO_PROFILE_TEST_DEBUG = "0"
$env:POLINT_WRITE_STORE_DISABLED_BASELINE = "1"
cargo test -p polint --lib --all-features --locked eval::baseline::tests::regenerate_committed_store_disabled_baselines -- --exact
```

The committed numeric values are historical evidence and are informational
unless the complete recorded measurement context exactly matches the comparison
environment. Portable blocking performance checks use fresh store-disabled and
store-enabled controls on the same host. Normal tests still validate the
committed artifact schema and its fixture/version/digest contract.

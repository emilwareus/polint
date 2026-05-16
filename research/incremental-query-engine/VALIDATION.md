# Validation

## Source Collection

Downloaded papers are stored in:

```text
research/incremental-query-engine/papers/
```

Implementation repositories are cloned in:

```text
research/incremental-query-engine/repos/
```

The repository root `.gitignore` ignores this clone directory with:

```text
research/*/repos/
```

## Repository Sources Validated

The following codebases were inspected locally:

- Salsa
- rust-analyzer
- Go tools/gopls
- TypeScript compiler
- Pyright
- Pyrefly
- Pyre/Pysa
- Bazel
- Buck2
- Souffle
- Ruff/Ty

Key implementation files and lessons are indexed in [REPO-INDEX.md](REPO-INDEX.md).

## Paper Sources Validated

The following downloaded PDFs were present during validation:

- `adapton-demand-driven-incremental-computation.pdf`
- `demanded-abstract-interpretation-2021.pdf`
- `differential-dataflow-2013.pdf`
- `flowlog-vldb-2026.pdf`
- `incidfa-oopsla-2025.pdf`
- `incremental-codeql-fse-2023.pdf`
- `naiad-timely-dataflow.pdf`
- `using-standard-typing-algorithms-incrementally-2018.pdf`

Source URLs and lessons are indexed in [PAPER-INDEX.md](PAPER-INDEX.md).

## Accuracy Checks Performed

The recommendations were checked against these failure modes:

| Failure mode | Covered by |
|---|---|
| Stale reuse after source edit | Input snapshots, shape digests, dependency index, invalidation planner. |
| Stale reuse after extension edit | Extension code/model/validation digests and quarantine. |
| Rule option over-invalidation | Separate diagnostic keys from parser/layer keys. |
| Body edit invalidating whole repo | Text versus shape digests and equality pruning. |
| Recursive summary churn | SCC recompute with summary equality/backdating. |
| Missing dependency edges | Trace recorder and fail-closed policy for undeclared reads. |
| Provider/schema drift | Provider and schema versions in layer keys. |
| Official tool drift | Tool invocation key with version/config/environment digest. |
| Public API hardening too early | `pub(crate)` incremental module and typed SDK views only. |

## Final Validation Commands

Run before committing:

```sh
file research/incremental-query-engine/papers/*
git check-ignore -v research/incremental-query-engine/repos/salsa
git check-ignore -v research/incremental-query-engine/repos/rust-analyzer
git diff --check -- research/README.md research/ROADMAP.md research/incremental-query-engine
LC_ALL=C rg -n "[^\\x00-\\x7F]" research/incremental-query-engine research/README.md research/ROADMAP.md
git status --short --untracked-files=all
```

## Validation Result

Validation performed on 2026-05-16:

- `file research/incremental-query-engine/papers/*` confirmed every downloaded
  paper is a PDF.
- `git check-ignore -v research/incremental-query-engine/repos/...` confirmed
  cloned implementation repositories are ignored by `.gitignore`.
- `git diff --check -- research/README.md research/ROADMAP.md research/incremental-query-engine`
  completed with no whitespace errors.
- ASCII scan of the new research folder returned no non-ASCII text. The only
  non-ASCII match in the wider checked range was pre-existing roadmap text
  mentioning the contextual Astree analyzer.
- `git status --short --untracked-files=all` listed only research docs and
  downloaded PDFs, not ignored cloned repositories.

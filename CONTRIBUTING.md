# Contributing to polint

Thank you for helping to improve polint. Bug reports, documentation fixes, tests, and
focused features are welcome.

## Before you start

- Search the open issues before you create a new issue.
- For a large change, open an issue first. Explain the problem and the proposed scope.
- Read [AGENTS.md](AGENTS.md) and [ARCHITECTURE.md](ARCHITECTURE.md). They define the
  engineering invariants.
- polint ships no built-in policy rules. Do not add one. A rule that is useful to
  everyone belongs in the templates behind `polint new-rule --template`, as a scaffold
  the user edits, not as a rule polint enables.
- Keep the public surface to `polint::sdk::prelude` and `polint::runner`. Rule packs must
  not need anything else.
- Diagnostics must carry evidence. A rule that reports a location without saying what it
  found and how to repair it is not finished.

## Local development

You need Rust 1.95 or newer with `rustfmt` and Clippy, plus Go and Node for the language
frontends the tests exercise. `cargo deny` is needed for the supply-chain job.

```sh
cargo build --workspace
make check
```

`make check` is the full local gate and mirrors `.github/workflows/ci.yml`.

Useful commands:

```sh
make lint            # cargo fmt --check, then clippy -D warnings
make test            # cargo test --workspace --all-features
make doc             # rustdoc with -D warnings
make deny            # advisories, licenses, bans
```

The CLI and library are in `crates/polint`. The published crate is `polint`.

Golden outputs live in `tests/golden/`. Regenerate them with
`POLINT_UPDATE_GOLDENS=1 cargo test` and read the diff before committing it. The golden
suite also enforces a wall-clock and memory budget, so it can fail on a loaded machine.
Re-run it alone before assuming a regression.

The external graph-accuracy gate is not part of `make check`. It scores polint's call
graph against the Jelly JS/TS and Go x/tools RTA oracles and fails on an F1 regression
against `research/evaluation-harness/baselines/persisted-graph-accuracy.json`. Run it on
demand with `make eval-gate`, or on GitHub with `gh workflow run eval-gate.yml`. It clones
the pinned benchmark repositories and writes reports to `.context/graph-benchmarks/`.

## Benchmarks

`make bench-run` is the benchmark runner. It is the same script
`.github/workflows/bench-run.yml` executes, so a laptop number and a CI number come from
one code path. It logs the machine, fetches the pinned public corpora, measures wall
clock, peak RSS, on-disk cache size and output md5 across warm, cold and no-cache tiers,
scores the two call-graph oracles, and writes a readable `summary.md` plus every raw
sample into `.context/bench-run/`.

```sh
make bench-run                    # the CI default selection
make bench-run SCALE=1            # adds the excalidraw and hugo scale corpora
make bench-run SCALE=1 GRAFANA=1 DEEP_TARGETS=all   # everything; local only, takes hours
make bench-run ACCURACY=0         # speed only, no oracle scoring
make bench-run BUILD_COST=1       # also measures the repo-local rule-host build cost
```

It needs Go and Node in addition to the Rust toolchain. On GitHub, run it with
`gh workflow run bench-run.yml --ref <branch>`; the same report lands in the job summary
and in the `bench-run-reports` artifact.

Two rules the runner exists to keep:

- **Public corpora only.** Every measured repository is public open source at the commit
  SHA its suite manifest pins, and the target list in
  `scripts/bench-runner/bench_matrix.py` is an explicit allowlist. A manifest whose
  license is `proprietary` or still `license-review-needed` is rejected rather than
  measured.
- **Never blend measurements.** Accuracy and performance are separate sections, the two
  performance workloads are separate tables, and every partial oracle carries the
  sentence that says so. `scripts/bench-runner/how-to-interpret.md` ships in the artifact
  and explains what each benchmark can and cannot establish; keep it current when you add
  a benchmark.

`bench-run.yml` never fails a job on a benchmark number. `eval-gate.yml` is the workflow
that gates; `bench-run.yml` is the one that measures.

## Pull requests

1. Make one focused change.
2. Add or update tests for behavior changes.
3. Update public documentation when users must change how they use polint.
4. Run `make check`.
5. Explain the reason for the change and how you tested it.

Do not hand-edit generated output. Regenerate it.

## Documentation style

Short sentences and direct language. Verify every command, file path, and public symbol
against the current code before you commit it.

Keep the README on purpose, install, and the first successful run. Detailed reference
material goes in `docs/`. Do not use em-dashes.

## Reporting a vulnerability

Do not open a public issue. See [SECURITY.md](SECURITY.md).

## Conduct

Be respectful and constructive. Focus reviews on the work, give clear reasons for
requested changes, and assume good intent. Harassment and discriminatory behavior are
not accepted.

## License

By submitting a contribution, you agree that it is licensed under the repository's
[MIT License](LICENSE).

# Code-Preserving Rule Build

## Question

Is there a build, distribution, and execution architecture that removes almost
all of the compilation a customer pays for on a `polint check`, **without
changing what a polint rule is**?

## Why it matters

A repository with `.polint/rules` does not run its scan in the prebuilt `polint`
binary. `polint check` shells out to `cargo run` against the repo's rule pack
(`crates/polint/src/cli/mod.rs:4252-4343`), and that pack depends on the whole
`polint` library — parsers, kernel, solvers, and a bundled SQLite. So the first
thing a user waits for is a compile of the analysis engine, once per repository,
and Cargo is started again on every later scan even when nothing changed.

The obvious way to make that cheap is to stop shipping rules as compiled code.
That is the one thing this research may not do. polint's product value is that a
policy is real, expressive, typed Rust: `#[polint::rule]`, typed fact views,
`RuleCtx`, `RuleResult`, ordinary control flow, ordinary review, ordinary
`cargo`-shaped tests. A declarative policy language was considered previously and
rejected. This topic exists to find the build architecture that keeps that model
intact.

## Status

Research complete; implementation not started beyond the measurement slice.

**Product decision:** the architecture is proposed for a direct, intentionally
breaking **0.3.0** migration (not yet released). 0.3.0 replaces the 0.2
rule-pack manifest/build/execution contract with the thin-SDK/protocol path.
There is no legacy backend, automatic legacy fallback, multi-minor rollout, or
deprecation window. Users upgrade polint and migrate their pack manifest; rule
`.rs` source and the typed Rust authoring API remain stable.

| Deliverable | State |
| --- | --- |
| [`FINAL-REPORT.md`](FINAL-REPORT.md) | Complete. Current-state evidence, seven code-preserving alternatives with explicit rejection reasons, decision matrix, recommended architecture, security and trust boundaries, experiment plan with budgets and kill criteria. |
| [`IMPLEMENTATION-PLAN.md`](IMPLEMENTATION-PLAN.md) | Complete. Package graph, boundary design, snapshot format, host/rule protocol, build and artifact lifecycle, threat model, eleven phases with per-task files and acceptance criteria, testing matrix, budgets, direct breaking 0.3.0 migration and release plan, documentation plan, risks, and a requirement→phase traceability table. |
| [`RESEARCH-BRIEF.md`](RESEARCH-BRIEF.md) | The original brief the report answers, retained so the report's scope is auditable. |
| [`IMPLEMENTATION-PLAN-BRIEF.md`](IMPLEMENTATION-PLAN-BRIEF.md) | The original plan brief with its compatibility requirement explicitly superseded by Emil's later breaking-migration decision. |
| Phase A measurement harness | Tasks A1, A3, A4 landed: `polint-bench build-cost`, `make build-cost` / `make build-cost-baseline`, and a measured baseline at [`../evaluation-harness/baselines/build-cost.json`](../evaluation-harness/baselines/build-cost.json). Metric definitions and limits are in [`../evaluation-harness/README.md`](../evaluation-harness/README.md). A2 (more repositories), A5 (extend the per-check cost record), and A6 (a CI job) are outstanding. |
| Phases B–K | Not started. `IMPLEMENTATION-PLAN.md` §14.1 has the order; B does not depend on A2. |

## Summary of the conclusion

**Prebuilt engine host + thin-SDK rule binary over a fact-snapshot protocol.**

Today the customer compiles the engine and the engine does the analysis; the
prebuilt CLI is only an orchestrator. Invert it. Let the prebuilt binary produce
facts, write them as an owned snapshot, and let the repo-local rule pack compile
against a thin SDK that contains no parsers, no solvers, no SQLite, and no
kernel. The rule process deserializes the snapshot once and the existing typed
views borrow from it, so `SourceFiles::all(self) -> &'a [SourceFile]` and every
other accessor keeps its exact signature and **rule sources do not change by a
byte**.

The package cutover is nevertheless intentional and breaking: after 0.3.0 is
released, a 0.2 pack updates its dependency to `polint-sdk` renamed as `polint`,
rebuilds/regenerates its host artifacts as needed, runs `polint test` and
`polint check`, and upgrades. A planned `polint rules migrate` command is only a
one-shot rewrite aid; it is not a compatibility layer.

Rejected, each with its reason recorded: a DSL or declarative policy format
(violates the product invariant), native `cdylib` plugins (no stable Rust ABI, so
facts would have to become `repr(C)`), WASM as the *primary* backend (does not
remove the author's compile, and needs the same SDK split anyway — it is kept as
a later optional distribution and sandbox backend), RPC fact views (breaks the
borrowed-slice API), shipping a populated target directory, and compiler-flag
tuning alone.

## Measured, versus proposed

The report's §3.3.2 and the committed `build-cost` baseline are measurements. The
budgets in `FINAL-REPORT.md` §9 and `IMPLEMENTATION-PLAN.md` §10 are targets and
kill criteria, not results, and are labelled as such. No benchmark result in
either document is invented.

## Deviations from the standard topic layout

`research/AGENTS.md` describes a fuller topic layout. This topic is a
build-architecture investigation of one codebase rather than an algorithms
study, so several of those files would have been empty headings:

| Standard file | Here |
| --- | --- |
| `RECOMMENDED_IMPLEMENTATION.md` | `IMPLEMENTATION-PLAN.md` is it, under the name the brief asked for. |
| `RESEARCH-ANALYSIS.md` | Folded into `FINAL-REPORT.md` §6 (alternatives, rejections) and §11 (risks, unresolved decisions). |
| `REPO-INDEX.md` | No external repository was cloned. The implementations discussed as ecosystem precedent (Dylint, golangci-lint module plugins, CodeQL packs, Wasmtime) were **not** inspected in the research environment and are listed as unverified in `FINAL-REPORT.md` §13.3 with the URL to check. |
| `PAPER-INDEX.md` | No papers were downloaded. External sources are official Rust/Cargo documentation, indexed in `FINAL-REPORT.md` §13.2 (fetched and quoted) and §13.3 (not reachable, flagged at every use site). |
| `VALIDATION.md` | The validation plan is `FINAL-REPORT.md` §9 (experiments E1–E10, budgets, kill criteria) and `IMPLEMENTATION-PLAN.md` §9–§10 (test matrix, budgets, KC-1..KC-7). The first of those experiments is now executable: `make build-cost`. |
| `SUBAGENT-FINDINGS.md` | No subagents were used. |
| `algorithms/`, `papers/`, `repos/`, `domains/`, `languages/` | Not applicable to this topic. |

## PR size

The pull request carrying this topic is over the ≤ 1,500-changed-line budget
recorded in `docs/architecture-review/PLAN.md:79` and
`docs/architecture-review/HANDOFF.md:53`. That budget is a stop condition for the
`static-analysis-architecture-review` track, whose first working rule scopes the
whole set to work landing on that branch
(`docs/architecture-review/PLAN.md:76-78`); this topic is research plus one
unpublished measurement crate, on its own branch, targeting `main`. It is under
the companion ≤ 25-file limit, and the Rust it adds is confined to
`crates/polint-bench`, changes no product behaviour, and is revertible on its
own. Splitting the research prose from the harness would
separate the claims table in `IMPLEMENTATION-PLAN.md` §0 from the measurement
that turned three of its rows from reported to `[measured]`, which is the
opposite of the point.

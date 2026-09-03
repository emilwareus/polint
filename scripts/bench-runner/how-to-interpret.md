# How to interpret a polint benchmark run

This file ships inside every `bench-run-reports` artifact. It describes what each
benchmark measures, what its oracle can establish, and where it is weak. Read it
before quoting a number from `summary.md`.

<!-- summary-bullets -->
- **Accuracy and performance are different measurements and are never combined.**
  There is no single "polint score" in this report and there should not be one.
- **Both accuracy oracles are partial.** Jelly's oracle is dynamic (it contains
  only edges some execution actually took). The Go x/tools RTA oracle enumerates
  a few dozen edges against thousands polint reports over the same code. Read
  precision and recall separately; neither precision figure is a false-positive
  rate over real code.
- **The performance workloads are capability-gated, not `polint check`.** They
  exercise the analysis kernel over a whole repository without a repo-local rule
  pack, so they exclude rule-host compilation and rule execution. The rule-host
  build cost is reported separately when it is measured.
- **The two performance workloads differ by two orders of magnitude** and must
  not be compared with each other. `syntactic` is the floor; `deep` plans every
  public capability including dataflow.
- **Numbers only compare against numbers from a comparable machine.** The
  machine is recorded in `environment.json`; a hosted GitHub runner is roughly a
  quarter of a developer workstation and its wall clock reflects that.
<!-- /summary-bullets -->

## What is in the artifact

| file | contents |
| --- | --- |
| `summary.md` | the rendered report, identical to the job summary |
| `environment.json` / `.txt` / `.md` | the machine the run happened on |
| `run-context.json` | the exact command, options, and start/finish timestamps |
| `corpus-pins.tsv` | id, commit, url, checkout path, digest for every pinned clone |
| `perf.json` | every performance sample, not just the medians |
| `accuracy/` | per-suite evaluation JSON and markdown, plus the suite's own `summary.md` |
| `accuracy-status.json` | whether the accuracy step ran, its exit code, and the npm-tree state |
| `build-cost.json` | rule-host build-cost report, when that step ran |
| `how-to-interpret.md` | this file |

## The corpora

Every corpus is public open source, cloned at the exact commit SHA its suite
manifest under `research/evaluation-harness/suites/` declares. No private
repository is fetched, measured, or reported by this runner, and the target list
in `scripts/bench-runner/bench_matrix.py` is an explicit allowlist rather than a
glob over the suites directory.

| corpus | upstream | license | role |
| --- | --- | --- | --- |
| `jelly` | `cs-au-dk/jelly` | BSD-3-Clause | JS/TS call-graph oracle, and a real TS codebase to time |
| `golang-tools` | `golang/tools` | BSD-3-Clause | Go RTA call-graph oracle, and a real Go codebase to time |
| `excalidraw` | `excalidraw/excalidraw` | MIT | TypeScript scale target |
| `hugo` | `gohugoio/hugo` | Apache-2.0 | Go scale target |
| `grafana` | `grafana/grafana` | AGPL-3.0 | polyglot scale target, local only |

### Corpora deliberately excluded

- **`secbench-js-smoke`** (`SecBench/SecBench.js`) and **`gosec-samples`**
  (`securego/gosec`) both carry `license = "license-review-needed"` in their suite
  manifests. Until that review concludes, this runner does not clone, measure, or
  report them. An unreviewed license is not a benchmark result.
- **`grafana`** is 1.5M LOC. It is available locally (`make bench-run GRAFANA=1`)
  and is never selected on a hosted runner, whose disk and memory it does not fit.

## Accuracy: what the oracles establish

### `jelly-callgraph-micro`

The oracle is the set of call-graph edges Jelly recorded by *running* its own
test cases (`tests/*/…callgraph.json`). polint's call graph is scored edge by
edge against that set.

What this can establish: that polint finds edges a real execution took, and that
it does not lose them between releases.

What it cannot establish:

- **The oracle is dynamic, so it is incomplete by construction.** An edge polint
  reports that the oracle does not contain may be a genuine edge that no test run
  exercised. Precision against a dynamic oracle is a lower bound on precision.
- **The corpus is micro-benchmarks.** Most cases are hand-written and a few dozen
  lines long.
- **One case dominates and needs an npm tree.** `tests/helloworld` is an Express
  application and accounts for 342 of the 1,479 expected edges (23%). Its imports
  only resolve when `node_modules` is installed in that directory. The
  `accuracy-status.json` row `npm_tree` records what this run did:
  - `installed` - `npm ci` ran against the upstream `tests/helloworld/package-lock.json`
    (which is committed in `cs-au-dk/jelly`, so the tree is pinned and
    reproducible), and the numbers include the case.
  - `not-installed`, `absent` or `failed` - the case's edges cannot resolve, so
    **recall is a lower bound** and must be labelled as such wherever it is quoted.

  Installing it is off by default because it is expensive and because it makes the
  oracle's partiality dominate the score. Both arms, measured on 2026-09-03 on an
  8-core AMD EPYC-Rome container (release tier, all 76 cases):

  | npm tree | TP | FP | FN | precision | recall | F1 | `helloworld` edges observed | suite runtime |
  | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
  | not installed | 249 | 8 | 1230 | 96.89% | 16.84% | 28.69% | 10 (of 342 expected) | 131 s |
  | installed | 376 | 449 | 1103 | 45.58% | 25.42% | 32.64% | 2,748 (of 342 expected) | 2,624 s |

  Read that table carefully, because the naive reading is wrong. Installing the
  tree did not make polint less precise: it made polint able to see Express, so it
  reports 2,748 edges in a case whose *dynamic* oracle recorded 342. Precision
  falls from 97% to 46% because the oracle does not enumerate what a real
  execution did not reach, not because 449 new findings are wrong. Recall rises
  because 127 genuinely expected edges became resolvable. Quote whichever arm you
  ran, name it, and never mix rows from the two.

### `go-x-tools-rta-callgraph`

The oracle is rapid type analysis call-graph edges over `golang.org/x/tools`.

What this can establish: recall against a sound-by-construction static oracle.

What it cannot establish:

- **The expected-edge set is tiny relative to what polint reports.** The oracle
  enumerates tens of edges; polint reports thousands over the same code. The
  resulting precision figure is therefore dominated by edges the oracle does not
  enumerate at all, and it is **not** a false-positive rate. Quote recall from
  this suite; quote precision only with this sentence attached.

### The regression gate

`eval-gate.yml` (and `make eval-gate`) fails when F1 drops more than 0.005 below
the committed `research/evaluation-harness/baselines/persisted-graph-accuracy.json`.
The benchmark runner reports the same numbers but never fails the job on them:
one workflow gates, the other measures.

**A baseline is only a comparison when the same scoring code produced it.** The
committed baseline was last written on 2026-08-07 (`c3f6a040`). Call-graph scoring
changed the following day in `477ac54a` ("retire reachability precision theater"),
and the baseline was not regenerated, so today's measurement is scored differently
from the numbers it is compared against. The clearest symptom is the Go suite:
the baseline records 10,233 observed edges and 100% recall against 37 expected
edges, which is what "observe everything" produces, while the current scoring
reports tens of edges per case. Check `git log` on the baseline file against the
harness before reading a gap in the summary's comparison table as an engine
regression.

## Performance: what the workloads measure

Both workloads run the shipped `polint` binary built from the commit under test,
from inside the corpus checkout, and neither needs a repo-local rule pack.

### `syntactic` - `polint facts sample --cap file_metrics --limit 1 --format json`

Walks and parses every source file in the repository and computes per-file
metrics. `--limit 1` bounds only how many rows are printed. This is the floor
cost of pointing polint at a repository.

### `deep` - `polint inspect unknowns --format json`

Plans every public unknown capability - `resolved_imports`, `symbols`,
`references`, `events`, `calls`, `control_flow`, `dataflow` - over the whole
repository, and reports what did not resolve. This is the deepest analysis
reachable from the shipped CLI without a rule pack.

**This is a capability-gated run, not a `polint check` run.** It excludes:

- compiling the repo-local rule host (measured separately by `build-cost`);
- running rules and rendering diagnostics;
- the capabilities a specific rule pack would request beyond these seven.

Because `deep` costs minutes per run on a large repository, it is opt-in per
target (`--deep-targets`, `make bench-run DEEP_TARGETS=all`). Targets that did not
run it say so in `summary.md` rather than silently reporting only the cheap
workload.

Measured reasons for the current default (`--deep-targets jelly`), taken on
2026-09-03 on an 8-core AMD EPYC-Rome container with 15 GiB RAM:

| target | `deep` outcome | consequence |
| --- | --- | --- |
| `jelly` (29,787 LOC) | completes; median 115 s warm / 89 s cold / 78 s no-cache, **5.4 GiB peak RSS** | in the default set |
| `golang-tools` (398,228 LOC) | **did not finish in 25 minutes**, 3.1 GiB peak RSS and still climbing | opt-in only; a hosted runner cannot afford it |

**Memory headroom warning.** 5.4 GiB peak RSS on the smallest corpus is close to
a standard hosted runner's total memory (about 7 GiB / 16 GiB depending on the
tier). A `deep` cell that gets OOM-killed on CI is reported as `killed` with
`signal: SIGKILL` and the job still finishes green: the runner records what
happened rather than hiding it or turning the workflow red. Read the runner's
`mem_total_gib` in `environment.json` before treating a killed cell as a
regression.

Re-measure before changing the default, and update this table with what you saw.

### Cache tiers

| tier | what it does | what it tells you |
| --- | --- | --- |
| `warm` | an unmeasured priming run first, then measure with the cache present | the cost a developer pays on a second run |
| `cold` | `<repo>/.polint/cache` deleted before every measured run | the cost of a first run, or of a CI job with no cache restore |
| `no-cache` | cache deleted and `--no-cache` passed, so nothing is read or written | the cost with the cache subsystem out of the picture |

`no-cache` is not the same as `cold`: `cold` still writes a cache, and the write
is part of its cost.

### How the numbers are taken

- Wall clock is `time.monotonic_ns` around the child process.
- Peak RSS is the child's own `rusage.ru_maxrss` from `os.wait4` - the same field
  `/usr/bin/time -v` prints as "Maximum resident set size". Measuring it
  in-process keeps CI and a laptop on one code path instead of depending on GNU
  `time` being installed.
- Runs are strictly sequential; nothing else is measured in parallel.
- The reported figure is the **median** of the successful runs in the cell, and
  every individual sample is printed beside it. A median over three samples on a
  shared CI runner is not a tight measurement; treat a difference under roughly
  20% as noise unless the samples are tight.
- A cell stops at its first failing run. A crash is reported as a crash; it is
  never averaged into a median.

### Output identity

Each run's stdout is md5-hashed. For one target and workload the digest must be
identical across the warm, cold, and no-cache tiers, and identical across runs of
the same commit. A digest that moves while the commit does not is a caching or
nondeterminism defect, and `summary.md` flags it. This is the check that makes
the cache tiers safe to compare: if the cache changed the answer, the timings
would be measuring two different computations.

## Rule-host build cost

`polint-bench build-cost` measures what a repo-local rule pack costs to compile:
Cargo invocations, compiled units, wall clock, and bytes written. It is the cost
`polint check` pays that neither performance workload above includes. It is
opt-in because a cold cell compiles the full dependency closure and writes
hundreds of megabytes.

## Comparing runs over time

`perf.json` and `environment.json` are designed to be diffed across runs. When
comparing:

1. Compare `environment.json` first. A different CPU model, core count, or
   container status makes the wall clock incomparable. Peak RSS is more portable
   than wall clock but still moves with allocator and kernel differences.
2. Compare medians, then look at the samples. Three samples on a hosted runner
   routinely spread 20%.
3. Compare `stdout_md5` at the same polint commit. A change there is a
   correctness signal and outranks any timing change.
4. Compare accuracy only against a run with the same `npm_tree` status and the
   same suite tier.

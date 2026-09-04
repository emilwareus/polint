#!/usr/bin/env bash
# One benchmark run: log the machine, fetch the pinned public corpora, measure,
# and render a readable report.
#
# This is the single code path. `.github/workflows/bench-run.yml` and
# `make bench-run` both call this script with the same options, so a CI number
# and a laptop number come from the same instructions.
#
#   scripts/bench-runner/bench-run.sh
#
# Options, as environment variables (the Makefile target sets them from its own
# flags, and the workflow sets them from its dispatch inputs):
#
#   BENCH_OUT=.context/bench-run   artifact directory to write
#   BENCH_RUNS=3                   measured runs per cell
#   BENCH_TIMEOUT_SECONDS=1200     per-run wall-clock budget
#   BENCH_SCALE=0                  1 adds the excalidraw and hugo scale corpora
#   BENCH_GRAFANA=0                1 adds grafana as well (local only; 1.5M LOC)
#   BENCH_DEEP_TARGETS=jelly       targets that run the deep workload, or `all`
#   BENCH_ONLY=                    restrict the run to these target ids (comma
#                                  separated); empty means every selected target
#   BENCH_ACCURACY=1               1 also scores the two call-graph oracles
#   BENCH_NPM_JELLY=0              1 installs the Jelly tests/helloworld npm tree,
#                                  which resolves 342 of the 1,479 expected edges
#                                  but costs about 42 minutes for that one case;
#                                  0 leaves it unresolved and the report labels
#                                  recall as a lower bound
#   BENCH_BUILD_COST=0             1 also measures the rule-host build cost
#   BENCH_BUILD=1                  1 builds the release binaries first
#   BENCH_ACCURACY_TIER=release    suite tier for the accuracy run
#   POLINT_BIN=target/release/polint  binary under test
#
# No private repository is fetched, measured, or reported. Every corpus is
# public open source at the commit SHA its suite manifest pins.
set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "$repo_root"

out_dir=${BENCH_OUT:-$repo_root/.context/bench-run}
case $out_dir in
  /*) ;;
  *) out_dir=$repo_root/$out_dir ;;
esac
runs=${BENCH_RUNS:-3}
timeout_seconds=${BENCH_TIMEOUT_SECONDS:-1200}
scale=${BENCH_SCALE:-0}
grafana=${BENCH_GRAFANA:-0}
deep_targets=${BENCH_DEEP_TARGETS:-jelly}
only=${BENCH_ONLY:-}
accuracy=${BENCH_ACCURACY:-1}
npm_jelly=${BENCH_NPM_JELLY:-0}
build_cost=${BENCH_BUILD_COST:-0}
do_build=${BENCH_BUILD:-1}
accuracy_tier=${BENCH_ACCURACY_TIER:-release}
polint_bin=${POLINT_BIN:-$repo_root/target/release/polint}

started_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
rm -rf "$out_dir"
mkdir -p "$out_dir"

command_line="BENCH_RUNS=$runs BENCH_SCALE=$scale BENCH_GRAFANA=$grafana"
command_line="$command_line BENCH_DEEP_TARGETS=$deep_targets BENCH_ACCURACY=$accuracy"
command_line="$command_line BENCH_NPM_JELLY=$npm_jelly"
command_line="$command_line BENCH_BUILD_COST=$build_cost scripts/bench-runner/bench-run.sh"

step() { printf '\n=== %s ===\n' "$1" >&2; }

# ---- 1. build ------------------------------------------------------------
if [ "$do_build" = 1 ]; then
  step "build the binaries under test"
  cargo build --release --locked -p polint -p polint-bench
fi
if [ ! -x "$polint_bin" ]; then
  echo "::error::polint binary not found at $polint_bin (set BENCH_BUILD=1 or POLINT_BIN)" >&2
  exit 1
fi
"$polint_bin" --version

# ---- 2. machine ----------------------------------------------------------
step "log the machine"
POLINT_BIN="$polint_bin" "$repo_root/scripts/bench-runner/log-environment.sh" "$out_dir" "$repo_root"

# ---- 3. corpora ----------------------------------------------------------
step "resolve and fetch the pinned public corpora"
"$repo_root/scripts/bench-runner/fetch-corpus.sh" pins \
  --suites callgraph --out "$out_dir/corpus-pins.tsv"
npm_args=()
if [ "$npm_jelly" = 1 ]; then
  npm_args+=(--npm-jelly)
fi
"$repo_root/scripts/bench-runner/fetch-corpus.sh" fetch \
  --suites callgraph "${npm_args[@]}" --out "$out_dir/npm-tree.json"

scale_args=()
if [ "$scale" = 1 ] || [ "$grafana" = 1 ]; then
  scale_only="excalidraw-excalidraw-scale,gohugoio-hugo-scale"
  if [ "$grafana" = 1 ]; then
    scale_only="$scale_only,grafana-grafana-scale"
  fi
  "$repo_root/scripts/bench-runner/fetch-corpus.sh" pins \
    --suites scale --only "$scale_only" --out "$out_dir/corpus-pins-scale.tsv"
  cat "$out_dir/corpus-pins-scale.tsv" >> "$out_dir/corpus-pins.tsv"
  "$repo_root/scripts/bench-runner/fetch-corpus.sh" fetch \
    --suites scale --only "$scale_only"
  scale_args+=(--scale)
  if [ "$grafana" = 1 ]; then
    scale_args+=(--scale-heavy)
  fi
fi

# ---- 4. accuracy ---------------------------------------------------------
accuracy_rc=""
if [ "$accuracy" = 1 ]; then
  step "score the call-graph oracles (Jelly, Go x/tools)"
  baseline_rel=research/evaluation-harness/baselines/persisted-graph-accuracy.json
  # The measurement rewrites the committed baseline. Restore it afterwards unless
  # the caller already had it modified, and keep the delta in the artifact: a
  # benchmark run should report a moved baseline, not silently commit one.
  baseline_was_clean=1
  if [ -n "$(git status --porcelain -- "$baseline_rel" 2>/dev/null)" ]; then
    baseline_was_clean=0
  fi
  rm -rf "$repo_root/.context/graph-benchmarks"
  mkdir -p "$out_dir/accuracy"
  # Keep the committed baseline beside the measurement so the report can show the
  # delta even when the gate fails before it rewrites the file.
  cp "$repo_root/$baseline_rel" "$out_dir/accuracy/committed-baseline.json"
  set +e
  POLINT_REQUIRE_BENCH_CORPUS=1 \
  POLINT_WRITE_GRAPH_BENCH=1 \
  POLINT_GRAPH_BENCH_TIER="$accuracy_tier" \
    cargo test -p polint --lib --all-features --locked \
    eval::external::tests::external_graph_baseline_reports_can_be_generated \
    -- --nocapture
  accuracy_rc=$?
  set -e
  if [ -d "$repo_root/.context/graph-benchmarks" ]; then
    cp -R "$repo_root/.context/graph-benchmarks/." "$out_dir/accuracy/"
  fi
  git diff -- "$baseline_rel" > "$out_dir/accuracy/baseline-diff.patch" || true
  if [ "$baseline_was_clean" = 1 ]; then
    git checkout -- "$baseline_rel" 2>/dev/null || true
  fi
  echo "accuracy step exited $accuracy_rc" >&2
fi
python3 - "$out_dir" "$accuracy" "$accuracy_rc" "$accuracy_tier" <<'PY'
import json
import sys
from pathlib import Path

out_dir, ran, rc, tier = Path(sys.argv[1]), sys.argv[2] == "1", sys.argv[3], sys.argv[4]
npm_path = out_dir / "npm-tree.json"
npm = json.loads(npm_path.read_text()) if npm_path.is_file() else {"status": "unknown", "detail": ""}
(out_dir / "accuracy-status.json").write_text(
    json.dumps(
        {
            "ran": ran,
            "exit_code": int(rc) if rc else None,
            "tier": tier if ran else None,
            "npm_tree": npm,
        },
        indent=2,
    )
    + "\n"
)
PY

# ---- 5. performance matrix ----------------------------------------------
step "measure the performance matrix"
python3 "$repo_root/scripts/bench-runner/bench_matrix.py" \
  --repo-root "$repo_root" \
  --polint-bin "$polint_bin" \
  --out "$out_dir/perf.json" \
  --runs "$runs" \
  --timeout-seconds "$timeout_seconds" \
  --deep-targets "$deep_targets" \
  --only "$only" \
  "${scale_args[@]}"

# ---- 6. rule-host build cost --------------------------------------------
if [ "$build_cost" = 1 ]; then
  step "measure the rule-host build cost"
  label=local
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    label="github-${RUNNER_OS:-unknown}-${RUNNER_ARCH:-unknown}"
  fi
  set +e
  cargo run --release --locked -p polint-bench -- build-cost \
    --label "$label" \
    --runs 1 \
    --repo examples/basic \
    --scenario cold \
    --scenario warm-noop \
    --out "$out_dir/build-cost.json"
  build_cost_rc=$?
  set -e
  printf '{"ran": true, "exit_code": %s}\n' "$build_cost_rc" > "$out_dir/build-cost-status.json"
else
  printf '{"ran": false, "reason": "not selected (BENCH_BUILD_COST=1 measures it)"}\n' \
    > "$out_dir/build-cost-status.json"
fi

# ---- 7. report -----------------------------------------------------------
step "render the report"
cp "$repo_root/scripts/bench-runner/how-to-interpret.md" "$out_dir/how-to-interpret.md"
finished_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
COMMAND="$command_line" STARTED="$started_at" FINISHED="$finished_at" \
  python3 -c '
import json, os, sys
json.dump(
    {
        "command": os.environ["COMMAND"],
        "started_at_utc": os.environ["STARTED"],
        "finished_at_utc": os.environ["FINISHED"],
    },
    open(sys.argv[1], "w"),
    indent=2,
)
' "$out_dir/run-context.json"

python3 "$repo_root/scripts/bench-runner/render_summary.py" --artifact-dir "$out_dir"
echo "artifact directory: $out_dir" >&2

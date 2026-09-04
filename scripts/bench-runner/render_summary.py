#!/usr/bin/env python3
"""Render one readable markdown report from a benchmark run's artifact directory.

Reads whatever the run produced (`environment.json`, `perf.json`, the accuracy
reports, `build-cost.json`) and writes `summary.md`. The same file is printed to
the job summary on GitHub Actions and to stdout locally, so a run is read the
same way in both places.

Two rules this renderer enforces so the output stays honest:

  * accuracy numbers and performance numbers never share a table, and the two
    performance workloads never share a table either;
  * anything that was excluded, skipped, timed out, or scored against a partial
    oracle is printed, not dropped.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

# Oracle honesty notes, keyed by suite id. These describe what the oracle can and
# cannot establish and are printed beside every accuracy number.
ORACLE_NOTES: dict[str, list[str]] = {
    "jelly-callgraph-micro": [
        "Oracle: the dynamic call graphs Jelly records for its own test cases "
        "(`tests/*/…callgraph.json`), scored edge by edge.",
        "PARTIAL ORACLE. A dynamic oracle only contains edges some execution actually "
        "took, so an edge polint reports that the oracle lacks is not necessarily a "
        "false positive, and recall is measured against observed executions rather "
        "than against all reachable behaviour.",
        "The corpus is micro-benchmarks. The largest case, `tests/helloworld`, is an "
        "Express app worth 342 of the 1,479 expected edges, and it only resolves once "
        "its npm tree is installed. See the npm-tree row above for what this run did.",
    ],
    "go-x-tools-rta-callgraph": [
        "Oracle: rapid type analysis (RTA) call-graph edges from `golang.org/x/tools`, "
        "scored edge by edge.",
        "PARTIAL ORACLE. The expected-edge set is small (tens of edges) relative to "
        "the edges polint reports over the same code, so precision here is dominated "
        "by edges the oracle simply does not enumerate. Read recall and precision "
        "separately; do not read this precision as a false-positive rate.",
    ],
}


def read_json(path: Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        with path.open(encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, json.JSONDecodeError):
        return None


def read_text(path: Path) -> str | None:
    if not path.is_file():
        return None
    try:
        return path.read_text(encoding="utf-8").strip()
    except OSError:
        return None


def mib(value: object) -> str:
    if not isinstance(value, (int, float)):
        return "-"
    return f"{value / 1024 / 1024:.0f}"


def seconds(value: object) -> str:
    if not isinstance(value, (int, float)):
        return "-"
    return f"{value / 1000:.2f}"


def pct(value: object) -> str:
    if not isinstance(value, (int, float)):
        return "n/a"
    return f"{value * 100:.2f}%"


def f1_of(metrics: dict) -> float | None:
    if isinstance(metrics.get("f1"), (int, float)):
        return float(metrics["f1"])
    precision, recall = metrics.get("precision"), metrics.get("recall")
    if isinstance(precision, (int, float)) and isinstance(recall, (int, float)):
        if precision + recall > 0:
            return 2 * precision * recall / (precision + recall)
    return None


def render_header(out: list[str], environment: dict | None, context: dict | None) -> None:
    out.append("# polint benchmark run")
    out.append("")
    if environment:
        polint = environment.get("polint_under_test", {})
        dirty = " (dirty worktree)" if polint.get("worktree_dirty") else ""
        out.append(
            f"**polint under test:** `{polint.get('crate_version', '?')}` at commit "
            f"`{polint.get('commit', '?')}` on `{polint.get('branch', '?')}`{dirty}  "
        )
        out.append(f"**Binary reports:** `{polint.get('binary_version', '?')}`  ")
        out.append(
            f"**Measured on:** {environment.get('context', '?')} at "
            f"{environment.get('captured_at_utc', '?')}  "
        )
        run_url = (environment.get("runner") or {}).get("run_url")
        if run_url:
            out.append(f"**Run:** {run_url}  ")
    if context:
        out.append(f"**Command:** `{context.get('command', '?')}`  ")
    out.append("")
    out.append(
        "Every corpus below is public open source at a pinned commit. Accuracy and "
        "performance are reported separately and are never combined into a single score."
    )
    out.append("")


def render_environment(out: list[str], environment_md: str | None) -> None:
    out.append("## Machine")
    out.append("")
    if environment_md:
        out.append(environment_md)
    else:
        out.append("_environment.md missing; the run did not log its machine._")
    out.append("")


def render_corpus(out: list[str], perf: dict | None, pins: str | None) -> None:
    out.append("## Corpus")
    out.append("")
    out.append("| target | repository | pinned commit | license | LOC | source files | group |")
    out.append("| --- | --- | --- | --- | ---: | ---: | --- |")
    for target in (perf or {}).get("targets", []):
        url = str(target.get("source_url", "")).removeprefix("https://github.com/")
        commit = str(target.get("checkout_commit") or target.get("source_commit") or "")[:12]
        loc = target.get("loc")
        out.append(
            f"| `{target.get('id')}` | {url} | `{commit}` | {target.get('license')} | "
            f"{loc if loc is not None else '-'} | {target.get('source_file_count', '-')} | "
            f"{target.get('group')} |"
        )
    out.append("")
    excluded = (perf or {}).get("excluded", [])
    if excluded:
        out.append("Not measured in this run:")
        out.append("")
        for item in excluded:
            out.append(f"- `{item.get('id')}` ({item.get('license')}): {item.get('reason')}")
        out.append("")
    if pins:
        out.append("<details><summary>Pin digests (id, commit, url, checkout, digest)</summary>")
        out.append("")
        out.append("```")
        out.append(pins)
        out.append("```")
        out.append("")
        out.append("</details>")
        out.append("")


def render_accuracy(out: list[str], accuracy_dir: Path, status: dict | None) -> None:
    out.append("## Accuracy (call-graph oracles)")
    out.append("")
    if status is None or not status.get("ran"):
        out.append(
            "_Not run. The accuracy suites are scored by `eval-gate.yml` / `make eval-gate`._"
        )
        out.append("")
        return

    npm = status.get("npm_tree") or {}
    out.append("| condition | value |")
    out.append("| --- | --- |")
    out.append(f"| tier | {status.get('tier', '?')} |")
    out.append(
        f"| Jelly `tests/helloworld` npm tree | **{npm.get('status', 'unknown')}** - "
        f"{npm.get('detail', '')} |"
    )
    out.append("")

    # `committed-baseline.json` sits in the same directory and matches the same
    # glob; it is the gate's reference, not a measured run.
    runs = []
    for path in sorted(accuracy_dir.glob("*-baseline.json")):
        if path.name == "committed-baseline.json":
            continue
        run = read_json(path)
        if run and run.get("suite_id"):
            runs.append(run)
    if not runs:
        out.append(
            f"_The accuracy step exited {status.get('exit_code')} and produced no per-suite "
            "report. Nothing measured, so nothing is claimed here._"
        )
        out.append("")
        return

    out.append(
        "| suite | cases | expected edges | observed edges | TP | FP | FN | precision | "
        "recall | F1 | unknowns |"
    )
    out.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
    for run in runs:
        metrics = run.get("metrics", {})
        f1 = f1_of(metrics)
        out.append(
            f"| `{run['suite_id']}` | {len(run.get('cases', []))} | "
            f"{metrics.get('graph_edges_expected', '-')} | "
            f"{metrics.get('graph_edges_observed', '-')} | "
            f"{metrics.get('true_positives', '-')} | {metrics.get('false_positives', '-')} | "
            f"{metrics.get('false_negatives', '-')} | {pct(metrics.get('precision'))} | "
            f"{pct(metrics.get('recall'))} | {pct(f1)} | {metrics.get('unknown_count', '-')} |"
        )
    out.append("")

    gate = "passed" if status.get("exit_code") == 0 else "FAILED"
    out.append(
        f"Regression gate against the committed "
        f"`research/evaluation-harness/baselines/persisted-graph-accuracy.json`: **{gate}** "
        f"(process exit {status.get('exit_code')}). This report never fails the job on it; "
        "`eval-gate.yml` is the workflow that gates."
    )
    out.append("")

    committed = read_json(accuracy_dir / "committed-baseline.json")
    if committed:
        rows = {row["suite_id"]: row for row in committed.get("rows", [])}
        out.append("Measured against the committed baseline:")
        out.append("")
        out.append(
            "| suite | measured recall | baseline recall | measured precision | "
            "baseline precision | measured F1 | baseline F1 | measured edges observed | "
            "baseline edges observed |"
        )
        out.append("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |")
        for run in runs:
            row = rows.get(run["suite_id"], {})
            metrics = run.get("metrics", {})
            out.append(
                f"| `{run['suite_id']}` | {pct(metrics.get('recall'))} | "
                f"{pct(row.get('recall'))} | {pct(metrics.get('precision'))} | "
                f"{pct(row.get('precision'))} | {pct(f1_of(metrics))} | "
                f"{pct(f1_of(row))} | {metrics.get('graph_edges_observed', '-')} | "
                f"{row.get('graph_edges_observed', '-')} |"
            )
        out.append("")
        out.append(
            "A baseline is only a fair comparison when it was produced by the same scoring "
            "code. Check `git log` on the baseline file against the eval harness before "
            "reading a gap here as an engine regression."
        )
        out.append("")

    out.append("### What these numbers do and do not establish")
    out.append("")
    for run in runs:
        out.append(f"**`{run['suite_id']}`**")
        out.append("")
        for note in ORACLE_NOTES.get(run["suite_id"], []):
            out.append(f"- {note}")
        for note in run.get("limitations", []):
            out.append(f"- Reported by the suite: {note}")
        out.append("")


def render_perf(out: list[str], perf: dict | None) -> None:
    out.append("## Performance")
    out.append("")
    if not perf:
        out.append("_perf.json missing; no performance matrix was measured._")
        out.append("")
        return

    out.append(
        f"{perf.get('runs_per_cell')} sequential runs per cell, "
        f"{perf.get('timeout_seconds')}s per-run timeout. "
        f"Wall clock: {perf['measurement']['wall_clock']}. "
        f"Peak RSS: {perf['measurement']['peak_rss']}."
    )
    out.append("")

    for workload in perf.get("workloads", []):
        out.append(f"### Workload `{workload['id']}` - {workload['label']}")
        out.append("")
        out.append(f"`{workload['command']}`")
        out.append("")
        out.append(workload["note"])
        out.append("")
        out.append(
            "| target | tier | median wall | samples (s) | median peak RSS | samples (MiB) "
            "| cache after (MiB) | status |"
        )
        out.append("| --- | --- | ---: | --- | ---: | --- | ---: | --- |")
        measured_any = False
        for target in perf.get("targets", []):
            cells = [
                cell for cell in target.get("cells", []) if cell["workload"] == workload["id"]
            ]
            for cell in cells:
                measured_any = True
                runs = cell.get("runs", [])
                wall_samples = ", ".join(seconds(run["wall_ms"]) for run in runs) or "-"
                rss_samples = ", ".join(mib(run["peak_rss_bytes"]) for run in runs) or "-"
                status = cell["status"] if cell["status"] == "ok" else f"**{cell['status']}**"
                out.append(
                    f"| `{target['id']}` | {cell['tier']} | "
                    f"{seconds(cell.get('wall_ms_median'))} s | {wall_samples} | "
                    f"{mib(cell.get('peak_rss_bytes_median'))} MiB | {rss_samples} | "
                    f"{mib(cell.get('cache_bytes_median'))} | {status} |"
                )
        if not measured_any:
            out.append("| _none_ | - | - | - | - | - | - | not measured |")
        out.append("")

        failures = [
            (target["id"], cell)
            for target in perf.get("targets", [])
            for cell in target.get("cells", [])
            if cell["workload"] == workload["id"] and cell["status"] != "ok"
        ]
        for target_id, cell in failures:
            out.append(
                f"- `{target_id}` / {cell['tier']}: **{cell['status']}** - {cell.get('detail')}"
            )
        if failures:
            out.append("")

    skipped = [
        (target["id"], skip)
        for target in perf.get("targets", [])
        for skip in target.get("skipped_workloads", [])
    ]
    if skipped:
        out.append("Workloads not run:")
        out.append("")
        for target_id, skip in skipped:
            out.append(f"- `{target_id}` / `{skip['workload']}`: {skip['reason']}")
        out.append("")

    missing = [
        target for target in perf.get("targets", []) if target.get("skip_reason")
    ]
    for target in missing:
        out.append(f"- `{target['id']}`: {target['skip_reason']}")
    if missing:
        out.append("")


def render_output_identity(out: list[str], perf: dict | None) -> None:
    out.append("## Output identity")
    out.append("")
    out.append(
        "The md5 of stdout for one target and workload must be the same across the warm, "
        "cold, and no-cache tiers, and the same across runs of the same commit. A digest "
        "that moves without the commit moving is a caching or nondeterminism bug, not a "
        "performance result."
    )
    out.append("")
    rows = []
    for target in (perf or {}).get("targets", []):
        for workload_id, identity in (target.get("output_identity") or {}).items():
            stable = identity.get("identical_across_tiers")
            digests = identity.get("digests", [])
            rows.append(
                f"| `{target['id']}` | `{workload_id}` | "
                f"{'yes' if stable else '**NO**'} | "
                f"{', '.join(f'`{digest}`' for digest in digests)} |"
            )
    if rows:
        out.append("| target | workload | identical across tiers | md5 |")
        out.append("| --- | --- | --- | --- |")
        out.extend(rows)
    else:
        out.append("_No completed cells to compare._")
    out.append("")


def render_build_cost(out: list[str], report: dict | None, status: dict | None) -> None:
    out.append("## Rule-host build cost")
    out.append("")
    if status is not None and not status.get("ran"):
        out.append(f"_Not run: {status.get('reason', 'not selected')}._")
        out.append("")
        return
    if not report:
        out.append("_Not run._")
        out.append("")
        return
    out.append(
        "What a repo-local rule pack costs to compile, measured by "
        "`polint-bench build-cost`. This is the cost `polint check` pays that the "
        "capability-gated performance workloads above do not include."
    )
    out.append("")
    out.append(
        "| repo | scenario | status | cargo wall clock | compiled units | rules target bytes |"
    )
    out.append("| --- | --- | --- | ---: | ---: | ---: |")
    for cell in report.get("cells", []):
        runs = cell.get("runs", [])
        metrics = runs[0].get("metrics", {}) if runs else {}
        out.append(
            f"| `{cell.get('repo')}` | {cell.get('scenario')} | {cell.get('status')} | "
            f"{seconds(metrics.get('cargo_wall_clock_ms'))} s | "
            f"{metrics.get('compiled_units', '-')} | "
            f"{mib(metrics.get('rules_target_bytes_written'))} MiB |"
        )
    out.append("")
    for limit in report.get("limits", []):
        out.append(f"- {limit}")
    out.append("")


def render_interpretation(out: list[str], how_to_interpret: str | None) -> None:
    out.append("## How to interpret this run")
    out.append("")
    if how_to_interpret:
        out.append(
            "The artifact ships `how-to-interpret.md` with the full description of every "
            "benchmark, its oracle, and its limitations. The short version:"
        )
        out.append("")
        # Surface the "short version" bullets the document marks for the summary.
        marker = "<!-- summary-bullets -->"
        if marker in how_to_interpret:
            block = how_to_interpret.split(marker)[1].split("<!-- /summary-bullets -->")[0]
            out.append(block.strip())
        out.append("")
        out.append("<details><summary>Full how-to-interpret.md</summary>")
        out.append("")
        out.append(how_to_interpret)
        out.append("")
        out.append("</details>")
    else:
        out.append("_how-to-interpret.md missing from this run._")
    out.append("")


def render_reproduce(out: list[str], context: dict | None) -> None:
    out.append("## Reproduce")
    out.append("")
    out.append("```sh")
    out.append("make bench-run                 # the CI default selection")
    out.append("make bench-run SCALE=1         # adds excalidraw and hugo")
    out.append("make bench-run SCALE=1 GRAFANA=1 DEEP_TARGETS=all  # everything, local only")
    out.append("```")
    out.append("")
    if context:
        out.append(
            f"This run: `{context.get('command', '?')}` "
            f"(started {context.get('started_at_utc', '?')}, "
            f"finished {context.get('finished_at_utc', '?')})."
        )
        out.append("")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-dir", type=Path, required=True)
    args = parser.parse_args()
    root = args.artifact_dir

    environment = read_json(root / "environment.json")
    context = read_json(root / "run-context.json")
    perf = read_json(root / "perf.json")
    accuracy_status = read_json(root / "accuracy-status.json")
    build_cost = read_json(root / "build-cost.json")
    build_cost_status = read_json(root / "build-cost-status.json")

    out: list[str] = []
    render_header(out, environment, context)
    render_environment(out, read_text(root / "environment.md"))
    render_corpus(out, perf, read_text(root / "corpus-pins.tsv"))
    render_accuracy(out, root / "accuracy", accuracy_status)
    render_perf(out, perf)
    render_output_identity(out, perf)
    render_build_cost(out, build_cost, build_cost_status)
    render_interpretation(out, read_text(root / "how-to-interpret.md"))
    render_reproduce(out, context)

    text = "\n".join(out).rstrip() + "\n"
    (root / "summary.md").write_text(text, encoding="utf-8")
    print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

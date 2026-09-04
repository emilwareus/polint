# Scale-envelope experiment log

Target: excalidraw v0.17.6 (385 TS/TSX files, 2.5 MB source, 86,527 LOC) full
pipeline (`dataflow` + `file_metrics` + `function_metrics` + `complexity_metrics`)
under **6 GB peak RSS** and **300 s wall**, on this host (8 cores / 15 GB, no swap).

Rule: every experiment gets a hypothesis, a measurement, and a keep-or-revert
verdict. Anything that does not move RSS or wall is reverted the same hour.

## Measurement protocol

- Driver: the eval harness's isolated perf child
  (`eval::bench::runner::tests::perf_child_measure_entry`), release profile.
  This is the same path the committed `scale-corpus-run.json` baseline used.
- Wrapper: `.scale-envelope/rssrun.py` samples whole-process-tree RSS from
  `/proc/<pid>/status` every 200 ms and enforces an `RLIMIT_AS` guard so a
  runaway run cannot take the host down.
- `.polint/cache` in the corpus checkout is deleted before every cold run.
- Load average checked < 2 before every timed run; runs are sequential and
  exclusive.


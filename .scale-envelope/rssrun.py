#!/usr/bin/env python3
"""Run a command under an RSS sampler with a hard address-space guard.

Samples /proc/<pid>/status for VmRSS/VmHWM every INTERVAL seconds (whole
process tree: the target plus any children), records a timeline, and reports
peak RSS + wall clock. A RLIMIT_AS guard keeps a runaway run from taking the
host down; the child dies with an allocation failure instead of an OOM kill.
"""
from __future__ import annotations

import argparse
import json
import os
import resource
import signal
import subprocess
import sys
import time

INTERVAL = 0.2


def read_status(pid: int) -> tuple[int, int]:
    """(VmRSS, VmHWM) in bytes for one pid; (0, 0) if it is gone."""
    try:
        with open(f"/proc/{pid}/status", "rb") as handle:
            rss = hwm = 0
            for line in handle:
                if line.startswith(b"VmRSS:"):
                    rss = int(line.split()[1]) * 1024
                elif line.startswith(b"VmHWM:"):
                    hwm = int(line.split()[1]) * 1024
                if rss and hwm:
                    break
            return rss, hwm
    except (OSError, ValueError, IndexError):
        return 0, 0


def descendants(root: int) -> list[int]:
    pids = [root]
    seen = {root}
    frontier = [root]
    while frontier:
        parent = frontier.pop()
        try:
            with open(f"/proc/{parent}/task/{parent}/children", "rb") as handle:
                kids = [int(tok) for tok in handle.read().split()]
        except (OSError, ValueError):
            continue
        for kid in kids:
            if kid not in seen:
                seen.add(kid)
                pids.append(kid)
                frontier.append(kid)
    return pids


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--label", default="run")
    parser.add_argument("--timeline", default=None, help="write a JSON sample timeline here")
    parser.add_argument("--as-limit-gb", type=float, default=11.0)
    parser.add_argument("--timeout", type=float, default=3600.0)
    parser.add_argument("cmd", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    cmd = args.cmd[1:] if args.cmd and args.cmd[0] == "--" else args.cmd
    if not cmd:
        print("no command", file=sys.stderr)
        return 2

    limit = int(args.as_limit_gb * (1 << 30))

    def preexec() -> None:
        resource.setrlimit(resource.RLIMIT_AS, (limit, limit))
        os.setsid()

    started = time.monotonic()
    proc = subprocess.Popen(cmd, preexec_fn=preexec)
    samples: list[tuple[float, int]] = []
    peak = 0
    try:
        while proc.poll() is None:
            total = 0
            for pid in descendants(proc.pid):
                rss, _ = read_status(pid)
                total += rss
            if total:
                now = round(time.monotonic() - started, 3)
                samples.append((now, total))
                peak = max(peak, total)
            if time.monotonic() - started > args.timeout:
                os.killpg(proc.pid, signal.SIGKILL)
                break
            time.sleep(INTERVAL)
    except KeyboardInterrupt:
        os.killpg(proc.pid, signal.SIGKILL)
        raise
    code = proc.wait()
    wall = time.monotonic() - started

    summary = {
        "label": args.label,
        "cmd": cmd,
        "exit_code": code,
        "wall_s": round(wall, 2),
        "peak_rss_bytes": peak,
        "peak_rss_gb": round(peak / (1 << 30), 3),
        "samples": len(samples),
    }
    print(json.dumps(summary), file=sys.stderr)
    if args.timeline:
        with open(args.timeline, "w", encoding="utf-8") as handle:
            json.dump({"summary": summary, "timeline": samples}, handle)
    return 0 if code == 0 else 1


if __name__ == "__main__":
    sys.exit(main())

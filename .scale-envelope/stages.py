#!/usr/bin/env python3
"""Summarize a stage trace (run stderr) into a per-provider attribution table."""
import re, sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
PAT = re.compile(
    r'provider="?([\w.]+)"?\s+elapsed_ms=(\d+)\s+rss_mb=(\d+)\s+rss_delta_mb=(\d+)'
    r'\s+peak_rss_mb=(\d+)(?:\s+facts=(\d+)\s+keys=(\d+)\s+key_mb=(\d+))?'
)

rows = []
for line in open(sys.argv[1], errors="replace"):
    m = PAT.search(ANSI.sub("", line))
    if m:
        g = m.groups()
        rows.append((g[0], *(int(x) if x is not None else 0 for x in g[1:])))

tw = sum(r[1] for r in rows) or 1
peak = max((r[4] for r in rows), default=0)
hdr = f"{'provider':<26}{'ms':>8}{'wall%':>7}{'rss_MB':>8}{'dRSS':>7}{'peak':>7}{'facts':>11}{'keys':>11}{'keyMB':>7}"
print(hdr)
prev_f = prev_k = 0
for name, ms, rss, drss, pk, facts, keys, keymb in rows:
    print(f"{name:<26}{ms:>8}{100*ms/tw:>6.1f}%{rss:>8}{drss:>7}{pk:>7}{facts:>11}{keys:>11}{keymb:>7}")
    prev_f, prev_k = facts, keys
print(f"{'TOTAL':<26}{tw:>8}{100.0:>6.1f}%{'':>8}{'':>7}{peak:>7}{prev_f:>11}{prev_k:>11}")

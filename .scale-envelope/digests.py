#!/usr/bin/env python3
"""Compare per-provider output digests between two runs (fact-level identity)."""
import re, sys

ANSI = re.compile(r"\x1b\[[0-9;]*m")
PAT = re.compile(r'provider="?([\w.]+)"?.*?digest="?([\w-]+)"?')

def load(path):
    out = {}
    for line in open(path, errors="replace"):
        m = PAT.search(ANSI.sub("", line))
        if m:
            out[m.group(1)] = m.group(2)
    return out

a, b = load(sys.argv[1]), load(sys.argv[2])
bad = 0
for k in a:
    if k not in b:
        print(f"MISSING  {k}"); bad += 1
    elif a[k] != b[k]:
        print(f"DIFFER   {k}: {a[k]} -> {b[k]}"); bad += 1
print(f"{len(a) - bad}/{len(a)} provider output digests identical"
      + ("" if not bad else f"  ({bad} differ)"))
sys.exit(1 if bad else 0)

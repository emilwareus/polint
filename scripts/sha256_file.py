#!/usr/bin/env python3
"""Emit one SHA256 checksum line in GNU `sha256sum` text format.

Used by release packaging (GitHub Actions and `release-local-check.sh`).
Output format: ``<hexdigest>  <basename>\\n``

Examples::

    python3 scripts/sha256_file.py dist/polint-linux-x86_64.tar.gz \\
        -o dist/polint-linux-x86_64.tar.gz.sha256

    python3 scripts/sha256_file.py dist/archive.tar.gz > dist/archive.tar.gz.sha256
"""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path


def checksum_line(path: Path) -> str:
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return f"{digest}  {path.name}\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument(
        "path",
        type=Path,
        help="file to hash",
    )
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=None,
        help="write checksum line to this file (default: stdout)",
    )
    args = parser.parse_args(argv)

    if not args.path.is_file():
        print(f"error: not a file: {args.path}", file=sys.stderr)
        return 1

    line = checksum_line(args.path)
    if args.output is not None:
        args.output.write_text(line, encoding="utf-8")
    else:
        sys.stdout.write(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

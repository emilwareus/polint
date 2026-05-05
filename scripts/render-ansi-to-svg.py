#!/usr/bin/env python3
"""Render ANSI-colored text on stdin to a self-contained SVG on stdout.

Used to embed colored polint output in the README so it renders on every
markdown host (GitHub, crates.io, IDE previews) — not just github.com's
``ansi`` code blocks. Supports the SGR subset polint emits today: 0 (reset),
1 (bold), 2 (dim/faint), and the standard 30-37 / 90-97 foreground colors.

Usage::

    polint check --color always | scripts/render-ansi-to-svg.py > out.svg
    scripts/render-ansi-to-svg.py < some-fixture.ansi > out.svg

The SVG embeds a dark "terminal" background with a window-chrome traffic-light
header and renders text as ``<tspan>`` runs so colors and weights survive in
any modern viewer.
"""

from __future__ import annotations

import argparse
import re
import sys
from html import escape

# Catppuccin-ish palette tuned for dark terminal contrast.
COLORS = {
    30: "#45475a", 31: "#f38ba8", 32: "#a6e3a1", 33: "#f9e2af",
    34: "#89b4fa", 35: "#cba6f7", 36: "#94e2d5", 37: "#cdd6f4",
    90: "#6c7086", 91: "#f38ba8", 92: "#a6e3a1", 93: "#f9e2af",
    94: "#89b4fa", 95: "#cba6f7", 96: "#94e2d5", 97: "#ffffff",
}

BG = "#1e1e2e"
FG = "#cdd6f4"
HEADER_FG = "#9399b2"
TRAFFIC = ("#f38ba8", "#f9e2af", "#a6e3a1")


class Style:
    __slots__ = ("bold", "dim", "fg")

    def __init__(self) -> None:
        self.bold = False
        self.dim = False
        self.fg: str | None = None

    def apply(self, params: list[int]) -> None:
        if not params:
            params = [0]
        for p in params:
            if p == 0:
                self.bold = self.dim = False
                self.fg = None
            elif p == 1:
                self.bold = True
            elif p == 2:
                self.dim = True
            elif p == 22:
                self.bold = self.dim = False
            elif p == 39:
                self.fg = None
            elif p in COLORS:
                self.fg = COLORS[p]

    def attrs(self, default_fg: str) -> dict[str, str]:
        a: dict[str, str] = {"fill": self.fg or default_fg}
        if self.bold:
            a["font-weight"] = "bold"
        if self.dim:
            a["opacity"] = "0.55"
        return a


SGR = re.compile(r"\x1b\[([0-9;]*)m")
ANSI_ANY = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def parse_line(line: str, style: Style) -> tuple[list[tuple[Style, str]], Style]:
    """Return ``(chunks, final_style)`` for ``line`` starting from ``style``.

    Trailing SGR sequences with no following text (e.g. ``\\x1b[0m`` at end of
    line) still propagate through ``final_style`` so subsequent lines start
    with the correct state.
    """
    chunks: list[tuple[Style, str]] = []
    pos = 0
    for m in SGR.finditer(line):
        if m.start() > pos:
            chunks.append((style, line[pos : m.start()]))
        raw = m.group(1)
        params = [int(p) for p in raw.split(";") if p] if raw else [0]
        new = Style()
        new.bold, new.dim, new.fg = style.bold, style.dim, style.fg
        new.apply(params)
        style = new
        pos = m.end()
    if pos < len(line):
        chunks.append((style, line[pos:]))
    return chunks, style


def visible_len(line: str) -> int:
    return len(ANSI_ANY.sub("", line))


def render(
    text: str,
    *,
    title: str = "polint check",
    font_size: int = 14,
    line_height: int = 20,
    padding_x: int = 22,
    padding_y: int = 22,
    header_h: int = 36,
) -> str:
    lines = text.splitlines() or [""]
    char_w = font_size * 0.62  # tuned for typical monospace
    max_chars = max((visible_len(line) for line in lines), default=0)
    width = int(padding_x * 2 + max_chars * char_w)
    width = max(width, 460)
    body_h = padding_y * 2 + len(lines) * line_height
    height = header_h + body_h

    svg: list[str] = []
    svg.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" '
        f'role="img" aria-label="{escape(title)}">'
    )
    svg.append('<defs><style>'
               '.t { font-family: ui-monospace, SFMono-Regular, "JetBrains Mono", '
               '"Fira Code", Menlo, Consolas, monospace; '
               f'font-size: {font_size}px; }}'
               '</style></defs>')
    svg.append(f'<rect width="100%" height="100%" rx="8" ry="8" fill="{BG}"/>')
    # Header bar
    svg.append(
        f'<rect x="0" y="0" width="{width}" height="{header_h}" '
        f'fill="#181825" rx="8" ry="8"/>'
    )
    svg.append(
        f'<rect x="0" y="{header_h - 8}" width="{width}" height="8" fill="#181825"/>'
    )
    cx = 18
    for color in TRAFFIC:
        svg.append(f'<circle cx="{cx}" cy="{header_h // 2}" r="6" fill="{color}"/>')
        cx += 18
    svg.append(
        f'<text x="{width // 2}" y="{header_h // 2 + 4}" class="t" '
        f'fill="{HEADER_FG}" text-anchor="middle">{escape(title)}</text>'
    )

    style = Style()
    y = header_h + padding_y + font_size
    for line in lines:
        x = padding_x
        svg.append(f'<text x="{x}" y="{y}" class="t" fill="{FG}" xml:space="preserve">')
        chunks, style = parse_line(line, style)
        if not chunks:
            svg.append('<tspan> </tspan>')  # keep empty lines visible
        else:
            for st, chunk in chunks:
                if not chunk:
                    continue
                attrs = st.attrs(FG)
                attr_str = " ".join(f'{k}="{v}"' for k, v in attrs.items())
                svg.append(f'<tspan {attr_str}>{escape(chunk)}</tspan>')
        svg.append('</text>')
        y += line_height
    svg.append('</svg>')
    return "\n".join(svg)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--title", default="polint check", help="window title")
    parser.add_argument("--input", "-i", help="input file (defaults to stdin)")
    parser.add_argument("--output", "-o", help="output file (defaults to stdout)")
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r", encoding="utf-8") as fh:
            text = fh.read()
    else:
        text = sys.stdin.read()

    svg = render(text, title=args.title)
    if args.output:
        with open(args.output, "w", encoding="utf-8") as fh:
            fh.write(svg + "\n")
    else:
        sys.stdout.write(svg + "\n")


if __name__ == "__main__":
    main()

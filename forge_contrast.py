#!/usr/bin/env python3
"""forge_contrast.py — WCAG 2.1 contrast check on Loom token pairs.

Reads loom-tokens.css, extracts every --loom-color-* token (light +
dark theme), and computes WCAG relative-luminance contrast ratio for
common (text, bg) pairs. Strict-fails any pair < 4.5:1 (AA normal
text). Warn-only for < 3.0:1 (AA large text floor — anything below
this is illegible regardless of size).

Output: structured JSON to stdout for forge.sh to ingest, plus
human-readable lines to stderr.

Usage:
    python3 forge_contrast.py /tmp/skillshots-poc/static/loom-tokens.css

Exit codes:
    0 — all checked pairs pass AA
    1 — at least one pair fails strict (< 4.5:1)
    2 — usage error
"""
import json
import re
import sys
from typing import Dict, List, Tuple


# ---- WCAG color math --------------------------------------------------

def hsl_to_rgb(h: float, s: float, l: float) -> Tuple[float, float, float]:
    """HSL (h: 0-360, s: 0-1, l: 0-1) → RGB (0-1, 0-1, 0-1)."""
    s = max(0.0, min(1.0, s))
    l = max(0.0, min(1.0, l))
    c = (1 - abs(2 * l - 1)) * s
    x = c * (1 - abs(((h / 60) % 2) - 1))
    m = l - c / 2
    if 0 <= h < 60:    r1, g1, b1 = c, x, 0
    elif 60 <= h < 120:  r1, g1, b1 = x, c, 0
    elif 120 <= h < 180: r1, g1, b1 = 0, c, x
    elif 180 <= h < 240: r1, g1, b1 = 0, x, c
    elif 240 <= h < 300: r1, g1, b1 = x, 0, c
    else:                r1, g1, b1 = c, 0, x
    return (r1 + m, g1 + m, b1 + m)


def hex_to_rgb(hex_str: str) -> Tuple[float, float, float]:
    """`#rrggbb` or `#rgb` → RGB (0-1)."""
    h = hex_str.lstrip("#")
    if len(h) == 3:
        h = "".join(c * 2 for c in h)
    if len(h) != 6:
        raise ValueError(f"bad hex: {hex_str}")
    r = int(h[0:2], 16) / 255
    g = int(h[2:4], 16) / 255
    b = int(h[4:6], 16) / 255
    return (r, g, b)


def relative_luminance(rgb: Tuple[float, float, float]) -> float:
    """WCAG 2.1 relative luminance — Y in CIE XYZ, ish."""
    def chan(c: float) -> float:
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    r, g, b = rgb
    return 0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)


def contrast_ratio(fg: Tuple[float, float, float], bg: Tuple[float, float, float]) -> float:
    """WCAG contrast ratio — (L1 + 0.05) / (L2 + 0.05), L1 = lighter."""
    l1 = relative_luminance(fg)
    l2 = relative_luminance(bg)
    if l1 < l2:
        l1, l2 = l2, l1
    return (l1 + 0.05) / (l2 + 0.05)


# ---- Token-CSS parser -------------------------------------------------

# `--loom-color-x: hsl(220 90% 28%);` or `: #ffffff;`
COLOR_DECL_RE = re.compile(
    r"--loom-color-([a-z0-9-]+)\s*:\s*(hsl\([^)]+\)|#[0-9a-fA-F]+)\s*;"
)
# `:root` or `:root[data-theme="dark"]` or `:root[data-theme=name]`
SELECTOR_RE = re.compile(r":root(?:\[data-theme[~^|*$]?=\"?([^\"\]]+)\"?\])?\s*\{")
HSL_RE = re.compile(
    r"hsl\(\s*([\d.]+)\s+([\d.]+)%\s+([\d.]+)%(?:\s*/\s*[\d.]+)?\s*\)"
)


def parse_tokens(css: str) -> Dict[str, Dict[str, Tuple[float, float, float]]]:
    """Return: { theme: { token_name: rgb_tuple } }. theme key 'light' = no data-theme."""
    out: Dict[str, Dict[str, Tuple[float, float, float]]] = {}
    cursor = 0
    while True:
        m = SELECTOR_RE.search(css, cursor)
        if not m:
            break
        theme = m.group(1) or "light"
        block_start = m.end()
        depth = 1
        i = block_start
        while i < len(css) and depth > 0:
            if css[i] == "{": depth += 1
            elif css[i] == "}": depth -= 1
            i += 1
        block = css[block_start:i - 1]
        cursor = i
        out.setdefault(theme, {})
        for d in COLOR_DECL_RE.finditer(block):
            name, val = d.group(1), d.group(2).strip()
            try:
                if val.startswith("#"):
                    rgb = hex_to_rgb(val)
                else:
                    hm = HSL_RE.search(val)
                    if not hm:
                        continue
                    rgb = hsl_to_rgb(float(hm.group(1)), float(hm.group(2)) / 100, float(hm.group(3)) / 100)
                out[theme][name] = rgb
            except (ValueError, IndexError):
                continue
    return out


# ---- Pairs we care about ---------------------------------------------

# Each entry: (fg_token, bg_token, label, min_ratio).
# min_ratio 4.5 = AA normal text. 3.0 = AA UI/large text.
PAIRS: List[Tuple[str, str, str, float]] = [
    ("ink", "bg-canvas", "body text on canvas", 4.5),
    ("ink", "surface", "body text on card surface", 4.5),
    ("ink", "surface-muted", "body text on muted surface", 4.5),
    ("ink-muted", "bg-canvas", "muted text on canvas", 4.5),
    ("ink-muted", "surface", "muted text on card surface", 4.5),
    ("ink-muted", "surface-muted", "muted text on muted surface", 4.5),
    ("primary-fg", "primary", "button text on primary bg", 4.5),
    ("ink", "warn-bg", "ink on warn callout bg", 4.5),
    ("ink", "bg-overlay", "ink on overlay surface", 4.5),
    ("ink-muted", "bg-overlay", "muted ink on overlay surface", 4.5),
    ("border", "bg-canvas", "border on canvas (UI element)", 3.0),
    ("border-strong", "bg-canvas", "strong border on canvas", 3.0),
    ("danger", "bg-canvas", "danger color on canvas", 3.0),
    ("success", "bg-canvas", "success color on canvas", 3.0),
    ("warn", "bg-canvas", "warn color on canvas", 3.0),
]


# ---- Main -------------------------------------------------------------

def main(argv: List[str]) -> int:
    if len(argv) < 2:
        sys.stderr.write("usage: forge_contrast.py <path-to-loom-tokens.css>\n")
        return 2
    try:
        with open(argv[1], encoding="utf-8") as f:
            css = f.read()
    except OSError as e:
        sys.stderr.write(f"error: cannot read {argv[1]}: {e}\n")
        return 2

    themes = parse_tokens(css)
    if not themes:
        sys.stderr.write("error: no token blocks parsed from tokens.css\n")
        return 2

    findings: List[Dict] = []
    strict_count = 0
    warn_count = 0

    for theme in sorted(themes.keys()):
        tokens = themes[theme]
        for fg_name, bg_name, label, min_ratio in PAIRS:
            fg = tokens.get(fg_name)
            bg = tokens.get(bg_name)
            if fg is None or bg is None:
                # Token may not exist in this theme — skip silently.
                continue
            ratio = contrast_ratio(fg, bg)
            severity = None
            if ratio < min_ratio:
                # Strict iff fails the headline AA threshold (4.5).
                # AA-large 3.0 floor for UI elements is "warn".
                severity = "strict" if min_ratio >= 4.5 else "warn"
                if severity == "strict":
                    strict_count += 1
                else:
                    warn_count += 1
                findings.append({
                    "severity": severity,
                    "theme": theme,
                    "fg_token": fg_name,
                    "bg_token": bg_name,
                    "label": label,
                    "ratio": round(ratio, 2),
                    "min_required": min_ratio,
                })
                sys.stderr.write(
                    f"  {severity.upper()}  {theme:>5}  {ratio:5.2f}:1 (need {min_ratio}) "
                    f"{fg_name} on {bg_name}  ({label})\n"
                )

    summary = {
        "themes_checked": sorted(themes.keys()),
        "pairs_tested_per_theme": len(PAIRS),
        "strict_findings": strict_count,
        "warn_findings": warn_count,
        "findings": findings,
    }
    print(json.dumps(summary, indent=2))
    return 0 if strict_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))

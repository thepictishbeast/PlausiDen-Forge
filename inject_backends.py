#!/usr/bin/env python3
"""inject_backends.py — wire data-backend attrs to buttons/links.

Maps button text → backend ID by hand-curated dictionary so we
don't accidentally bind the wrong UI to the wrong API. Idempotent:
buttons that already have data-backend are left alone.

After running, both the `phantom_button` warns (no data-backend on
buttons) and `backend_coverage` warns (no UI references this backend)
should drop substantially.
"""
import os
import re
import sys

# (regex matched in the button's text content, backend-id).
# Order matters — first match wins. Buttons whose text doesn't
# match any pattern are left alone (handled by the doctrine that
# requires explicit declaration).
TEXT_TO_BACKEND = [
    (r"\bsign\s*in\b",           "sign-in"),
    (r"\bsign\s*up\b",           "sign-up"),
    (r"\bsign\s*out\b",          "sign-out"),
    (r"\blog\s*out\b",           "sign-out"),
    (r"\bpost\s*(?:a\s*)?skill\b",  "post-skill"),
    (r"\bsubmit\s*entry\b|^submit$",  "upload-entry"),
    (r"\bsave\s*draft\b",        "upload-entry"),
    (r"\bcash\s*out\b",          "cash-out"),
    (r"\bvote\b|\bwatch\s*[+&]?\s*vote\b", "cast-vote"),
    (r"\benter\s*for\b|\benter\s*challenge\b", "enter-challenge"),
    (r"\bfollow\b",              "follow"),
    (r"\bunfollow\b",            "follow"),
    (r"\breport\s*this\s*challenge\b|\breport\s*challenge\b", "report-challenge"),
    (r"\breport\s*[@\w]+\b|\breport\s*profile\b",   "report-profile"),
    (r"\ball\s*categories\b|\b(basketball|skate|parkour|mind|billiards|darts|cafe)\b", "list-challenges"),
    (r"\b(live|voting|closing\s*soon|by\s*me)\b",   "list-challenges"),
    (r"\bin\s*progress\b|\bclosed\b",               "list-touches"),
    (r"\ball\b|\bwins\b|\blosses\b",                "view-profile"),
    (r"\brandom\s*order\b|\bmost\s*recent\b",       "view-challenge"),
]

# Anchor link text → backend-id. Anchors often *navigate* to a page
# that itself fetches a backend on load (e.g. /leaderboard.html).
ANCHOR_TO_BACKEND = [
    (r"^battle\s*feed$",   "list-challenges"),
    (r"^leaderboard$",     "list-leaderboard"),
    (r"^my\s*wins$",       "list-touches"),
    (r"^profile$",         "view-profile"),
    (r"^post\s*a\s*skill$",  "post-skill"),
    (r"open\s*vote.*",     "list-open-votes"),
]


def assign_backend(html: str) -> tuple[str, int]:
    """Walk every <button> + <a> tag; if text matches a pattern AND
    no data-backend is present, inject one."""
    changes = 0

    def handle_button(m: re.Match) -> str:
        nonlocal changes
        opening = m.group(1)   # the <button ...> tag
        body = m.group(2)      # the inner content
        if "data-backend=" in opening:
            return m.group(0)
        text_only = re.sub(r"<[^>]+>", "", body).strip().lower()
        for pat, backend in TEXT_TO_BACKEND:
            if re.search(pat, text_only):
                changes += 1
                # Insert data-backend before the closing >.
                new_opening = opening.rstrip(">") + f' data-backend="{backend}">'
                return new_opening + body + "</button>"
        return m.group(0)

    def handle_anchor(m: re.Match) -> str:
        nonlocal changes
        opening = m.group(1)
        body = m.group(2)
        if "data-backend=" in opening:
            return m.group(0)
        text_only = re.sub(r"<[^>]+>", "", body).strip().lower()
        for pat, backend in ANCHOR_TO_BACKEND:
            if re.search(pat, text_only):
                changes += 1
                new_opening = opening.rstrip(">") + f' data-backend="{backend}">'
                return new_opening + body + "</a>"
        return m.group(0)

    html = re.sub(
        r"(<button[^>]*>)(.*?)</button>",
        handle_button,
        html,
        flags=re.S,
    )
    html = re.sub(
        r"(<a\b[^>]*>)(.*?)</a>",
        handle_anchor,
        html,
        flags=re.S,
    )
    return html, changes


def main():
    static_dir = "/tmp/skillshots-poc/static"
    total = 0
    for fn in sorted(os.listdir(static_dir)):
        if not fn.endswith(".html"):
            continue
        path = os.path.join(static_dir, fn)
        with open(path, encoding="utf-8") as f:
            src = f.read()
        new_src, n = assign_backend(src)
        if n > 0:
            with open(path, "w", encoding="utf-8") as f:
                f.write(new_src)
            print(f"  {fn}: {n} attrs added")
            total += n
        else:
            print(f"  {fn}: 0")
    print(f"total attrs added: {total}")


if __name__ == "__main__":
    main()

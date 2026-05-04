#!/usr/bin/env python3
"""inject_seo.py — bulk-add SEO meta tags + JSON-LD to every page.

Idempotent: re-running over already-injected files leaves them
unchanged (we look for marker comment <!-- forge:seo:start -->).

Per-page metadata (title, description, og:image) lives in PAGES below.
"""
import os
import re
import sys

BASE = "https://skillshots.example"
DEFAULT_OG_IMAGE = "/og-image.png"  # owner can replace with a real PNG later

PAGES = {
    "index.html": {
        "description": "SkillShots — vote-judged skill challenges with real cash pots. Submit, watch, vote, win.",
        "og_title": "SkillShots — Real challenges. Real stakes. Public results.",
        "twitter_title": "SkillShots",
        "schema_type": "WebSite",
        "canonical_path": "/",
    },
    "challenge.html": {
        "description": "View a SkillShots challenge — entries, voting period, pot, and rules.",
        "og_title": "SkillShots challenge",
        "twitter_title": "SkillShots challenge",
        "schema_type": "WebPage",
        "canonical_path": "/challenge.html",
    },
    "leaderboard.html": {
        "description": "Top SkillShots earners this week. Real cash, vote-judged challenges.",
        "og_title": "SkillShots leaderboard",
        "twitter_title": "SkillShots leaderboard",
        "schema_type": "WebPage",
        "canonical_path": "/leaderboard.html",
    },
    "post-skill.html": {
        "description": "Post a skill challenge on SkillShots. Set a pot, define rules, accept entries.",
        "og_title": "Post a skill — SkillShots",
        "twitter_title": "Post a skill — SkillShots",
        "schema_type": "WebPage",
        "canonical_path": "/post-skill.html",
    },
    "my-wins.html": {
        "description": "Your SkillShots wins, in-progress challenges, and cash-out options.",
        "og_title": "My wins — SkillShots",
        "twitter_title": "My wins — SkillShots",
        "schema_type": "WebPage",
        "canonical_path": "/my-wins.html",
    },
    "profile.html": {
        "description": "SkillShots profile — wins, losses, cash-out history, and challenge stats.",
        "og_title": "Profile — SkillShots",
        "twitter_title": "Profile — SkillShots",
        "schema_type": "ProfilePage",
        "canonical_path": "/profile.html",
    },
}


def build_block(spec: dict) -> str:
    canonical = BASE + spec["canonical_path"]
    schema_json = (
        '{'
        f'"@context":"https://schema.org",'
        f'"@type":"{spec["schema_type"]}",'
        f'"name":"{spec["og_title"]}",'
        f'"description":"{spec["description"]}",'
        f'"url":"{canonical}"'
        '}'
    )
    return (
        '\n  <!-- forge:seo:start -->\n'
        f'  <meta name="description" content="{spec["description"]}">\n'
        f'  <link rel="canonical" href="{canonical}">\n'
        f'  <meta property="og:title" content="{spec["og_title"]}">\n'
        f'  <meta property="og:description" content="{spec["description"]}">\n'
        f'  <meta property="og:type" content="website">\n'
        f'  <meta property="og:url" content="{canonical}">\n'
        f'  <meta property="og:image" content="{BASE}{DEFAULT_OG_IMAGE}">\n'
        f'  <meta name="twitter:card" content="summary_large_image">\n'
        f'  <meta name="twitter:title" content="{spec["twitter_title"]}">\n'
        f'  <meta name="twitter:description" content="{spec["description"]}">\n'
        f'  <script type="application/ld+json">{schema_json}</script>\n'
        '  <!-- forge:seo:end -->\n'
    )


def inject(path: str, spec: dict) -> bool:
    with open(path, encoding="utf-8") as f:
        src = f.read()
    if "<!-- forge:seo:start -->" in src:
        return False  # already injected; skip
    block = build_block(spec)
    # Insert AFTER the <title> tag.
    # Pattern: </title>
    new_src, n = re.subn(r"(</title>)", r"\1" + block, src, count=1)
    if n == 0:
        sys.stderr.write(f"warn: {path} has no </title> — skipped\n")
        return False
    with open(path, "w", encoding="utf-8") as f:
        f.write(new_src)
    return True


def main():
    static_dir = "/tmp/skillshots-poc/static"
    changed = 0
    for filename, spec in PAGES.items():
        path = os.path.join(static_dir, filename)
        if not os.path.exists(path):
            sys.stderr.write(f"warn: {path} missing\n")
            continue
        if inject(path, spec):
            changed += 1
            print(f"  injected: {filename}")
        else:
            print(f"  skipped (already done): {filename}")
    print(f"changed {changed} files")


if __name__ == "__main__":
    main()

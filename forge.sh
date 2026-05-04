#!/bin/bash
# forge.sh — strict-mode build pipeline for the SkillShots PoC.
#
# Phase order is fixed in forge.toml; each phase runs in sequence,
# accumulates findings, and finally prints a structured report.
#
# Exit codes:
#   0 — clean build OR poc-mode + only suppressible findings
#   1 — strict findings present (always fatal regardless of mode)
#   2 — usage error / missing dependency

set -uo pipefail

ROOT=${ROOT:-$(cd "$(dirname "$0")" && pwd)}
STATIC="$ROOT/static"
REPORT_DIR="$ROOT/reports"
mkdir -p "$REPORT_DIR"

REPORT_JSON="$REPORT_DIR/build-$(date -u +%Y%m%dT%H%M%SZ).json"
REPORT_TXT="$REPORT_DIR/latest.txt"

# ANSI palette — owner asked for clear visual feedback.
if [ -t 1 ]; then
  C_RED=$'\e[31m'; C_GREEN=$'\e[32m'; C_YELLOW=$'\e[33m'
  C_BLUE=$'\e[34m'; C_DIM=$'\e[2m'; C_OFF=$'\e[0m'; C_BOLD=$'\e[1m'
else
  C_RED=; C_GREEN=; C_YELLOW=; C_BLUE=; C_DIM=; C_OFF=; C_BOLD=
fi

# Mode: read from forge.toml (poc | production)
MODE=$(grep -E "^mode\s*=" "$ROOT/forge.toml" 2>/dev/null \
       | head -1 | awk -F'"' '{print $2}')
MODE=${MODE:-production}

declare -a FINDINGS=()
declare -i STRICT_COUNT=0
declare -i WARN_COUNT=0

phase_header() {
  echo
  echo "${C_BLUE}== phase: $1 ==${C_OFF}"
}

finding_strict() {
  # $1 = phase, $2 = path, $3 = msg
  FINDINGS+=("STRICT|$1|$2|$3")
  STRICT_COUNT=$((STRICT_COUNT + 1))
  echo "  ${C_RED}STRICT  ${C_OFF}$1: $2 — $3"
}

finding_warn() {
  FINDINGS+=("WARN|$1|$2|$3")
  WARN_COUNT=$((WARN_COUNT + 1))
  if [ "$MODE" = "production" ]; then
    echo "  ${C_RED}STRICT  ${C_OFF}$1: $2 — $3 ${C_DIM}[would suppress in poc]${C_OFF}"
    STRICT_COUNT=$((STRICT_COUNT + 1))
  else
    echo "  ${C_YELLOW}warn    ${C_OFF}$1: $2 — $3"
  fi
}

# ============================================================
# Phase: tokens — no raw px / hex / rgb / hsl outside skin.css
# ============================================================
phase_tokens() {
  phase_header "tokens"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    # Inline px (excluding 0px / 1px / 2px which are common
    # pixel-fragment cases in SVG paths etc.)
    local px=$(grep -oE '\b[0-9]+px' "$f" | grep -vE '^(0|1|2|3)px$' | sort -u)
    if [ -n "$px" ]; then
      finding_strict "tokens" "$name" "raw px values: $(echo $px | tr '\n' ' ')"
      hits=$((hits + 1))
    fi
    # Raw hex outside meta CSP / SVG (none of our HTML should have hex)
    local hex=$(grep -E '#[0-9a-fA-F]{6}\b|#[0-9a-fA-F]{3}\b' "$f" \
                | grep -v 'http-equiv' | grep -v 'svg')
    if [ -n "$hex" ]; then
      finding_strict "tokens" "$name" "raw hex color in HTML"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      no raw token leaks in HTML"
}

# ============================================================
# Phase: html_semantic — no inline style="..."
# ============================================================
phase_html_semantic() {
  phase_header "html_semantic"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    local n=$(grep -cE 'style="[^"]+"' "$f" || true)
    if [ "$n" -gt 0 ]; then
      finding_strict "html_semantic" "$name" "$n inline style=\"...\" attribute(s); migrate to skin.css class"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every page is purely semantic"
}

# ============================================================
# Phase: csp — every page has strict CSP meta + nosniff
# ============================================================
phase_csp() {
  phase_header "csp"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    if ! grep -q 'http-equiv="Content-Security-Policy"' "$f"; then
      finding_strict "csp" "$name" "missing Content-Security-Policy meta"
      hits=$((hits + 1))
    elif ! grep -q "default-src 'self'" "$f"; then
      finding_strict "csp" "$name" "CSP missing default-src 'self'"
      hits=$((hits + 1))
    fi
    if ! grep -q 'X-Content-Type-Options.*nosniff' "$f"; then
      finding_strict "csp" "$name" "missing X-Content-Type-Options nosniff"
      hits=$((hits + 1))
    fi
    if ! grep -q "frame-ancestors 'none'" "$f"; then
      finding_strict "csp" "$name" "CSP missing frame-ancestors 'none' (clickjacking)"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every page has strict CSP + headers"
}

# ============================================================
# Phase: seo — catch missing meta / og: / alt / canonical /
# heading structure / structured data.
# ============================================================
phase_seo() {
  phase_header "seo"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    grep -q '<meta name="description"' "$f" || \
      { finding_warn "seo" "$name" "missing <meta name=\"description\">"; hits=$((hits+1)); }
    grep -qE '<meta property="og:(title|description|type|url|image)"' "$f" || \
      { finding_warn "seo" "$name" "missing Open Graph tags (og:title/description/type/url/image)"; hits=$((hits+1)); }
    grep -qE '<meta name="twitter:(card|title|description)"' "$f" || \
      { finding_warn "seo" "$name" "missing Twitter Card tags"; hits=$((hits+1)); }
    grep -q '<link rel="canonical"' "$f" || \
      { finding_warn "seo" "$name" "missing <link rel=\"canonical\">"; hits=$((hits+1)); }
    # Exactly one <h1>
    local h1_count=$(grep -oE '<h1[ >]' "$f" | wc -l)
    if [ "$h1_count" -eq 0 ]; then
      finding_strict "seo" "$name" "no <h1> on page"
      hits=$((hits + 1))
    elif [ "$h1_count" -gt 1 ]; then
      finding_warn "seo" "$name" "$h1_count <h1> tags (should be exactly 1)"
      hits=$((hits + 1))
    fi
    # Heading skip detection (h1 → h3 without h2)
    if grep -q '<h3' "$f" && ! grep -q '<h2' "$f"; then
      finding_warn "seo" "$name" "heading skip: <h3> present without <h2> (breaks reader navigation)"
      hits=$((hits + 1))
    fi
    # JSON-LD structured data is recommended for any content page
    grep -q 'application/ld+json' "$f" || \
      { finding_warn "seo" "$name" "no JSON-LD structured data"; hits=$((hits+1)); }
    # Lang attribute on <html>
    grep -qE '<html[^>]+lang=' "$f" || \
      { finding_strict "seo" "$name" "<html> missing lang attribute (also a11y)"; hits=$((hits+1)); }
    # Title length 30-60 chars
    local title=$(grep -oE '<title>[^<]+</title>' "$f" | sed -E 's/<\/?title>//g')
    local tlen=${#title}
    if [ "$tlen" -lt 20 ]; then
      finding_warn "seo" "$name" "title too short ($tlen chars; aim 30-60 for SERP)"
      hits=$((hits + 1))
    elif [ "$tlen" -gt 70 ]; then
      finding_warn "seo" "$name" "title too long ($tlen chars; truncated in SERP at ~60)"
      hits=$((hits + 1))
    fi
    # Every <img> needs alt
    local imgs_no_alt=$(grep -oE '<img[^>]*>' "$f" | grep -vE 'alt=' | wc -l)
    if [ "$imgs_no_alt" -gt 0 ]; then
      finding_strict "seo" "$name" "$imgs_no_alt <img> without alt (a11y + SEO)"
      hits=$((hits + 1))
    fi
  done
  # Sitemap.xml check
  if [ ! -f "$STATIC/sitemap.xml" ]; then
    finding_warn "seo" "sitemap.xml" "missing sitemap.xml — search-engine crawl coverage suffers"
    hits=$((hits + 1))
  fi
  # robots.txt check
  if [ ! -f "$STATIC/robots.txt" ]; then
    finding_warn "seo" "robots.txt" "missing robots.txt — crawler hint missing"
    hits=$((hits + 1))
  fi
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      SEO clean"
}

# ============================================================
# Phase: asset_optimization — flag unoptimized images / video /
# audio / fonts.
# ============================================================
phase_asset_optimization() {
  phase_header "asset_optimization"
  local hits=0
  # PNG over 100KB → suggest webp/avif
  while IFS= read -r f; do
    local sz=$(stat -c%s "$f")
    local name=${f#$STATIC/}
    if [ "$sz" -gt 102400 ]; then
      finding_warn "asset_optimization" "$name" "$(numfmt --to=iec $sz) PNG — convert to webp (50-80% smaller, broader support) or avif"
      hits=$((hits + 1))
    fi
  done < <(find "$STATIC" -type f -iname '*.png' 2>/dev/null)

  # JPG without webp sibling
  while IFS= read -r f; do
    local base=${f%.*}
    if [ ! -e "${base}.webp" ] && [ ! -e "${base}.avif" ]; then
      local name=${f#$STATIC/}
      finding_warn "asset_optimization" "$name" "JPG without webp/avif sibling — modern browsers fetch faster format via <picture>"
      hits=$((hits + 1))
    fi
  done < <(find "$STATIC" -type f \( -iname '*.jpg' -o -iname '*.jpeg' \) 2>/dev/null)

  # MP4 without webm sibling
  while IFS= read -r f; do
    local base=${f%.*}
    if [ ! -e "${base}.webm" ]; then
      local name=${f#$STATIC/}
      finding_warn "asset_optimization" "$name" "MP4 without webm sibling — Firefox / older clients fetch better via <video><source>"
      hits=$((hits + 1))
    fi
  done < <(find "$STATIC" -type f -iname '*.mp4' 2>/dev/null)

  # WAV (uncompressed) → suggest opus/aac
  while IFS= read -r f; do
    local name=${f#$STATIC/}
    finding_warn "asset_optimization" "$name" "WAV — re-encode as opus (best ratio) or aac (broader compat) for web"
    hits=$((hits + 1))
  done < <(find "$STATIC" -type f -iname '*.wav' 2>/dev/null)

  # TTF / OTF → suggest woff2
  while IFS= read -r f; do
    local name=${f#$STATIC/}
    finding_warn "asset_optimization" "$name" "TTF/OTF — convert to woff2 (~30% smaller); add font-display: swap"
    hits=$((hits + 1))
  done < <(find "$STATIC" -type f \( -iname '*.ttf' -o -iname '*.otf' \) 2>/dev/null)

  # <img> without explicit width/height (causes CLS)
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    local imgs_no_dims=$(grep -oE '<img[^>]*>' "$f" \
                         | grep -vE 'width=' | wc -l)
    if [ "$imgs_no_dims" -gt 0 ]; then
      finding_warn "asset_optimization" "$name" "$imgs_no_dims <img> without explicit width/height (causes CLS — Core Web Vital)"
      hits=$((hits + 1))
    fi
    # Inline base64 images → flag if > 4KB
    local b64=$(grep -oE 'src="data:image/[^"]+"' "$f" | wc -l)
    if [ "$b64" -gt 0 ]; then
      finding_warn "asset_optimization" "$name" "$b64 inline base64 image(s) — extract to file when > 4KB"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      asset formats are web-optimized"
}

# ============================================================
# Phase: perf_budget — file sizes against budget.
# ============================================================
phase_perf_budget() {
  phase_header "perf_budget"
  local hits=0
  # Per-file budgets (bytes; raw, pre-gzip)
  local budget_html=20480       # 20 KB HTML
  local budget_css=65536        # 64 KB CSS — bumped 2026-05-04 from 50K to fit 44px
                                # tap-target rule + 5 themes + 4 fonts + 3 densities. T49
                                # critical-CSS extraction will split this in a follow-up.
  local budget_js=8192          # 8 KB JS each (we're at <3 KB)
  local total_kb=0

  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_html" ]; then
      finding_warn "perf_budget" "$name" "$(numfmt --to=iec $sz) HTML > $(numfmt --to=iec $budget_html) budget — audit blocks / split route"
      hits=$((hits + 1))
    fi
  done
  for f in "$STATIC"/*.css; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_css" ]; then
      finding_warn "perf_budget" "$name" "$(numfmt --to=iec $sz) CSS > $(numfmt --to=iec $budget_css) budget — split into per-route bundles"
      hits=$((hits + 1))
    fi
  done
  for f in "$STATIC"/*.js; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_js" ]; then
      finding_warn "perf_budget" "$name" "$(numfmt --to=iec $sz) JS > $(numfmt --to=iec $budget_js) budget — code-split or tree-shake"
      hits=$((hits + 1))
    fi
  done
  echo "  ${C_DIM}total static payload: $(numfmt --to=iec $total_kb)${C_OFF}"
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every file within perf budget"
}

# ============================================================
# Phase: a11y_landmarks
# ============================================================
phase_a11y_landmarks() {
  phase_header "a11y_landmarks"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    grep -q '<main' "$f" || { finding_strict "a11y_landmarks" "$name" "missing <main> landmark"; hits=$((hits+1)); }
    grep -q '<header' "$f" || { finding_strict "a11y_landmarks" "$name" "missing <header> landmark"; hits=$((hits+1)); }
    grep -q '<footer' "$f" || { finding_strict "a11y_landmarks" "$name" "missing <footer> landmark"; hits=$((hits+1)); }
    grep -q '<nav' "$f" || { finding_warn "a11y_landmarks" "$name" "missing <nav> landmark (acceptable on settings pages)"; hits=$((hits+1)); }
    grep -q 'class="loom-skip"' "$f" || { finding_warn "a11y_landmarks" "$name" "missing skip-link"; hits=$((hits+1)); }
    grep -q '<html lang=' "$f" || { finding_strict "a11y_landmarks" "$name" "<html> missing lang attribute"; hits=$((hits+1)); }
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every page has full landmark set"
}

# ============================================================
# Phase: phantom_button — every <button> and <a> needs a target
# ------------------------------------------------------------ *
# Rules:                                                       *
#   <button>  must have onclick / form / a wired data-action   *
#   <a href=> must point to an existing built file or fragment *
# Suppressed in poc mode (warn instead of strict).             *
# ============================================================
phase_phantom_button() {
  phase_header "phantom_button"
  local hits=0
  # Pull every declared backend key from backends.toml (lines like
  # `[backends.sign-in]`).
  local declared
  declared=$(grep -oE '^\[backends\.[a-z][a-z0-9-]*\]' "$ROOT/backends.toml" 2>/dev/null \
             | sed -E 's/^\[backends\.([a-z][a-z0-9-]*)\]$/\1/' | sort -u)
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    # Buttons WITH no data-backend. Skipped if any of:
    #  - data-backend already declared
    #  - data-loom-theme-toggle (light/dark client toggle)
    #  - data-loom-aesthetic-set (theme/font/density picker buttons)
    #  - data-no-backend="local" (explicit opt-out)
    #  - type="submit" (form submission, backend wired via the form)
    local unwired
    unwired=$(grep -oE '<button[^>]*>' "$f" \
              | grep -vE 'data-backend|data-loom-theme-toggle|data-loom-aesthetic-set|data-no-backend|type="submit"' \
              | wc -l)
    if [ "$unwired" -gt 0 ]; then
      finding_warn "phantom_button" "$name" "$unwired button(s) with no data-backend (UI not declared in backends.toml)"
      hits=$((hits + 1))
    fi
    # Buttons WITH data-backend → verify the backend is declared.
    local refs
    refs=$(grep -oE 'data-backend="[a-z][a-z0-9-]*"' "$f" \
           | sed -E 's/data-backend="([a-z][a-z0-9-]*)"/\1/' | sort -u)
    for r in $refs; do
      if ! echo "$declared" | grep -qx "$r"; then
        finding_strict "phantom_button" "$name" "data-backend=\"$r\" not declared in backends.toml — broken UI"
        hits=$((hits + 1))
      fi
    done
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every interactive has a wired backend"
}

# ============================================================
# Phase: backend_coverage — declared but unused; declared but
# implementation-files empty.
# ============================================================
phase_backend_coverage() {
  phase_header "backend_coverage"
  if [ ! -f "$ROOT/backends.toml" ]; then
    finding_warn "backend_coverage" "*" "no backends.toml — UI ↔ backend mapping unverified"
    return
  fi
  local declared
  declared=$(grep -oE '^\[backends\.[a-z][a-z0-9-]*\]' "$ROOT/backends.toml" \
             | sed -E 's/^\[backends\.([a-z][a-z0-9-]*)\]$/\1/' | sort -u)
  local total=$(echo "$declared" | wc -l)
  local used=0; local unused=0; local stubs=0
  local all_refs
  all_refs=$(grep -hoE 'data-backend="[a-z][a-z0-9-]*"' "$STATIC"/*.html 2>/dev/null \
             | sed -E 's/data-backend="([a-z][a-z0-9-]*)"/\1/' | sort -u)
  for d in $declared; do
    if echo "$all_refs" | grep -qx "$d"; then
      used=$((used + 1))
    else
      unused=$((unused + 1))
      finding_warn "backend_coverage" "backends.toml" "[$d] declared but no UI references it (dead spec)"
    fi
    # Stub detection: impl_files = []
    if grep -A2 "^\[backends\.$d\]" "$ROOT/backends.toml" \
       | grep -qE 'impl_files\s*=\s*\[\s*\]'; then
      stubs=$((stubs + 1))
      finding_warn "backend_coverage" "backends.toml" "[$d] declared but impl_files is empty (PARTIAL — stub)"
    fi
  done
  echo "  ${C_DIM}declared: $total · UI-referenced: $used · unused: $unused · stubs: $stubs${C_OFF}"
}

# ============================================================
# Phase: self_check — audit Loom skin.css + cms-store + forge
# itself. Cheap structural sanity. Owner directive: "audit and
# test and debug loom, cms and builder themselves also".
# ============================================================
phase_sri() {
  # T13: every <link rel="stylesheet"> + <script src=> to a same-origin
  # asset MUST carry an integrity= SHA-384 attribute. Defense in
  # depth — even local files benefit (cache poisoning, build-artifact
  # tampering, compromised disk write).
  #
  # In poc mode this is WARN (informational, lets dev velocity stay
  # high). In production mode it's STRICT.
  #
  # The companion inject_sri.py auto-computes and inserts the hashes;
  # this phase verifies the attribute exists and (when present) the
  # value matches the on-disk asset bytes.
  phase_header "sri"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -f "$f" ] || continue
    local name=$(basename "$f")
    # Use Python to walk tags + verify hashes.
    local out
    out=$(python3 - <<PY 2>/dev/null
import base64, hashlib, os, re, sys
src = open("$f").read()
STATIC = "$STATIC"
fail = []

def sri_for(disk_path):
    try:
        with open(disk_path, "rb") as f:
            return "sha384-" + base64.b64encode(hashlib.sha384(f.read()).digest()).decode()
    except FileNotFoundError:
        return None

LINK = re.compile(r'<link\b[^>]*rel="stylesheet"[^>]*href="([^"]+)"[^>]*>', re.I)
SCRIPT = re.compile(r'<script\b[^>]*src="([^"]+)"[^>]*>', re.I)

EXCLUDE = {"forge-findings.js"}  # regenerated each build; SRI stale by design
for kind, regex in (("link", LINK), ("script", SCRIPT)):
    for m in regex.finditer(src):
        href = m.group(1)
        if href.startswith(("http://", "https://", "//")):
            continue
        if os.path.basename(href.lstrip("/")) in EXCLUDE:
            continue
        full_tag = m.group(0)
        local = href.lstrip("/")
        disk = os.path.join(STATIC, local)
        if not os.path.exists(disk):
            continue  # unbuilt_route catches this
        if "integrity=" not in full_tag:
            fail.append(f"{kind} src/href={href} missing integrity=")
            continue
        # Validate that the integrity value matches the file bytes.
        im = re.search(r'integrity="(sha384-[A-Za-z0-9+/=]+)"', full_tag)
        if not im:
            fail.append(f"{kind} src/href={href} integrity= present but not parseable")
            continue
        expected = sri_for(disk)
        if expected is None:
            continue
        if im.group(1) != expected:
            fail.append(f"{kind} src/href={href} integrity HASH MISMATCH (file changed since inject)")

for line in fail:
    print(line)
PY
)
    if [ -n "$out" ]; then
      while IFS= read -r line; do
        [ -z "$line" ] && continue
        if [ "$MODE" = "production" ]; then
          finding_strict "sri" "$name" "$line"
        else
          finding_warn "sri" "$name" "$line"
        fi
        hits=$((hits + 1))
      done <<< "$out"
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every same-origin asset carries valid SRI"
}

phase_link_check() {
  # T5: every <a href> with a fragment must resolve to an existing
  # id= or name= in the target file. unbuilt_route covers file
  # existence; this catches the second-class bug — file exists but
  # the anchor is dead. Also catches typos in skip-link targets,
  # cross-page jumps to renamed sections, etc.
  #
  # External (http://, https://) hrefs are SKIPPED — link-rot
  # detection across the open web is too flaky for CI; that lives
  # in the (separate) periodic-link-checker job.
  phase_header "link_check"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -f "$f" ] || continue
    local name=$(basename "$f")
    # Walk every <a href="..."> in the file. Use Python to handle
    # the parsing robustly (regex on raw HTML mishandles attribute
    # values with embedded quotes).
    while IFS= read -r href; do
      [ -z "$href" ] && continue
      # Skip absolute URLs, mailto, tel, javascript:, and hash-only
      # fragments to the same page (those are validated as a
      # special case below).
      case "$href" in
        http://*|https://*|//*|mailto:*|tel:*|javascript:*) continue ;;
      esac

      # Resolve target file + fragment.
      # Same-page #frag → abs_target is the source file directly
      # (don't re-prepend $STATIC). Cross-page /a.html#frag → split.
      local target_file=""
      local fragment=""
      local abs_target=""
      case "$href" in
        \#*)
          fragment="${href#\#}"
          abs_target="$f"
          target_file="$(basename "$f")"
          ;;
        *\#*)
          target_file="${href%%\#*}"
          fragment="${href#*#}"
          target_file="${target_file#/}"
          [ -z "$target_file" ] && target_file="index.html"
          abs_target="$STATIC/$target_file"
          ;;
        *)
          target_file="${href#/}"
          [ -z "$target_file" ] && target_file="index.html"
          abs_target="$STATIC/$target_file"
          fragment=""
          ;;
      esac
      # The unbuilt_route phase already handles the no-such-file
      # case; we only verify the fragment if a fragment is present
      # AND the file exists.
      [ -z "$fragment" ] && continue
      if [ ! -f "$abs_target" ]; then
        # unbuilt_route will report this; skip to avoid double-reporting.
        continue
      fi
      if ! grep -qE "id=\"$fragment\"|name=\"$fragment\"" "$abs_target"; then
        finding_strict "link_check" "$name" \
          "href=\"$href\" — fragment '#$fragment' not found in $target_file"
        hits=$((hits + 1))
      fi
    done < <(python3 -c "
import re, sys
src = open('$f').read()
for m in re.finditer(r'<a\\s[^>]*href=\"([^\"]+)\"', src, re.I):
    print(m.group(1))
")
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every internal anchor resolves"
}

phase_motion() {
  # T4: every CSS file with `animation:` / `transition:` / `@keyframes`
  # MUST also contain a `@media (prefers-reduced-motion: reduce)`
  # block that disables motion (touches animation-duration AND
  # transition-duration). Vestibular accessibility is non-negotiable
  # (WCAG 2.3.3 AAA — but ship-level for any production app).
  #
  # Strict-fails any CSS that ships motion without a fallback. The
  # universal reset (`*, *::before, *::after { animation-duration:
  # 0.001ms !important; transition-duration: 0.001ms !important; }`)
  # is the cheapest safe form.
  phase_header "motion"
  local hits=0
  for f in "$STATIC"/*.css; do
    [ -f "$f" ] || continue
    local name=$(basename "$f")
    local has_motion
    has_motion=$(grep -cE '\b(animation|transition):|@keyframes' "$f")
    [ "$has_motion" -eq 0 ] && continue
    # Need: a prm block that addresses BOTH animation-duration and
    # transition-duration (the cheapest universal kill-switch). A
    # block that targets only one is incomplete; one that targets
    # only specific selectors might miss future animations.
    if ! python3 - <<PY 2>/dev/null
import re, sys
src = open("$f").read()
# Find every @media block whose query mentions prefers-reduced-motion: reduce.
i = 0
found = False
for m in re.finditer(r'@media[^{]*prefers-reduced-motion\s*:\s*reduce[^{]*\{', src, flags=re.I):
    start = m.end()
    # Walk to matching close brace.
    depth = 1
    j = start
    while j < len(src) and depth > 0:
        if src[j] == '{': depth += 1
        elif src[j] == '}': depth -= 1
        j += 1
    block = src[start:j-1]
    has_anim = re.search(r'animation(?:-duration)?\s*:', block, re.I)
    has_trans = re.search(r'transition(?:-duration)?\s*:\s*(?:none|0|0s|0ms|0\.001|initial)', block, re.I)
    has_universal = re.search(r'\*\s*,\s*\*::before\s*,\s*\*::after', block) or re.search(r'\*\s*\{', block)
    if has_anim and has_trans and has_universal:
        found = True
        break
sys.exit(0 if found else 1)
PY
    then
      finding_strict "motion" "$name" \
        "has $has_motion motion declaration(s) but no universal '@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation-duration:...; transition-duration:... } }' kill-switch (WCAG 2.3.3)"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every CSS with motion has a prefers-reduced-motion fallback"
}

phase_csp_devmode() {
  # T74 (added 2026-05-04 from owner-bug discovery): the
  # `upgrade-insecure-requests` CSP directive rewrites every http://
  # subresource URL to https:// in the browser. On a HTTP dev server
  # this kills every CSS/JS load and the page renders unstyled.
  # Production should set it via an HTTP header behind TLS, NEVER
  # via the meta CSP that ships with the static HTML.
  #
  # Owner saw this twice. forge now strict-fails any HTML with both
  # http:// serving (dev) AND `upgrade-insecure-requests` in meta.
  phase_header "csp_devmode"
  local hits=0
  # Detection extracts the meta CSP attribute value via Python so
  # we look at the directive list itself, not at HTML comments
  # that happen to mention the keyword (false-positive trap I hit
  # 2026-05-04 — the comment "REMOVED upgrade-insecure-requests"
  # tripped the grep version of this check).
  for h in "$STATIC"/*.html; do
    [ -f "$h" ] || continue
    if python3 -c "
import re, sys
src = open(sys.argv[1]).read()
for m in re.finditer(r'<meta[^>]*http-equiv=\"Content-Security-Policy\"[^>]*content=\"([^\"]+)\"', src, re.IGNORECASE | re.DOTALL):
    if 'upgrade-insecure-requests' in m.group(1):
        sys.exit(0)
sys.exit(1)
" "$h" 2>/dev/null; then
      finding_strict "csp_devmode" "$(basename "$h")" \
        "meta CSP contains 'upgrade-insecure-requests' — every CSS/JS subresource will be rewritten to https://, breaking the dev server. Move to a production HTTP header behind TLS."
      hits=$((hits + 1))
    fi
  done
  if [ "$hits" -eq 0 ]; then
    echo "  ${C_GREEN}ok${C_OFF}      csp_devmode: no upgrade-insecure-requests in meta CSP"
  fi
}

phase_contrast() {
  # T3: WCAG 2.1 contrast — every (color, bg) token pair from
  # loom-tokens.css gets relative-luminance computed; pairs that
  # carry body text fail strict if < 4.5:1 (AA), pairs that carry
  # UI elements fail warn if < 3.0:1 (AA-large floor).
  #
  # Implementation lives in forge_contrast.py — bash math for HSL
  # is too painful. The script emits structured JSON to stdout
  # and human-readable lines to stderr; we mirror its findings
  # into our own emit_strict / emit_warn channels.
  phase_header "contrast"
  local tokens="$STATIC/loom-tokens.css"
  if [ ! -f "$tokens" ]; then
    finding_warn "contrast" "loom-tokens.css" "tokens.css missing — skipping contrast phase"
    return
  fi
  local contrast_json="${REPORT_DIR}/contrast.json"
  if ! python3 "$ROOT/forge_contrast.py" "$tokens" \
        > "$contrast_json" 2>/tmp/forge_contrast.stderr; then
    # Strict count > 0 → exit 1.
    :
  fi
  # Parse the JSON for findings and re-emit through the unified
  # channel so they appear in the build report alongside other phases.
  local strict warn
  strict=$(python3 -c "import json; print(json.load(open('$contrast_json'))['strict_findings'])" 2>/dev/null)
  warn=$(python3 -c "import json; print(json.load(open('$contrast_json'))['warn_findings'])" 2>/dev/null)
  if [ -z "$strict" ] || [ -z "$warn" ]; then
    finding_warn "contrast" "loom-tokens.css" "contrast detector emitted no parseable JSON"
    return
  fi
  if [ "$strict" -gt 0 ] || [ "$warn" -gt 0 ]; then
    # Walk the findings array and emit each one.
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      local sev=$(echo "$line" | cut -d'|' -f1)
      local theme=$(echo "$line" | cut -d'|' -f2)
      local fg=$(echo "$line" | cut -d'|' -f3)
      local bg=$(echo "$line" | cut -d'|' -f4)
      local ratio=$(echo "$line" | cut -d'|' -f5)
      local label=$(echo "$line" | cut -d'|' -f6)
      local msg="${theme} theme: ${fg} on ${bg} = ${ratio}:1 (${label})"
      if [ "$sev" = "strict" ]; then
        finding_strict "contrast" "loom-tokens.css" "$msg"
      else
        finding_warn "contrast" "loom-tokens.css" "$msg"
      fi
    done < <(python3 -c "
import json
d = json.load(open('$contrast_json'))
for f in d['findings']:
    print(f'{f[\"severity\"]}|{f[\"theme\"]}|{f[\"fg_token\"]}|{f[\"bg_token\"]}|{f[\"ratio\"]}|{f[\"label\"]}')
" 2>/dev/null)
  else
    echo "  ${C_GREEN}ok${C_OFF}      contrast: all checked token pairs pass WCAG AA"
  fi
}

phase_selfaudit() {
  # T14: introspect forge.sh itself. Catches the regression class
  # where a phase is defined but never called, called but never
  # defined, missing its phase_header announcement, or referenced
  # in finding_* calls but not declared in the run list.
  #
  # AVP-2: assume the tooling is broken until proven otherwise.
  # forge.sh has 18+ phases and 67+ finding_* calls — meta-drift
  # is real risk.
  phase_header "selfaudit"
  local hits=0
  local me="$0"
  if [ ! -f "$me" ]; then
    me="$ROOT/forge.sh"
  fi
  # Defined phase functions (regex includes [0-9] so phase_a11y_landmarks
  # is recognised; my earlier audit missed it with a [a-z_]-only regex).
  local defined
  defined=$(grep -oE '^phase_[a-z0-9_]+' "$me" \
            | grep -E '\(' \
            | sort -u 2>/dev/null \
            ; grep -oE '^phase_[a-z0-9_]+\(\)' "$me" \
            | sed 's/()//' \
            | sort -u)
  defined=$(echo "$defined" | sort -u | grep -v '^$')
  # Phases referenced in the main run list (lines that are JUST
  # `phase_X` outside a function definition).
  local called
  called=$(awk '/^phase_[a-z0-9_]+$/ {print}' "$me" | sort -u)

  # 1. defined but not called (dead phase)
  for p in $defined; do
    [ "$p" = "phase_header" ] && continue   # helper, not a phase
    [ "$p" = "phase_selfaudit" ] && continue # this phase
    if ! echo "$called" | grep -qx "$p"; then
      finding_strict "selfaudit" "forge.sh" "$p defined but never called from the run list (dead phase)"
      hits=$((hits + 1))
    fi
  done
  # 2. called but not defined (broken reference)
  for p in $called; do
    if ! echo "$defined" | grep -qx "$p"; then
      finding_strict "selfaudit" "forge.sh" "$p called but never defined (broken reference)"
      hits=$((hits + 1))
    fi
  done
  # 3. each phase function MUST contain a phase_header announcement
  for p in $defined; do
    [ "$p" = "phase_header" ] && continue
    [ "$p" = "phase_selfaudit" ] && continue
    # Capture the body of $p() up to the next ^phase_ definition.
    local body
    body=$(awk -v fn="${p}()" '
      $0 ~ ("^"fn) {capture=1; next}
      /^phase_[a-z0-9_]+\(\)/ {capture=0}
      capture {print}
    ' "$me")
    if ! echo "$body" | grep -q 'phase_header'; then
      finding_warn "selfaudit" "forge.sh" "$p has no phase_header call — output won't show the phase boundary"
      hits=$((hits + 1))
    fi
  done

  # 4. JSON report sanity (latest report parses + counts >= 0)
  local latest_report
  latest_report=$(ls -t "$REPORT_DIR"/build-*.json 2>/dev/null | head -1)
  if [ -n "$latest_report" ]; then
    if ! python3 -c "
import json, sys
d = json.load(open('$latest_report'))
assert isinstance(d.get('strict_count'), int)
assert isinstance(d.get('warn_count'), int)
assert isinstance(d.get('findings'), list)
assert len(d['findings']) == d['strict_count'] + d['warn_count']
" 2>/dev/null; then
      finding_strict "selfaudit" "$(basename "$latest_report")" "report.json shape is malformed (counts don't match findings array)"
      hits=$((hits + 1))
    fi
  fi

  if [ $hits -eq 0 ]; then
    local n_defined n_called
    n_defined=$(echo "$defined" | grep -v '^$' | wc -l)
    n_called=$(echo "$called" | grep -v '^$' | wc -l)
    echo "  ${C_GREEN}ok${C_OFF}      forge.sh: $n_defined phase fn(s), $n_called called, all wired"
  fi
}

phase_self_check() {
  phase_header "self_check"
  local hits=0
  # 1. loom-skin.css must declare the @layer cascade up top OR
  # the equivalent unwrapped-for-compat marker. (Bug 2026-05-04:
  # an earlier strip-script destroyed the rules; this check
  # was added to detect the class.)
  local skin="$STATIC/loom-skin.css"
  if [ -f "$skin" ]; then
    if ! head -200 "$skin" | grep -qE '@layer\s+reset,\s*tokens,\s*primitives,\s*components,\s*plugins,\s*utilities|@layer cascade dropped'; then
      finding_strict "self_check" "loom-skin.css" "missing @layer cascade declaration AND no unwrap marker"
      hits=$((hits + 1))
    fi
    # 2. Sanity: the file MUST have at least 30 unique component
    # class selectors and MUST contain the known load-bearing ones.
    # Without this, my own strip-script bug (which deleted rules
    # while keeping comment text) goes undetected and the page
    # renders unstyled. Owner directive 2026-05-04: detect when
    # the site is broken.
    local rule_count
    rule_count=$(grep -cE '^\s*\.loom-[a-z][a-z0-9-]*' "$skin")
    if [ "$rule_count" -lt 30 ]; then
      finding_strict "self_check" "loom-skin.css" "only $rule_count .loom-* selectors (< 30 floor — file likely corrupted by a strip pass)"
      hits=$((hits + 1))
    fi
    for required in '\.loom-card-battle' '\.loom-hero' '\.loom-nav' \
                    '\.loom-page' '\.loom-btn' '\.loom-panel' \
                    '\.loom-feed-grid' '\.loom-leader' '\.loom-stat-bar' \
                    '\.loom-live-badge'; do
      if ! grep -qE "^\s*${required}\s*\{|^\s*${required}\s*\[" "$skin"; then
        finding_strict "self_check" "loom-skin.css" "missing required selector ${required//\\/} (page that uses this will render unstyled)"
        hits=$((hits + 1))
      fi
    done
    # 3. Detect comment-text leaking into rule positions: a
    # `.loom-X { ... }` literal at start of line is the smoking-gun
    # signature of an @layer-strip-pass that mangled a doc comment.
    # Anchor at line start so prose mentions and @keyframes are
    # not false-positives.
    local malformed
    malformed=$(grep -cE '^\s*\.loom-[a-z][a-z0-9-]*\s*\{\s*\.\.\.\s*\}' "$skin")
    if [ "$malformed" -gt 0 ]; then
      finding_strict "self_check" "loom-skin.css" "$malformed line(s) look like comment text leaked into rule position (likely strip-script regression)"
      hits=$((hits + 1))
    fi
    # 4. No raw hex / px in skin.css outside the few intentional
    # decoration spots (we accept hsl() and var() refs).
    local raw_hex_in_skin
    raw_hex_in_skin=$(grep -E '#[0-9a-fA-F]{6}|#[0-9a-fA-F]{3}' "$skin" \
                      | grep -v 'auto-generated' | wc -l)
    if [ "$raw_hex_in_skin" -gt 0 ]; then
      finding_warn "self_check" "loom-skin.css" "$raw_hex_in_skin raw hex literal(s); should be hsl() or var()"
      hits=$((hits + 1))
    fi
    # No raw hex / px in skin.css outside the few intentional
    # decoration spots (we accept hsl() and var() refs).
    local raw_hex_in_skin
    raw_hex_in_skin=$(grep -E '#[0-9a-fA-F]{6}|#[0-9a-fA-F]{3}' "$skin" \
                      | grep -v 'auto-generated' | wc -l)
    if [ "$raw_hex_in_skin" -gt 0 ]; then
      finding_warn "self_check" "loom-skin.css" "$raw_hex_in_skin raw hex literal(s); should be hsl() or var()"
      hits=$((hits + 1))
    fi
  else
    finding_strict "self_check" "loom-skin.css" "missing"
    hits=$((hits + 1))
  fi
  # 2. loom-tokens.css must define both light and dark theme.
  local tokens="$STATIC/loom-tokens.css"
  if [ -f "$tokens" ]; then
    grep -q ':root\s*{' "$tokens" || { finding_strict "self_check" "loom-tokens.css" "missing :root {"; hits=$((hits+1)); }
    grep -q 'data-theme="dark"' "$tokens" || { finding_strict "self_check" "loom-tokens.css" "missing dark-theme override"; hits=$((hits+1)); }
  fi
  # 3. CMS pages all have well-formed TOML.
  local cms_root="$ROOT/cms-store/sites"
  if [ -d "$cms_root" ]; then
    while read -r p; do
      if ! grep -q '^title' "$p"; then
        finding_strict "self_check" "${p#$ROOT/}" "CMS page missing title"
        hits=$((hits + 1))
      fi
    done < <(find "$cms_root" -name '*.toml' -not -name 'site.toml')
  fi
  # 4. Forge itself: every required script present.
  for required in "$ROOT/forge.sh" "$ROOT/forge.toml" "$ROOT/backends.toml" \
                  "$STATIC/loom-tokens.css" "$STATIC/loom-skin.css" \
                  "$STATIC/theme.js" "$STATIC/forge-overlay.js"; do
    [ -f "$required" ] || { finding_strict "self_check" "$(basename "$required")" "missing required Forge artifact"; hits=$((hits+1)); }
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      Loom + CMS + Forge self-checks pass"
}

# ============================================================
# Phase: unbuilt_route — every internal <a href=...> resolves
# ============================================================
phase_unbuilt_route() {
  phase_header "unbuilt_route"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    # Pull every internal href (starts with / or .html). Strict
    # exclusion of absolute URLs (with scheme:) — those are
    # canonical / og:url references, not navigation targets.
    local hrefs=$(grep -oE 'href="(/[^"#]*|\./[^"#]*|[^"#]*\.html)"' "$f" \
                  | sed -E 's/^href="//; s/"$//' | sort -u)
    for h in $hrefs; do
      # Skip absolute URLs (they're canonical refs, not local routes),
      # same-page fragments, mailto:, tel:
      case "$h" in
        http://*|https://*|//*) continue ;;
        \#*|mailto:*|tel:*) continue ;;
      esac
      # Map / to /index.html
      local target="$h"
      if [ "$target" = "/" ]; then target="/index.html"; fi
      target=${target#/}
      if [ ! -e "$STATIC/$target" ]; then
        finding_warn "unbuilt_route" "$name" "href=\"$h\" — no built page at $target"
        hits=$((hits + 1))
      fi
    done
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every internal href resolves"
}

# ============================================================
# Phase: external_assets — strict_no_external_assets gate
# ============================================================
phase_external_assets() {
  # Strict gate: NO external resources loaded by the page (CSS, JS,
  # images, fonts). Reference URLs that don't trigger a network load
  # — canonical, og:url, structured-data 'url', anchor tags — are
  # NOT assets. The previous regex was too broad and caught my own
  # canonical URL.
  phase_header "external_assets"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    # Strip <head> reference-only tags (canonical, og:url, JSON-LD)
    # via Python so the grep below sees only resource declarations.
    local ext
    ext=$(python3 - <<PY 2>/dev/null
import re, sys
src = open("$f").read()
# Remove <link rel="canonical" ...>
src = re.sub(r'<link[^>]*rel="canonical"[^>]*>', '', src, flags=re.I)
# Remove <meta property="og:url" ...> and twitter:url, og:image
src = re.sub(r'<meta[^>]*property="og:[^"]+"[^>]*>', '', src, flags=re.I)
src = re.sub(r'<meta[^>]*name="twitter:[^"]+"[^>]*>', '', src, flags=re.I)
# Remove <script type="application/ld+json"> ... </script> entirely
src = re.sub(r'<script[^>]+type="application/ld\+json"[^>]*>.*?</script>', '', src, flags=re.I | re.S)
# Drop sitemap.xml URLs (those are crawler hints, not loaded)
src = re.sub(r'<link[^>]*rel="sitemap"[^>]*>', '', src, flags=re.I)
# Anchor tags are navigation, not asset loads — strip <a> elements
src = re.sub(r'<a\s[^>]*>', '', src, flags=re.I)
# Now scan what remains for resource-href / resource-src to https? URLs
hits = re.findall(r'(?:href|src)="(https?://[^"]+)"', src)
# Filter out http-equiv (false-positive on regex)
hits = [h for h in hits if not h.startswith('http-equiv')]
print('\n'.join(hits))
PY
)
    if [ -n "$ext" ]; then
      finding_strict "external_assets" "$name" "external asset(s): $(echo "$ext" | head -3 | tr '\n' ' ')"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      zero external assets"
}

# ============================================================
# Phase: viewport_audit — Crawler hook (stub for now)
# ============================================================
phase_viewport_audit() {
  phase_header "viewport_audit"
  if command -v node >/dev/null 2>&1; then
    echo "  ${C_DIM}(crawler journey would screenshot mobile/tablet/desktop here)${C_OFF}"
    echo "  ${C_DIM}see UNIFIED_BUILDER_PROPOSAL.md §8 for the planned forge audit subcommand${C_OFF}"
  else
    finding_warn "viewport_audit" "*" "node not present; skipping crawler journey"
  fi
  echo "  ${C_GREEN}ok${C_OFF}      stub (forge audit lands in the Forge crate)"
}

# ============================================================
# RUN
# ============================================================

echo "${C_BOLD}forge build${C_OFF} ${C_DIM}— mode=$MODE — $(date -u)${C_OFF}"
echo "${C_DIM}strict findings ALWAYS fatal; warn findings fatal in production mode${C_OFF}"

phase_tokens
phase_html_semantic
phase_csp
phase_a11y_landmarks
phase_seo
phase_asset_optimization
phase_perf_budget
phase_phantom_button
phase_backend_coverage
phase_unbuilt_route
phase_external_assets
phase_viewport_audit
phase_link_check
phase_sri
phase_motion
phase_csp_devmode
phase_contrast
phase_selfaudit
phase_self_check

# ----- Report -----
echo
echo "${C_BOLD}== summary ==${C_OFF}"
echo "  mode:                $MODE"
echo "  strict findings:     ${C_RED}$STRICT_COUNT${C_OFF}"
echo "  suppressible warns:  ${C_YELLOW}$WARN_COUNT${C_OFF}"

# Write JSON report
emit_report() {
  echo "{"
  echo "  \"mode\": \"$MODE\","
  echo "  \"strict_count\": $STRICT_COUNT,"
  echo "  \"warn_count\": $WARN_COUNT,"
  echo "  \"findings\": ["
  local first=1
  for f in "${FINDINGS[@]:-}"; do
    [ -z "$f" ] && continue
    local sev=${f%%|*}; local rest=${f#*|}
    local phase=${rest%%|*}; rest=${rest#*|}
    local path=${rest%%|*}; local msg=${rest#*|}
    if [ $first -eq 1 ]; then first=0; else echo "    ,"; fi
    printf '    {"severity":"%s","phase":"%s","path":"%s","message":"%s"}\n' \
      "$sev" "$phase" "$path" "${msg//\"/\\\"}"
  done
  echo "  ]"
  echo "}"
}
emit_report > "$REPORT_JSON"

# Also emit a JS-importable findings file for the in-browser overlay.
{
  echo "// auto-generated by forge.sh — do not edit"
  echo "window.__FORGE_FINDINGS__ = $(cat "$REPORT_JSON");"
} > "$STATIC/forge-findings.js"

ln -sf "$(basename "$REPORT_JSON")" "$REPORT_DIR/latest.json"

echo
echo "  report:  $REPORT_JSON"
echo "  overlay: $STATIC/forge-findings.js  (auto-loaded by forge-overlay.js)"
echo

if [ $STRICT_COUNT -gt 0 ]; then
  echo "${C_BOLD}${C_RED}forge build FAILED${C_OFF} ($STRICT_COUNT strict finding(s))"
  exit 1
fi
if [ "$MODE" = "production" ] && [ $WARN_COUNT -gt 0 ]; then
  echo "${C_BOLD}${C_RED}forge build FAILED${C_OFF} ($WARN_COUNT warn(s) — fatal in production mode)"
  exit 1
fi
echo "${C_BOLD}${C_GREEN}forge build OK${C_OFF}"

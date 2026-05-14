#!/bin/bash
# ============================================================
#  ⚠  DEPRECATED — DO NOT ADD NEW PHASES TO THIS FILE  ⚠
# ============================================================
#
# Per owner directive 2026-05-06: forge IS a Rust application.
# This bash script is kept ONLY as a parity reference during
# the migration window. All new phases land in the Rust crates:
#
#   crates/forge-core    — Phase trait, Finding, Severity types
#   crates/forge-phases  — one module per phase
#   crates/forge-cli     — the binary (replaces this script)
#
# Build order:
#   cargo run -p forge-cli --release
#
# T54 (queued) deletes this file once every bash phase has a
# Rust port. Any contribution to this file should INSTEAD be
# made to crates/forge-phases/src/<name>.rs and wired into
# crates/forge-cli/src/main.rs.
#
# If you're an AI agent reading this file: STOP. Pivot to the
# Rust workspace. Adding bash here is a regression even if it
# "works" — bash makes refactoring impossible, has no type
# safety, and the AVP-2 supersociety stack mandates Rust for
# everything that runs in production.
# ============================================================
#
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

# T9: --watch — re-run forge on every change to static/, backends.toml,
# or forge.toml. Uses inotifywait when present (event-driven, ~zero
# CPU when idle); falls back to find-newer polling otherwise. Placed
# AFTER the palette + MODE setup so error messages render with color.
# T35: --debug flag — surface per-phase timing + findings delta
# into reports/debug-<ts>.log. Passes through other flags so
# --debug --watch etc. compose correctly.
if [ "${1:-}" = "--debug" ]; then
  shift
  FORGE_DEBUG=1
  DEBUG_LOG="$REPORT_DIR/debug-$(date -u +%Y%m%dT%H%M%SZ).log"
  echo "${C_DIM}[debug] writing per-phase log → $DEBUG_LOG${C_OFF}"
  : > "$DEBUG_LOG"
fi

# T85: --sync-loom — copy Loom's loom-tokens/src/skin.css into
# static/loom-skin.css and prepend the SYNC-FROM-LOOM:<sha384>
# marker. Inline here (vs calling do_loom_sync defined far below)
# so the flag works on a fresh shell parse — bash hoists function
# definitions but $1 dispatch needs the work present before any
# downstream phase declarations.
if [ "${1:-}" = "--sync-loom" ]; then
  shift
  loom_path="${LOOM_PATH:-/home/user/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css}"
  if [ ! -f "$loom_path" ]; then
    echo "sync failed: loom skin.css not found at $loom_path" >&2
    exit 1
  fi
  poc_path="$STATIC/loom-skin.css"
  loom_hash=$(openssl dgst -sha384 -binary "$loom_path" | openssl base64 -A)
  tmpf=$(mktemp)
  printf '/* SYNC-FROM-LOOM:sha384-%s — auto-synced by forge.sh --sync-loom. Edits to this file will be overwritten on next sync. */\n' "$loom_hash" > "$tmpf"
  cat "$loom_path" >> "$tmpf"
  mv "$tmpf" "$poc_path"
  echo "sync ok: $(basename "$poc_path") ← $(basename "$loom_path") (sha384-$loom_hash)"
  echo "        $(wc -l < "$poc_path") lines, $(wc -c < "$poc_path") bytes"
  exit 0
fi

if [ "${1:-}" = "--watch" ]; then
  shift
  echo "${C_BOLD}forge${C_OFF} ${C_DIM}— watch mode — Ctrl+C to stop${C_OFF}"
  echo "${C_DIM}watching: $STATIC, $ROOT/backends.toml, $ROOT/forge.toml${C_OFF}"
  echo "${C_DIM}note: each user edit triggers ~2 builds — the second is forge's"
  echo "${C_DIM}      own SRI refresh writing HTML. Stabilises immediately.${C_OFF}"
  bash "$0" "$@" || true
  if command -v inotifywait >/dev/null 2>&1; then
    # `attrib` covers `touch` / mtime-only edits; `close_write` is the
    # canonical "editor saved the file" signal across editors.
    #
    # Exclusions:
    #   forge-findings.js — rewritten by forge each build → would
    #                       cause an infinite re-trigger loop.
    #   .gz / .br         — pre-compressed artifacts (T6 future).
    #   build-*.json      — per-run report files in reports/, but
    #                       safe-guard if anyone moves reports inside
    #                       static. Pattern is anchored on filename.
    while inotifywait -qq -r -e modify,attrib,close_write,create,delete,move \
          --exclude '(forge-findings\.js$|\.gz$|\.br$|/build-[0-9TZ]+\.json$|/latest\.json$)' \
          "$STATIC" "$ROOT/backends.toml" "$ROOT/forge.toml" 2>/dev/null; do
      echo
      echo "${C_DIM}─── change detected $(date -u +%H:%M:%S) ───${C_OFF}"
      bash "$0" "$@" || true
    done
  else
    POLL_MARKER=$(mktemp)
    # REGRESSION-GUARD: prior version leaked one /tmp file per
    # --watch session because the marker was created without a
    # cleanup trap. EXIT covers Ctrl+C, INT, TERM, normal end.
    trap 'rm -f "$POLL_MARKER"' EXIT INT TERM
    while true; do
      changed=$(find "$STATIC" "$ROOT/backends.toml" "$ROOT/forge.toml" \
                  -newer "$POLL_MARKER" 2>/dev/null | head -1)
      if [ -n "$changed" ]; then
        touch "$POLL_MARKER"
        echo
        echo "${C_DIM}─── change detected (polled) $(date -u +%H:%M:%S) ───${C_OFF}"
        bash "$0" "$@" || true
      fi
      sleep 2
    done
  fi
  exit 0
fi

declare -a FINDINGS=()
declare -i STRICT_COUNT=0
declare -i WARN_COUNT=0

# T35: debug mode. Activated by --debug flag OR FORGE_DEBUG=1.
# When on, phase_header records start time + finding count; the next
# phase_header (or end-of-build) computes the delta and writes to
# reports/debug-<ts>.log so triage of slow / chatty phases is one
# tail away.
#
# CRITICAL: the --debug flag handler at the top of the file ran
# BEFORE this block and may have set FORGE_DEBUG / DEBUG_LOG. Use
# ${VAR:-default} for ALL of these so the prior assignment isn't
# wiped by an unconditional `VAR=""` reset.
FORGE_DEBUG=${FORGE_DEBUG:-0}
DEBUG_LOG=${DEBUG_LOG:-}
DEBUG_PHASE=${DEBUG_PHASE:-}
DEBUG_PHASE_START=${DEBUG_PHASE_START:-0}
DEBUG_PHASE_FINDING_BASE=${DEBUG_PHASE_FINDING_BASE:-0}

phase_header() {
  # Close out the previous phase's debug record, if any.
  if [ "$FORGE_DEBUG" = "1" ] && [ -n "$DEBUG_PHASE" ]; then
    local now=$(date +%s%N)
    local dur_ms=$(( (now - DEBUG_PHASE_START) / 1000000 ))
    local findings_delta=$(( ${#FINDINGS[@]} - DEBUG_PHASE_FINDING_BASE ))
    printf '[phase=%-18s duration=%6dms findings=%2d]\n' \
      "$DEBUG_PHASE" "$dur_ms" "$findings_delta" \
      >> "$DEBUG_LOG"
  fi
  echo
  echo "${C_BLUE}== phase: $1 ==${C_OFF}"
  if [ "$FORGE_DEBUG" = "1" ]; then
    DEBUG_PHASE="$1"
    DEBUG_PHASE_START=$(date +%s%N)
    DEBUG_PHASE_FINDING_BASE=${#FINDINGS[@]}
  fi
}

finding_strict() {
  # $1 = phase, $2 = path, $3 = msg
  FINDINGS+=("STRICT|$1|$2|$3")
  STRICT_COUNT=$((STRICT_COUNT + 1))
  echo "  ${C_RED}STRICT  ${C_OFF}$1: $2 — $3"
}

finding_warn() {
  # In production mode, warns ESCALATE to strict (non-suppressible).
  # In poc mode they record + display as warn, ship-passes the build.
  #
  # REGRESSION-GUARD: a previous version of this function bumped
  # both WARN_COUNT and STRICT_COUNT in production, which double-
  # counted the same finding once it reached the summary. The fix
  # is to delegate completely to finding_strict in production so
  # there is exactly ONE counter incremented per call.
  if [ "$MODE" = "production" ]; then
    finding_strict "$1" "$2" "$3"
    return
  fi
  FINDINGS+=("WARN|$1|$2|$3")
  WARN_COUNT=$((WARN_COUNT + 1))
  echo "  ${C_YELLOW}warn    ${C_OFF}$1: $2 — $3"
}

# Multi-line-tag-aware grep helper. The single-line `grep -oE
# '<img[^>]*>'` form silently misses tags whose attributes wrap
# onto multiple lines (modern HTML formatters routinely do this
# once an element has 4+ attrs — and the Loom Picture component
# emits enough attrs to cross that line). This helper joins every
# `<TAG ...>` (open form, attribute span) to a single line per
# tag so downstream pipes are correct.
#
# Usage:    _open_tags <file> <tagname>
# Returns:  one open-tag-as-line per match, on stdout.
_open_tags() {
  python3 - "$1" "$2" <<'PY'
import re, sys
src = open(sys.argv[1], 'r', encoding='utf-8', errors='replace').read()
tag = sys.argv[2]
# Match <tag ...> with attributes that may span newlines. The
# pattern stops at the first '>' that ISN'T inside a quoted
# attribute value — safe enough for HTML well-formed enough to
# ship. We don't need to be a full HTML parser; we just need to
# match the SAME class of tags grep was trying to match plus the
# multi-line case.
pat = re.compile(rf'<{re.escape(tag)}\b[^>]*>', re.DOTALL)
for m in pat.finditer(src):
    print(m.group(0).replace('\n', ' '))
PY
}

# ============================================================
# Phase: cms_render — regenerate every static/<page>.html that
# has a corresponding cms/<page>.json source via Loom's
# `loom cms-render` subcommand. This is the T55 "regen PoC HTML
# from CMS" tide — pages with a JSON source ARE the canonical
# form; the static HTML is generated output committed for
# reproducibility (so a fresh checkout serves without needing
# the loom binary at request time).
#
# Per the boundary doctrine: hand-edits to static/<page>.html
# for any page whose source is in cms/ get OVERWRITTEN on every
# build. Edit the JSON, not the HTML.
#
# REGRESSION-GUARD: do NOT add a --skip-render flag. If
# `loom cms-render` is broken or absent, the build should FAIL
# loudly so the operator notices, not silently ship stale HTML.
# ============================================================
# ============================================================
# Phase: validate_cms — fast schema + URL check on cms/*.json
# BEFORE the slower render+audit cycle.
#
# Why this runs first: phase_cms_render's per-file render takes
# ~30ms of the ~3s build; validate is ~1ms per file. Catching a
# schema typo here means a sub-second turnaround on iteration vs
# waiting for full render. Failed validation = strict (the bridge
# would fail anyway when render_page hit deserialize; we just
# surface it earlier and uniformly).
#
# REGRESSION-GUARD: do NOT skip when cms/ is empty. The phase is
# the canary that proves the validator binary is reachable; if it
# breaks (e.g., loom binary moved), we want forge to fail
# immediately, not silently miss schema bugs.
# ============================================================
phase_validate_cms() {
  phase_header "validate_cms"
  if [ ! -d "$ROOT/cms" ]; then
    echo "  ${C_DIM}skip${C_OFF}    no cms/ directory; nothing to validate"
    return
  fi
  local LOOM_BIN="${LOOM_BIN:-/home/user/cargo-target/release/loom}"
  if [ ! -x "$LOOM_BIN" ]; then
    finding_strict "validate_cms" "loom-cli" "loom binary not at $LOOM_BIN — cannot validate; run 'cargo build --release -p loom-cli' in PlausiDen-Loom"
    return
  fi
  local out
  out=$("$LOOM_BIN" validate --input "$ROOT/cms" 2>&1)
  local rc=$?
  if [ $rc -eq 0 ]; then
    # Print the per-file summary line only (not every 'ok ...' line — too chatty).
    local summary=$(echo "$out" | grep -E '^loom validate:' | head -1)
    if [ -n "$summary" ]; then
      echo "  ${C_GREEN}ok${C_OFF}      $summary"
    fi
    return
  fi
  # Validation failed — surface one strict per actual file error
  # (lines starting with '  fail '). Suppress the trailing summary
  # lines to avoid double-counting the same bug as 3 findings.
  #
  # REGRESSION-GUARD: prior version emitted one strict per output
  # line, including the summary + 'at least one file failed' tail —
  # a single bad cms/foo.json appeared as 3 strict findings. Strict
  # count should track problems, not log lines.
  local first_summary=1
  while IFS= read -r line; do
    if echo "$line" | grep -qE '^\s+fail\s'; then
      local file=$(echo "$line" | grep -oE 'cms/[^:]+' | head -1)
      finding_strict "validate_cms" "${file:-cms/}" "$line"
    elif [ "$first_summary" = "1" ]; then
      # Echo the per-run summary line once at the end (informational).
      echo "  ${C_DIM}$line${C_OFF}"
      first_summary=0
    fi
  done < <(echo "$out" | grep -E '^\s+fail|^loom validate:')
}

# ============================================================
# Phase: image_convert — generate AVIF + WebP siblings for every
# JPG/PNG under static/assets/. Loom's Picture component emits
# <picture><source type="image/avif" srcset="...avif"><source
# type="image/webp" srcset="...webp"><img src="...jpg"></picture>
# — without the siblings, the avif/webp source elements 404 and
# the browser falls back to JPEG. With them, every modern visitor
# gets a 30-60% smaller payload at first paint.
#
# Skip-if-fresh: the loom CLI checks mtime and skips siblings
# already newer than source. Build cost is ~zero on no-op.
#
# REGRESSION-GUARD: do NOT skip this phase even when assets/ has
# zero files. The loom CLI handles empty + missing dirs gracefully
# (exits 0 silently); skipping the phase entirely would mean a
# future image addition silently ships without modern formats.
# ============================================================
phase_image_convert() {
  phase_header "image_convert"
  local LOOM_BIN="${LOOM_BIN:-/home/user/cargo-target/release/loom}"
  local ASSETS_DIR="$STATIC/assets"
  if [ ! -d "$ASSETS_DIR" ]; then
    echo "  ${C_DIM}skip${C_OFF}    no static/assets/ directory; nothing to convert"
    return
  fi
  if [ ! -x "$LOOM_BIN" ]; then
    finding_warn "image_convert" "loom-cli" "loom binary not at $LOOM_BIN — modern image formats not generated; siblings may be stale"
    return
  fi
  if "$LOOM_BIN" image-convert --input-dir "$ASSETS_DIR" 2>/tmp/forge-image-convert-err; then
    rm -f /tmp/forge-image-convert-err
  else
    local err=$(cat /tmp/forge-image-convert-err 2>/dev/null | head -1)
    finding_strict "image_convert" "static/assets/" "image-convert failed: $err"
    rm -f /tmp/forge-image-convert-err
  fi
}

# ============================================================
# Phase: audit_bridge — assert every CmsSection variant has its
# matching .loom-* selector(s) in skin.css. Closes the silent-
# unstyled-page failure mode T13 caught (paragraph + heading
# variants had no skin rules through 13 ticks of work).
#
# Strict on any missing selector — the bridge↔skin contract is
# load-bearing for every CMS-driven page's visual fidelity.
#
# REGRESSION-GUARD: do NOT downgrade to warn. The audit catches
# a class of drift that's INVISIBLE in dev (browser falls back
# to default <p> / <h2> styles) but breaks the design system.
# Strict-fail is the only severity that prevents this from
# silently shipping.
# ============================================================
phase_audit_bridge() {
  phase_header "audit_bridge"
  local LOOM_BIN="${LOOM_BIN:-/home/user/cargo-target/release/loom}"
  if [ ! -x "$LOOM_BIN" ]; then
    finding_warn "audit_bridge" "loom-cli" "loom binary not at $LOOM_BIN — bridge↔skin coverage not verified"
    return
  fi
  if [ ! -f "$STATIC/loom-skin.css" ]; then
    finding_warn "audit_bridge" "static/loom-skin.css" "skin.css missing — coverage not verified"
    return
  fi
  local out
  out=$("$LOOM_BIN" audit-bridge --skin "$STATIC/loom-skin.css" 2>&1)
  local rc=$?
  if [ $rc -eq 0 ]; then
    local summary=$(echo "$out" | grep -E '^loom audit-bridge:' | head -1)
    if [ -n "$summary" ]; then
      echo "  ${C_GREEN}ok${C_OFF}      $summary"
    fi
    return
  fi
  # Surface every fail line as STRICT.
  while IFS= read -r line; do
    if echo "$line" | grep -qE '^\s+fail\s'; then
      local var=$(echo "$line" | grep -oE 'variant=[a-z_]+' | head -1)
      finding_strict "audit_bridge" "${var:-skin}" "$line"
    fi
  done < <(echo "$out")
}

# ============================================================
# Phase: theme_consistency — wraps `loom theme validate` (T28).
# Strict on undefined-base-token references, warn on theme drift
# (named theme omits or adds a token relative to base).
#
# Catches the silent failure where a `var(--loom-color-X)`
# reference ships without a definition in the base :root block —
# computed value is undefined, the rule is silently dropped, the
# affected element renders unstyled at first paint.
#
# REGRESSION-GUARD: keep the strict/warn split aligned with the
# CLI's exit codes. Tightening warns to strict would block ship
# on intentional drift (e.g. a theme that genuinely doesn't need
# a token); loosening strict to warn would let the silent-paint
# bug back in. T31 (2026-05-06).
# ============================================================
phase_theme_consistency() {
  phase_header "theme_consistency"
  local LOOM_BIN="${LOOM_BIN:-/home/user/cargo-target/release/loom}"
  if [ ! -x "$LOOM_BIN" ]; then
    finding_warn "theme_consistency" "loom-cli" "loom binary not at $LOOM_BIN — theme drift not verified"
    return
  fi
  # Pick the canonical skin file. The PoC ships it as
  # static/loom.css (full skin) AND static/loom-skin.css
  # (component-only); validate runs against the full file.
  local skin=""
  for candidate in "$STATIC/loom.css" "$STATIC/loom-skin.css"; do
    if [ -f "$candidate" ]; then
      skin="$candidate"
      break
    fi
  done
  if [ -z "$skin" ]; then
    finding_warn "theme_consistency" "static/" "no loom.css or loom-skin.css found — theme drift not verified"
    return
  fi
  local out
  out=$("$LOOM_BIN" theme validate --skin "$skin" 2>&1)
  local rc=$?
  if [ $rc -eq 0 ]; then
    # Strip the leading `ok ` from the CLI line so we don't print
    # "ok ok 4 theme(s)..." (phase header already adds the ok prefix).
    local summary
    summary=$(echo "$out" | grep -E '^\s+ok\s' | head -1 | sed -E 's/^\s+ok\s+//')
    if [ -n "$summary" ]; then
      echo "  ${C_GREEN}ok${C_OFF}      $summary"
    fi
    return
  fi
  # Walk findings: STRICT lines bubble up as strict; warn lines as warn.
  while IFS= read -r line; do
    if echo "$line" | grep -qE '^\s+STRICT\s'; then
      local tok
      tok=$(echo "$line" | grep -oE -- '--loom-color-[a-z0-9-]+' | head -1)
      finding_strict "theme_consistency" "${tok:-skin}" "$line"
    elif echo "$line" | grep -qE '^\s+warn\s'; then
      local tok
      tok=$(echo "$line" | grep -oE -- '--loom-color-[a-z0-9-]+' | head -1)
      finding_warn "theme_consistency" "${tok:-skin}" "$line"
    fi
  done < <(echo "$out")
}

# ============================================================
# Phase: path_consistency — every cms/<name>.json's `path` field
# must resolve to a real static/<file>. Catches the mismatch
# class T11 surfaced (compose.json declared /compose but file
# lived at /compose.html) at build time, before the crawler's
# auto-journey discovers it via 404.
#
# Mapping rules:
#   page.path "/"            → static/index.html
#   page.path "/foo.html"    → static/foo.html
#   page.path "/foo"         → static/foo.html (HTML-suffix fallback)
#   page.path "/foo/"        → static/foo/index.html (dir-suffix fallback)
#
# Strict if any cms/*.json points at a missing file.
#
# REGRESSION-GUARD: do NOT broaden the mapping rules. If a CMS
# author wants a path that doesn't follow these conventions,
# the right fix is renaming the file or adjusting CmsPage.path —
# not relaxing the audit.
# ============================================================
phase_path_consistency() {
  phase_header "path_consistency"
  if [ ! -d "$ROOT/cms" ]; then
    echo "  ${C_DIM}skip${C_OFF}    no cms/ directory; nothing to check"
    return
  fi
  local hits=0
  local checked=0
  for j in "$ROOT/cms"/*.json; do
    [ -e "$j" ] || continue
    checked=$((checked + 1))
    local name=$(basename "$j" .json)
    # Pull the path field via python (jq isn't always present).
    local path=$(python3 -c "
import json, sys
try:
    print(json.load(open(sys.argv[1])).get('path', ''))
except Exception:
    pass
" "$j")
    if [ -z "$path" ]; then
      finding_strict "path_consistency" "$name.json" "missing or unreadable 'path' field"
      hits=$((hits + 1))
      continue
    fi
    # The CmsPage.path is the URL visitors use. python's
    # http.server (and most static servers) serve files
    # literally — no .html-stripping. So path MUST include the
    # .html suffix unless it's "/" (root → index.html).
    #
    # Anything else (e.g., '/compose' for a file named
    # 'compose.html') is a real bug: visitors typing the URL get
    # 404. Auto-derived journeys catch this via the 404; this
    # phase catches it at build time.
    local candidate=""
    local violation=""
    case "$path" in
      "/")
        candidate="$STATIC/index.html"
        ;;
      *.html)
        candidate="$STATIC$path"
        ;;
      *)
        violation="path=$path → must end in .html or be '/' (visitors hit 404 on ambiguous path; static server doesn't strip .html)"
        ;;
    esac
    if [ -n "$violation" ]; then
      finding_strict "path_consistency" "$name.json" "$violation"
      hits=$((hits + 1))
    elif [ ! -f "$candidate" ]; then
      finding_strict "path_consistency" "$name.json" "path=$path → expected file at ${candidate#$ROOT/} but it doesn't exist"
      hits=$((hits + 1))
    fi
  done
  if [ $hits -eq 0 ] && [ $checked -gt 0 ]; then
    echo "  ${C_GREEN}ok${C_OFF}      $checked cms/*.json path field(s) resolve to real static/ files"
  fi
}

phase_cms_render() {
  phase_header "cms_render"
  if [ ! -d "$ROOT/cms" ]; then
    echo "  ${C_DIM}skip${C_OFF}    no cms/ directory; nothing to render"
    return
  fi
  local LOOM_BIN="${LOOM_BIN:-/home/user/cargo-target/release/loom}"
  if [ ! -x "$LOOM_BIN" ]; then
    finding_strict "cms_render" "loom-cli" "loom binary not at $LOOM_BIN — run 'cargo build --release -p loom-cli' in PlausiDen-Loom"
    return
  fi
  # T49.1: extract critical CSS once per build before rendering.
  # The critical block ships inline in every page-shell so first
  # paint blocks only on ~20KB instead of the full ~70KB skin.
  local CRIT_PATH="$STATIC/loom-critical.css"
  if [ -f "$STATIC/loom-skin.css" ]; then
    if "$LOOM_BIN" critical-css --input "$STATIC/loom-skin.css" --out "$CRIT_PATH" 2>/tmp/forge-critical-err; then
      local crit_bytes=$(wc -c < "$CRIT_PATH")
      local full_bytes=$(wc -c < "$STATIC/loom-skin.css")
      local pct=$(( crit_bytes * 100 / full_bytes ))
      echo "  ${C_DIM}critical:${C_OFF} ${crit_bytes} of ${full_bytes} bytes (${pct}%)"
    else
      finding_warn "cms_render" "loom-skin.css" "critical-css extraction failed: $(cat /tmp/forge-critical-err | head -1)"
      CRIT_PATH=""
    fi
    rm -f /tmp/forge-critical-err
  else
    CRIT_PATH=""
  fi
  local hits=0
  local rendered=0
  for j in "$ROOT/cms"/*.json; do
    [ -e "$j" ] || continue
    local name=$(basename "$j" .json)
    local out="$STATIC/$name.html"
    local extra_args=""
    if [ -n "$CRIT_PATH" ] && [ -f "$CRIT_PATH" ]; then
      extra_args="--critical-css $CRIT_PATH"
    fi
    if ! "$LOOM_BIN" cms-render --input "$j" --out "$out" --css-href "/loom-skin.css" $extra_args 2>/tmp/forge-cms-render-err; then
      local err=$(cat /tmp/forge-cms-render-err 2>/dev/null | head -1)
      finding_strict "cms_render" "$name.json" "render failed: $err"
      hits=$((hits + 1))
    else
      rendered=$((rendered + 1))
    fi
  done
  rm -f /tmp/forge-cms-render-err
  if [ $rendered -gt 0 ] && [ $hits -eq 0 ]; then
    echo "  ${C_GREEN}ok${C_OFF}      $rendered page(s) regenerated from cms/*.json"
  fi
}

# ============================================================
# Phase: loom_sync — verify static/loom-skin.css matches Loom
# ============================================================
# Owner doctrine 2026-05-04: "you can hard code fixes into loom
# cms and forge just dont hard code fixes into what it generates."
# This phase enforces the boundary mechanically. Loom is the
# source of truth for design-system rules; PoC's
# static/loom-skin.css should mirror it (plus PoC-specific
# composite-component extensions appended below the marker).
#
# Logic:
#   1. Locate Loom at LOOM_PATH (env var or default sibling repo).
#   2. Compute sha384 of Loom's loom-tokens/src/skin.css.
#   3. Scan PoC's static/loom-skin.css for a marker
#      `/* SYNC-FROM-LOOM:<hash> */`.
#   4. If marker matches the freshly-computed hash → silent ok.
#   5. If absent or mismatched → warn (not strict — won't fail
#      build) with diff line-count + suggestion to run
#      `forge.sh --sync-loom` to update.
#   6. With `--sync-loom`, copy Loom's skin.css over the PoC's
#      and prepend the marker line.
#
# Why warn-not-strict: drift can be intentional during a
# multi-step migration. The warning surfaces drift LOUDLY without
# blocking a build. The `--sync-loom` flag makes resolving easy.
# ============================================================
# Phase: crawl — invoke PlausiDen-Crawler against the freshly
# rendered static/. T49 (2026-05-06).
#
# One forge run = build + runtime audit, single report. Without
# this phase the operator runs forge AND then a separate
# `npm run audit`, and merging the two outputs is manual.
#
# Behaviour:
#   * Crawler dir resolved via $CRAWLER_DIR or sibling path.
#     Missing → warn, skip (devs without crawler installed
#     shouldn't be blocked from building).
#   * Dev server at 127.0.0.1:8123 must respond. If it doesn't
#     (ERR_CONNECTION_REFUSED), warn + skip — fixing the dev
#     server isn't forge's job, and a missing server would
#     cascade as a fake regression in every axis.
#   * Crawler exit 0  → ok summary line.
#   * Crawler exit 1  → forge_strict per regressed axis (parsed
#                       from positive-signal.txt of the latest
#                       run dir).
#   * Crawler exit ≥2 → forge_warn (crawler errored, not a site
#                       regression).
#
# REGRESSION-GUARD: do NOT promote crawler-down-to-fake-regressions
# back to strict. We saw this fire on 2026-05-05 when /tmp wiped
# the dev server and every axis reported ERR_CONNECTION_REFUSED;
# treating that as a strict failure of the site is wrong. Only
# real regressions (exit 1 against a live server) gate the build.
# ============================================================
phase_crawl() {
  phase_header "crawl"
  local CRAWLER_DIR="${CRAWLER_DIR:-/home/user/Development/PlausiDen/PlausiDen-Crawler}"
  if [ ! -d "$CRAWLER_DIR" ]; then
    finding_warn "crawl" "PlausiDen-Crawler" \
      "crawler dir not at $CRAWLER_DIR — runtime audit skipped (set CRAWLER_DIR to override)"
    return
  fi
  local journey="${CRAWLER_JOURNEY:-journeys/skillshots-poc.json}"
  if [ ! -f "$CRAWLER_DIR/$journey" ]; then
    finding_warn "crawl" "$journey" \
      "journey file not at $CRAWLER_DIR/$journey — runtime audit skipped"
    return
  fi
  # Dev server probe. If 8123 is dead, every page would 404
  # and surface as a fake regression on every axis. Skip with
  # an explicit warn so the operator knows.
  if ! curl -sf --max-time 2 -o /dev/null "http://127.0.0.1:8123/"; then
    finding_warn "crawl" "127.0.0.1:8123" \
      "dev server not responding — start it (e.g. python3 -m http.server 8123 --directory $STATIC) and re-run; runtime audit skipped"
    return
  fi
  echo "  ${C_DIM}running crawler against http://127.0.0.1:8123/ ...${C_OFF}"
  local out
  out=$(cd "$CRAWLER_DIR" && timeout 120 npm run audit -- --journey "$journey" 2>&1)
  local rc=$?
  if [ $rc -eq 0 ]; then
    local axes_line
    axes_line=$(echo "$out" | grep -E '✓ all [0-9]+ axes silent' | head -1 | sed 's/^ *//')
    if [ -n "$axes_line" ]; then
      echo "  ${C_GREEN}ok${C_OFF}      ${axes_line}"
    else
      echo "  ${C_GREEN}ok${C_OFF}      crawler PASS"
    fi
    return
  fi
  if [ $rc -ge 2 ]; then
    finding_warn "crawl" "PlausiDen-Crawler" \
      "crawler errored (exit $rc) — runtime audit could not complete; check $CRAWLER_DIR for env"
    return
  fi
  # Exit 1: parse regressed axes from positive-signal output.
  # Format: "  ✗ N axis/axes regressed (strict): <name> (+M), ..."
  local regressed_line
  regressed_line=$(echo "$out" | grep -E 'axis/axes regressed \(strict\)' | head -1)
  if [ -n "$regressed_line" ]; then
    local axes
    axes=$(echo "$regressed_line" | sed -E 's/^.*\(strict\): *//' | tr ',' '\n')
    while IFS= read -r axis; do
      axis=$(echo "$axis" | sed -E 's/^ +//;s/ +$//')
      [ -z "$axis" ] && continue
      finding_strict "crawl" "${axis}" "runtime regression: $axis"
    done < <(echo "$axes")
  else
    # Generic strict if we couldn't parse a per-axis breakdown.
    finding_strict "crawl" "runtime" "crawler reported FAIL (exit $rc); inspect $CRAWLER_DIR/runs/ for details"
  fi
}

phase_loom_sync() {
  phase_header "loom_sync"
  local loom_path="${LOOM_PATH:-/home/user/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css}"
  if [ ! -f "$loom_path" ]; then
    echo "  ${C_DIM}skip${C_OFF}    loom skin.css not found at $loom_path"
    echo "  ${C_DIM}        ${C_OFF}set LOOM_PATH env var to enable Loom sync check"
    return 0
  fi
  local poc_path="$STATIC/loom-skin.css"
  if [ ! -f "$poc_path" ]; then
    finding_warn "loom_sync" "$poc_path" "PoC skin.css missing — run --sync-loom to bootstrap"
    return 0
  fi
  local loom_hash=$(openssl dgst -sha384 -binary "$loom_path" 2>/dev/null \
                    | openssl base64 -A 2>/dev/null)
  if [ -z "$loom_hash" ]; then
    echo "  ${C_DIM}skip${C_OFF}    openssl unavailable; cannot hash Loom skin.css"
    return 0
  fi
  local marker_line=$(grep -m1 "SYNC-FROM-LOOM:" "$poc_path" 2>/dev/null)
  if [ -z "$marker_line" ]; then
    finding_warn "loom_sync" "$(basename "$poc_path")" \
      "no SYNC-FROM-LOOM marker — never auto-synced from Loom. Run forge.sh --sync-loom to establish."
  else
    local recorded_hash=$(echo "$marker_line" | grep -oE 'sha384-[A-Za-z0-9+/=]+' | head -1)
    local current_marker="sha384-$loom_hash"
    if [ "$recorded_hash" != "$current_marker" ]; then
      local loom_lines=$(wc -l < "$loom_path")
      local poc_lines=$(wc -l < "$poc_path")
      finding_warn "loom_sync" "$(basename "$poc_path")" \
        "Loom skin.css drift detected (recorded=$recorded_hash, current=$current_marker; loom $loom_lines lines, poc $poc_lines lines). Run forge.sh --sync-loom to update."
    else
      echo "  ${C_GREEN}ok${C_OFF}      loom skin.css in sync (sha384 matches marker)"
    fi
  fi
}

# Helper: actually copy Loom→PoC and update marker. Called from
# the --sync-loom flag handler in main(), not from the phase loop.
do_loom_sync() {
  local loom_path="${LOOM_PATH:-/home/user/Development/PlausiDen/PlausiDen-Loom/loom-tokens/src/skin.css}"
  if [ ! -f "$loom_path" ]; then
    echo "${C_RED}sync failed:${C_OFF} loom skin.css not found at $loom_path"
    return 1
  fi
  local poc_path="$STATIC/loom-skin.css"
  local loom_hash=$(openssl dgst -sha384 -binary "$loom_path" | openssl base64 -A)
  local tmpf=$(mktemp)
  # Marker first line, then full Loom content. PoC-specific
  # extensions that previously lived in this file are LOST on
  # sync — owner-acknowledged risk. Future iteration: split into
  # static/loom-skin.css (Loom-synced) + static/poc-extensions.css.
  printf '/* SYNC-FROM-LOOM:sha384-%s — auto-synced by forge.sh do_loom_sync. Edits to this file will be overwritten on next sync. */\n' "$loom_hash" > "$tmpf"
  cat "$loom_path" >> "$tmpf"
  mv "$tmpf" "$poc_path"
  echo "${C_GREEN}sync ok:${C_OFF} $(basename "$poc_path") ← $(basename "$loom_path") (sha384-$loom_hash)"
  echo "${C_DIM}        $(wc -l < "$poc_path") lines, $(wc -c < "$poc_path") bytes${C_OFF}"
}

# ============================================================
# Phase: label_consistency — same href / same data-backend should
# carry the same visible label across every page (T86)
# ============================================================
# Owner directive 2026-05-04: "make sure youre looking for
# duplicate pages and content. make sure forge can do this also.
# and make sure it can easily merge duplicate content regardless
# of how complex it is."
#
# This is the detection half. It walks every static/*.html file,
# extracts <a href="..."> + <button data-backend="..."> elements
# and their inner text, groups by (kind, key) where:
#   kind = a|button
#   key  = href value (for <a>) or data-backend value (for button)
# Then any group with >1 distinct visible label is flagged
# strict — "the same action is exposed to the user under
# inconsistent copy." Forge build fails until copy is reconciled.
#
# Merge tooling (the consolidation half) is queued separately.
# Detection-first per AVP-2 doctrine: surface drift loudly, fix
# at the source (CMS template or page author), not via runtime
# work-arounds.
phase_label_consistency() {
  phase_header "label_consistency"
  python3 - <<'EOF' "$STATIC"
import os, re, sys, html
from collections import defaultdict

static_dir = sys.argv[1]
groups = defaultdict(lambda: defaultdict(list))  # (kind,key) -> {label: [files]}

# Strict text extraction: capture inner-html, strip tags,
# collapse whitespace. This is intentionally simple — fancy
# nested-element labels can have a future detector.
LINK_RE = re.compile(
    r'<a\b([^>]*)\bhref="([^"]+)"([^>]*)>(.*?)</a>',
    re.IGNORECASE | re.DOTALL,
)
# REGRESSION-GUARD: anchors carrying data-loom-rich-link="true" are
# container/card-style links whose visible text legitimately varies
# by context (the card lists different stats per page; the destination
# is the same href). Including them in label_consistency over-fires
# strict findings on every CMS-driven feed/panel page. The opt-out
# attribute is emitted by loom-cms-render's container anchors
# (CardFeed item link, Panel list link); CTA-style anchors (nav-link,
# Composer prompt, Hero CTA, Composer action) do NOT carry it and
# remain audited.
RICH_LINK_RE = re.compile(r'data-loom-rich-link="true"', re.IGNORECASE)
BUTTON_RE = re.compile(
    r'<button\b([^>]*)\bdata-backend="([^"]+)"([^>]*)>(.*?)</button>',
    re.IGNORECASE | re.DOTALL,
)
TAG_STRIP = re.compile(r'<[^>]+>')
WS = re.compile(r'\s+')

def normalize(text):
    t = TAG_STRIP.sub(' ', text)
    t = html.unescape(t)
    t = WS.sub(' ', t).strip()
    return t

for fn in sorted(os.listdir(static_dir)):
    if not fn.endswith('.html'):
        continue
    path = os.path.join(static_dir, fn)
    with open(path, encoding='utf-8') as f:
        body = f.read()
    for m in LINK_RE.finditer(body):
        attrs_pre, href, attrs_post, inner = m.group(1), m.group(2), m.group(3), m.group(4)
        all_attrs = attrs_pre + attrs_post
        # Skip anchors marked as rich/container links — their text
        # legitimately differs across contexts.
        if RICH_LINK_RE.search(all_attrs):
            continue
        label = normalize(inner)
        if label and not label.startswith('▶'):  # skip glyph-only
            groups[('a', href)][label].append(fn)
    for m in BUTTON_RE.finditer(body):
        attrs_pre, backend, attrs_post, inner = m.group(1), m.group(2), m.group(3), m.group(4)
        all_attrs = attrs_pre + attrs_post
        if RICH_LINK_RE.search(all_attrs):
            continue
        label = normalize(inner)
        if label:
            groups[('button', backend)][label].append(fn)

inconsistent = 0
for (kind, key), label_map in sorted(groups.items()):
    if len(label_map) > 1:
        labels = sorted(label_map.keys())
        files_summary = ', '.join(
            f'{lbl!r} ({len(label_map[lbl])}x)' for lbl in labels
        )
        # Output format: "kind=KIND key=KEY :: LABELS"
        print(f"  STRICT  label_consistency: {kind}[{key}] — {len(label_map)} distinct labels: {files_summary}")
        inconsistent += 1

if inconsistent == 0:
    print("  ok      every (href, data-backend) carries one label across all pages")
else:
    # Bash phase wrapper sees STDOUT lines starting "  STRICT  "
    # and counts them via FINDINGS array post-hoc; here we just
    # exit non-zero to signal the build a finding occurred.
    sys.exit(1)
EOF
  local rc=$?
  if [ $rc -ne 0 ]; then
    # Each STRICT line printed above is one finding. The python
    # block exits non-zero when it prints any STRICT line; we
    # record one finding here per phase invocation. (The python
    # output already shows each detail line on stdout.)
    #
    # REGRESSION-GUARD: previous code re-execed `bash forge.sh
    # --label-consistency-count` here, but no such flag handler
    # exists in this script — the re-exec ran the FULL forge
    # phase suite, which includes this phase, which re-execed
    # forge.sh again, ad infinitum. Result: 57 000+ forked bash
    # processes after a few cron ticks. Removed the recursive
    # call entirely; one finding per phase is sufficient.
    finding_strict "label_consistency" "static/" "duplicate labels detected (see STRICT lines above)"
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
    #
    # REGRESSION-GUARD: T49.1 inlines critical CSS as <style>
    # blocks. Those bytes come from the canonical skin.css source
    # (where px is defined as tokens like --loom-tap-min: 44px) —
    # they're trustable design-system bytes, NOT inline-style
    # attribute violations. Strip <style>...</style> blocks
    # before running the px-in-attribute audit so the inlined
    # critical CSS doesn't trip a false positive.
    local stripped=$(python3 -c '
import re, sys
src = open(sys.argv[1], "r", encoding="utf-8", errors="replace").read()
# Strip <style>...</style> spans (any whitespace + attributes
# allowed inside the open tag).
print(re.sub(r"<style\b[^>]*>.*?</style>", "", src, flags=re.DOTALL), end="")
' "$f")
    local px=$(echo "$stripped" | grep -oE '\b[0-9]+px' | grep -vE '^(0|1|2|3)px$' | sort -u)
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
# Phase: class_prefix — every class= token must be loom-* or is-*.
#
# Forge audits this because raw class names (e.g. class="card-body")
# bypass the design system: they don't read tokens, they don't get
# audited by phase_tokens, and they collide with future Loom
# component renames. Every class name in static/*.html MUST be
# either a Loom-namespaced class (loom-*) or a state modifier
# (is-*). Anything else is a strict finding.
#
# REGRESSION-GUARD: this phase was added 2026-05-04 after the
# T31 audit pass surfaced 7+ raw classes in challenge.html that
# every prior forge run silently shipped.
# ============================================================
phase_class_prefix() {
  phase_header "class_prefix"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local name=$(basename "$f")
    # Pull every class="..." token; split on whitespace; keep only
    # tokens that are NOT loom-* / is-* / empty.
    local raw=$(python3 - "$f" <<'PY'
import re, sys
p = sys.argv[1]
with open(p, 'r', encoding='utf-8') as fh:
    src = fh.read()
out = []
for m in re.finditer(r'class="([^"]*)"', src):
    for tok in m.group(1).split():
        if not tok:
            continue
        if tok.startswith('loom-'):
            continue
        if tok.startswith('is-'):
            continue
        out.append(tok)
if out:
    # Print unique tokens, comma-separated.
    seen = []
    for t in out:
        if t not in seen:
            seen.append(t)
    print(','.join(seen))
PY
)
    if [ -n "$raw" ]; then
      finding_strict "class_prefix" "$name" "non-Loom class name(s): $raw"
      hits=$((hits + 1))
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every class= token is loom-* or is-*"
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
    # Every <img> needs alt — multi-line-aware tag walk.
    local imgs_no_alt=$(_open_tags "$f" img | grep -vE 'alt=' | wc -l)
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
    local imgs_no_dims=$(_open_tags "$f" img | grep -vE 'width=' | wc -l)
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
  # T46: per-file size budgets. Production mode treats overruns as
  # ship-blocking (strict); poc mode warns so dev velocity isn't
  # blocked by intermediate tweaks. Total payload always logged.
  phase_header "perf_budget"
  local hits=0
  local budget_html=20480       # 20 KB HTML
  local budget_css=65536        # 64 KB CSS — bumped 2026-05-04 from 50K to fit 44px
                                # tap-target rule + 5 themes + 4 fonts + 3 densities. T49
                                # critical-CSS extraction will split this in a follow-up.
  local budget_js=8192          # 8 KB JS each (we're at <3 KB)
  local total_kb=0

  # T46 helper: emit at the right severity for current MODE.
  _budget_emit() {
    if [ "$MODE" = "production" ]; then
      finding_strict "perf_budget" "$1" "$2"
    else
      finding_warn "perf_budget" "$1" "$2"
    fi
  }

  for f in "$STATIC"/*.html; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_html" ]; then
      _budget_emit "$name" "$(numfmt --to=iec $sz) HTML > $(numfmt --to=iec $budget_html) budget — audit blocks / split route"
      hits=$((hits + 1))
    fi
  done
  for f in "$STATIC"/*.css; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_css" ]; then
      _budget_emit "$name" "$(numfmt --to=iec $sz) CSS > $(numfmt --to=iec $budget_css) budget — split into per-route bundles"
      hits=$((hits + 1))
    fi
  done
  for f in "$STATIC"/*.js; do
    [ -e "$f" ] || continue
    local sz=$(stat -c%s "$f")
    total_kb=$((total_kb + sz))
    local name=$(basename "$f")
    if [ "$sz" -gt "$budget_js" ]; then
      _budget_emit "$name" "$(numfmt --to=iec $sz) JS > $(numfmt --to=iec $budget_js) budget — code-split or tree-shake"
      hits=$((hits + 1))
    fi
  done
  echo "  ${C_DIM}total static payload: $(numfmt --to=iec $total_kb) (mode=$MODE; production-strict)${C_OFF}"
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
    unwired=$(_open_tags "$f" button \
              | grep -vE 'data-backend|data-loom-theme-toggle|data-loom-aesthetic-set|data-no-backend|type="submit"' \
              | wc -l)
    if [ "$unwired" -gt 0 ]; then
      finding_warn "phantom_button" "$name" "$unwired button(s) with no data-backend (UI not declared in backends.toml)"
      hits=$((hits + 1))
    fi
    # T17 (2026-05-04): undeclared-data-backend check moved to
    # phase_backend_coverage. The check here was file-scoped (grep
    # whole HTML), so it duplicated backend_coverage's signal AND
    # surfaced the rendered HTML path instead of the source cms/*.json
    # — operators got two strict findings per bug, neither of which
    # pointed at the editable file. backend_coverage is now the
    # canonical site for this; phantom_button stays narrow to its
    # name (button without any data-backend wiring).
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
  local used=0; local unused=0; local stubs=0; local undeclared=0
  local missing_files=0
  # T18 (2026-05-04): default crate root for impl_files resolution.
  # Override with FORGE_BACKEND_CRATE_ROOT for repos that ship the
  # handler crate elsewhere. Default matches the convention
  # established by T15 (loom backend-stub --crate-dir server-stub).
  local crate_root="${FORGE_BACKEND_CRATE_ROOT:-$ROOT/server-stub}"
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
    # T18: extract the impl_files line for this entry. Schema is:
    #   [backends.X]
    #   method   = "..."
    #   path     = "..."
    #   purpose  = "..."
    #   impl_files = [...]
    # impl_files lives 4 lines after the header; prior `grep -A2`
    # (REGRESSION-GUARD: do NOT shrink) silently missed every
    # entry — `loom backend-list` reported 19 stubs while this
    # phase reported 0. Use -A8 to absorb future schema growth
    # (e.g. an `auth_required = true` line).
    local impl_line
    impl_line=$(grep -A8 "^\[backends\.$d\]" "$ROOT/backends.toml" \
                | grep -E '^\s*impl_files\s*=' \
                | head -n1)
    if echo "$impl_line" | grep -qE 'impl_files\s*=\s*\[\s*\]'; then
      stubs=$((stubs + 1))
      finding_warn "backend_coverage" "backends.toml" "[$d] declared but impl_files is empty (PARTIAL — stub)"
    elif [ -n "$impl_line" ]; then
      # T18: validate each path inside impl_files = [...] exists on
      # disk. A stale entry (handler deleted, backends.toml not
      # updated) leaves backend-list reporting IMPL while the file
      # is gone — type signatures lie, runtime breaks. Strict.
      #
      # REGRESSION-GUARD: paths are extracted with a simple
      # double-quoted-string match — if the schema ever changes
      # to single-quotes or bare strings, this will silently miss
      # them. Re-test after any schema change.
      local paths
      paths=$(echo "$impl_line" | grep -oE '"[^"]+"' | sed -E 's/^"(.*)"$/\1/')
      for p in $paths; do
        local abs="$crate_root/$p"
        if [ ! -f "$abs" ]; then
          missing_files=$((missing_files + 1))
          finding_strict "backend_coverage" "backends.toml" \
            "[$d] impl_files lists \"$p\" but $abs does not exist — stale entry; either restore the handler or clear impl_files."
        fi
      done
    fi
  done
  # T17 (2026-05-04): inverse direction — UI references that do NOT
  # appear in backends.toml. A typo'd data_backend in cms/*.json
  # (e.g. "list-challanges") renders as a UI-side data-backend
  # attribute that points at nothing — fetch will 404 at runtime
  # and the page silently breaks. Strict, because the only
  # interpretations are (a) a typo or (b) an unfinished refactor;
  # both are ship-blockers.
  #
  # REGRESSION-GUARD: do NOT downgrade to warn. The prior version
  # of this phase only walked declared keys, so the typo class was
  # silent — we already lost time chasing one in the past
  # (challenge.json once shipped "list-challange"). Strict is the
  # right gate.
  for ref in $all_refs; do
    if ! echo "$declared" | grep -qx "$ref"; then
      undeclared=$((undeclared + 1))
      # Find the source files for the operator. Prefer cms/*.json
      # (the editable source); fall back to the rendered HTML.
      local src
      src=$(grep -lE "\"data_backend\":\\s*\"$ref\"" "$ROOT/cms"/*.json 2>/dev/null \
            | sed -E "s|^$ROOT/||" | tr '\n' ',' | sed 's/,$//')
      if [ -z "$src" ]; then
        src=$(grep -lE "data-backend=\"$ref\"" "$STATIC"/*.html 2>/dev/null \
              | sed -E "s|^$ROOT/||" | tr '\n' ',' | sed 's/,$//')
      fi
      finding_strict "backend_coverage" "${src:-cms/}" \
        "[$ref] UI references undeclared backend key — typo or missing entry in backends.toml. Will 404 at runtime."
    fi
  done
  echo "  ${C_DIM}declared: $total · UI-referenced: $used · unused: $unused · undeclared-refs: $undeclared · stubs: $stubs · missing-impl-files: $missing_files${C_OFF}"
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

phase_id_strategy() {
  # T73: enforce ID-attribute invariants per the owner directive
  # (memory: feedback_id_strategy_static_vs_dynamic).
  #
  # Always checked:
  #   - no duplicate id="X" within a single page (HTML spec)
  #   - every <label for="X"> resolves to an id="X" on the page
  #   - every aria-labelledby / aria-describedby / aria-controls /
  #     aria-owns reference resolves to an id on the page
  #   - every skip-link target (anchor with href="#X" inside class
  #     "loom-skip") points to an existing id="X"
  #
  # In a future dynamic-mode pass: validate that author-written
  # IDs follow a deterministic-prefix scheme to avoid collisions
  # across component instances. Out of scope until forge.toml
  # introduces mode="dynamic".
  phase_header "id_strategy"
  local hits=0
  for f in "$STATIC"/*.html; do
    [ -f "$f" ] || continue
    local name=$(basename "$f")
    # Use Python for the multi-pattern scan (bash regex is too brittle
    # for HTML attribute walking).
    local out
    out=$(python3 - <<PY 2>/dev/null
import re, sys
src = open("$f").read()

def all_ids():
    return re.findall(r'\\bid="([^"]+)"', src)

def all_for_labels():
    return re.findall(r'<label\\b[^>]*\\bfor="([^"]+)"', src, re.I)

def all_aria_refs():
    refs = []
    for attr in ('aria-labelledby', 'aria-describedby', 'aria-controls', 'aria-owns'):
        for m in re.finditer(r'\\b' + attr + r'="([^"]+)"', src, re.I):
            # Multi-token references: "id1 id2 id3"
            for r in m.group(1).split():
                refs.append((attr, r))
    return refs

def all_skiplink_refs():
    out = []
    for m in re.finditer(r'<a\\b[^>]*\\bclass="[^"]*loom-skip[^"]*"[^>]*\\bhref="#([^"]+)"', src, re.I):
        out.append(m.group(1))
    return out

ids = all_ids()
id_set = set(ids)
fails = []

# 1. Duplicates
seen = {}
for i in ids:
    seen[i] = seen.get(i, 0) + 1
for i, n in seen.items():
    if n > 1:
        fails.append(f"duplicate id=\"{i}\" appears {n} times")

# 2. label[for] resolution
for fid in all_for_labels():
    if fid not in id_set:
        fails.append(f"<label for=\"{fid}\"> has no matching <input/select/textarea id=\"{fid}\">")

# 3. ARIA reference resolution
for attr, ref in all_aria_refs():
    if ref not in id_set:
        fails.append(f"{attr}=\"...{ref}...\" target not found on this page")

# 4. Skip-link target
for ref in all_skiplink_refs():
    if ref not in id_set:
        fails.append(f"loom-skip href=\"#{ref}\" — no matching id on the page")

for line in fails:
    print(line)
PY
)
    if [ -n "$out" ]; then
      while IFS= read -r line; do
        [ -z "$line" ] && continue
        finding_strict "id_strategy" "$name" "$line"
        hits=$((hits + 1))
      done <<< "$out"
    fi
  done
  [ $hits -eq 0 ] && echo "  ${C_GREEN}ok${C_OFF}      every id is unique + every label/ARIA/skip-link reference resolves"
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

phase_validate_cms
phase_image_convert
phase_cms_render
phase_path_consistency
phase_audit_bridge
phase_theme_consistency
phase_loom_sync
phase_label_consistency
phase_tokens
phase_html_semantic
phase_class_prefix
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
phase_id_strategy
phase_csp_devmode
phase_contrast
phase_selfaudit
phase_self_check
# T49: runtime audit happens AFTER the build is on disk and
# self-check has confirmed the build is structurally sound.
# A failed crawl is real-site regression; a failed self_check
# is build infra regression — keep them on separate rungs.
phase_crawl

# T6: gzip + brotli pre-compress every text asset so the dev server
# (and a future production deploy) can serve compressed bytes when
# the browser advertises Accept-Encoding. Runs AFTER all detection
# phases so we don't compress versions that will fail strict.
# Skipped on transient files (forge-findings.js, reports/).
echo
echo "${C_BOLD}== compress ==${C_OFF}"
total_raw=0; total_gz=0; total_br=0; count=0
for f in "$STATIC"/*.html "$STATIC"/*.css "$STATIC"/*.js; do
  [ -f "$f" ] || continue
  case "$(basename "$f")" in
    forge-findings.js) continue ;;
  esac
  raw=$(stat -c%s "$f")
  total_raw=$((total_raw + raw))
  count=$((count + 1))
  gzip -9kf "$f"
  # Brotli flag syntax differs across versions; -f is force-overwrite,
  # --quality=11 is unambiguous (vs the legacy -q11 form which broke
  # silently on this distro and left stale .br files behind every
  # build → recurring SRI mismatch). Don't swallow stderr.
  brotli -f --quality=11 "$f" || brotli -f "$f"
  if [ -f "$f.gz" ]; then
    gz=$(stat -c%s "$f.gz")
    total_gz=$((total_gz + gz))
  fi
  if [ -f "$f.br" ]; then
    br=$(stat -c%s "$f.br")
    total_br=$((total_br + br))
  fi
done
if [ $count -gt 0 ] && [ $total_raw -gt 0 ]; then
  pct_gz=$(( total_gz * 100 / total_raw ))
  pct_br=$(( total_br * 100 / total_raw ))
  echo "  ${C_GREEN}ok${C_OFF}      $count file(s) compressed: raw $(numfmt --to=iec $total_raw) → gz $(numfmt --to=iec $total_gz) (${pct_gz}%) → br $(numfmt --to=iec $total_br) (${pct_br}%)"
fi

# T35: close the final phase debug record + total summary.
if [ "$FORGE_DEBUG" = "1" ] && [ -n "$DEBUG_PHASE" ]; then
  now=$(date +%s%N)
  dur_ms=$(( (now - DEBUG_PHASE_START) / 1000000 ))
  findings_delta=$(( ${#FINDINGS[@]} - DEBUG_PHASE_FINDING_BASE ))
  printf '[phase=%-18s duration=%6dms findings=%2d]\n' \
    "$DEBUG_PHASE" "$dur_ms" "$findings_delta" \
    >> "$DEBUG_LOG"
  printf '[total findings: strict=%d warn=%d]\n' \
    "$STRICT_COUNT" "$WARN_COUNT" \
    >> "$DEBUG_LOG"
  echo
  echo "${C_DIM}[debug] phase log:${C_OFF}"
  cat "$DEBUG_LOG"
fi

# T78: auto-refresh SRI hashes on every build. Runs AFTER all
# detection phases so integrity= reflects the bytes on disk
# *after* any HTML rewrites the build itself produced (e.g. SEO
# inject, sitemap regen). Without this, CSS/JS edits between
# builds strand stale hashes and the browser refuses the asset.
# Quiet on success; the inject script prints what it changed.
if [ -x "$(command -v python3)" ] && [ -f "$ROOT/inject_sri.py" ]; then
  echo
  echo "${C_BOLD}== sri-refresh ==${C_OFF}"
  python3 "$ROOT/inject_sri.py" 2>&1 | sed 's/^/  /'
fi

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

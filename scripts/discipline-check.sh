#!/usr/bin/env bash
#
# T92: super-society discipline gate.
#
# Per `super-society-tech-stack`, every PlausiDen-Forge crate must
# score on six axes simultaneously: fast + reliable + robust +
# secure + anonymous + private. This script asserts the
# mechanically-checkable parts of that discipline across every
# workspace crate.
#
# Exit codes:
#   0 — all checks passed (or none failed at the configured
#       severity)
#   1 — at least one mandatory-axis violation
#   2 — fatal (missing tooling, can't enumerate crates)
#
# Per-axis checks (mechanical):
#
# 1. secure
#    * #[forbid(unsafe_code)] declared at the crate root
#      (src/lib.rs or src/main.rs).
#
# 2. reliable
#    * #[deny(missing_docs)] declared at the crate root.
#    * At least one #[cfg(test)] mod or one #[test] fn in the
#      crate (otherwise the typed surface is unverified shape).
#
# 3. robust
#    * publish = false in Cargo.toml (intentional workspace-
#      internal scope; prevents accidental publish from a crate
#      not yet stable). Workspace bin / publishable crates
#      explicitly opt out by removing the line + adding an
#      ALLOW comment.
#
# 4. fast / anonymous / private — currently not mechanically
#    checked here; relies on dedicated gates (perf in CI,
#    privacy-core's `forge privacy validate`, etc.).
#
# Output: per-crate compliance matrix on stderr; aggregate
# pass/fail summary on stdout.

set -euo pipefail

# Parse args: optional --strict flag + optional workspace root.
strict=0
ROOT=""
while [ $# -gt 0 ]; do
    case "$1" in
    --strict)
        strict=1
        shift
        ;;
    -h | --help)
        cat <<'EOF'
Usage: discipline-check.sh [--strict] [WORKSPACE_ROOT]

Prints per-crate compliance matrix to stderr + aggregate JSON
verdict to stdout.

Default (advisory) mode always exits 0 unless tooling fails,
so it can run in CI as a continuously-visible scoreboard
without gating PRs. Use --strict to exit 1 on any violation
once cleanup work has reached zero.
EOF
        exit 0
        ;;
    *)
        ROOT="$1"
        shift
        ;;
    esac
done
ROOT="${ROOT:-$(pwd)}"

if [ ! -d "${ROOT}/crates" ]; then
    echo "error: ${ROOT}/crates not found" >&2
    exit 2
fi

mandatory_violations=0
total_crates=0

# Per-crate header.
printf "%-32s %-7s %-7s %-7s %-7s\n" \
    "crate" "secure" "docs" "tests" "publish" >&2
printf "%-32s %-7s %-7s %-7s %-7s\n" \
    "-----" "------" "----" "-----" "-------" >&2

for crate_dir in "${ROOT}"/crates/*/; do
    name=$(basename "${crate_dir%/}")
    total_crates=$((total_crates + 1))

    # secure: forbid(unsafe_code) in lib.rs or main.rs
    secure="ok"
    src="${crate_dir}src/lib.rs"
    [ -f "${src}" ] || src="${crate_dir}src/main.rs"
    if [ -f "${src}" ]; then
        if ! grep -qE '^#!\[forbid\(unsafe_code\)\]' "${src}"; then
            secure="MISS"
            mandatory_violations=$((mandatory_violations + 1))
        fi
    else
        secure="no-src"
    fi

    # reliable (docs): deny(missing_docs)
    docs="ok"
    if [ -f "${src}" ]; then
        if ! grep -qE '^#!\[deny\(missing_docs\)\]' "${src}"; then
            docs="MISS"
            mandatory_violations=$((mandatory_violations + 1))
        fi
    else
        docs="no-src"
    fi

    # reliable (tests): at least one test anywhere in src/ or
    # tests/. Previously only checked lib.rs/main.rs which
    # missed crates that put their tests in submodule files
    # (e.g. forge-phases has #[cfg(test)] blocks in
    # carbon_budget.rs / csp.rs / link_check.rs / etc.).
    tests="ok"
    if [ -d "${crate_dir}src" ]; then
        if ! grep -rqE '#\[cfg\(test\)\]|#\[test\]' "${crate_dir}src" \
            && [ ! -d "${crate_dir}tests" ]; then
            tests="MISS"
            mandatory_violations=$((mandatory_violations + 1))
        fi
    else
        tests="no-src"
    fi

    # robust (publish): publish = false in Cargo.toml
    publish="ok"
    cargo_toml="${crate_dir}Cargo.toml"
    if [ -f "${cargo_toml}" ]; then
        if ! grep -qE '^publish = false' "${cargo_toml}"; then
            publish="MISS"
            mandatory_violations=$((mandatory_violations + 1))
        fi
    else
        publish="no-cargo"
    fi

    printf "%-32s %-7s %-7s %-7s %-7s\n" \
        "${name}" "${secure}" "${docs}" "${tests}" "${publish}" >&2
done

# Aggregate summary on stdout (so CI can pipe + parse).
if [ "${mandatory_violations}" -eq 0 ]; then
    cat <<EOF
{
  "total_crates": ${total_crates},
  "violations": 0,
  "axes_checked": ["secure", "docs", "tests", "publish"],
  "verdict": "pass",
  "strict": ${strict}
}
EOF
    exit 0
else
    cat <<EOF
{
  "total_crates": ${total_crates},
  "violations": ${mandatory_violations},
  "axes_checked": ["secure", "docs", "tests", "publish"],
  "verdict": "fail",
  "strict": ${strict}
}
EOF
    # Advisory mode: violations surface but don't gate CI. Once
    # the cleanup task closes out forge-cli / forge-core / forge-
    # phases / forge-replay / forge-serve, flip the workflow
    # to --strict.
    if [ "${strict}" -eq 1 ]; then
        exit 1
    else
        exit 0
    fi
fi

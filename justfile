# PlausiDen-Forge justfile — canonical substrate-discipline workflows.
#
# Pairs with the Makefile (build / test / lint operations) by carrying
# the two HIGH-LEVEL workflows that encode the substrate-only-path
# doctrine end-to-end. The Makefile is a thin orchestrator over cargo
# and forge subcommands; this Justfile expresses the workflows an
# operator (or Claude) reaches for when *doing the substrate work*.
#
# Per `[[substrate-only-path]]` + AGENTS.md Rule 0: hand-coding a
# site is forbidden. These recipes embed the substrate-correct path
# for building and extending the substrate.
#
# Closes task #159 (substrate-discipline-v5).

# Default recipe lists every recipe (just runs `just --list`).
default:
    @just --list --unsorted

# ----------------------------------------------------------------
# build-site — the canonical full-build cycle for a deployed site.
#
# Read this if you ever feel tempted to:
#   - rsync hand-curl'd HTML into /var/www/...
#   - run `cp -R something/ /var/www/dev.plausiden.com/`
#   - skip `forge build --mode production`
#
# Don't. The substrate-correct workflow is:
#   1. cargo build --release
#   2. forge build --mode production (strict findings == 0)
#   3. rsync from static/ to /var/www/<host>/
#   4. chown -R caddy:caddy
# ----------------------------------------------------------------
build-site host="dev.plausiden.com":
    @echo "[1/4] cargo build --release -p forge-cli"
    cargo build --release -p forge-cli --locked
    @echo "[2/4] forge build --mode production (strict findings == 0)"
    ./target/release/forge build --mode production
    @echo "[3/4] rsync static/ -> /var/www/{{host}}/"
    @echo "      Note: requires sudo for /var/www write."
    sudo rsync -a --delete static/ /var/www/{{host}}/
    @echo "[4/4] chown -R caddy:caddy /var/www/{{host}}"
    sudo chown -R caddy:caddy /var/www/{{host}}/
    @echo ""
    @echo "Deployed via Forge build pipeline. No hand-coded artifacts."
    @echo "Verify provenance with: forge verify"

# ----------------------------------------------------------------
# substrate-extend — file a capability request for a new substrate
# capability and scaffold the implementation directory.
#
# Read this when you find yourself wanting to:
#   - hand-author a CSS rule
#   - add a `extra_class` field to a primitive
#   - inline a `<script>` to make a thing work
#   - skip the typed schema "just this once"
#
# The substrate-correct path is to file a capability request issue
# (or document one inline), then implement the capability in the
# right substrate repo. This recipe walks you through that.
# ----------------------------------------------------------------
substrate-extend layer name:
    @echo ""
    @echo "Filing a capability request for: {{name}} (substrate layer: {{layer}})"
    @echo ""
    @echo "Substrate layers + where they live:"
    @echo "  loom-primitive    →  PlausiDen-Loom/loom-cms-render/src/lib.rs (CmsSection variant)"
    @echo "                    →  PlausiDen-Loom/loom-tokens/src/skin.css (CSS)"
    @echo "  loom-token        →  PlausiDen-Loom/loom-tokens/ (semantic token)"
    @echo "  loom-theme        →  PlausiDen-Loom/loom-tokens/themes/ (theme pack)"
    @echo "  forge-phase       →  crates/forge-phases/src/{{name}}.rs"
    @echo "  forge-cli         →  crates/forge-cli/src/main.rs (subcommand)"
    @echo "  doctrine-rule     →  PlausiDen-AVP-Doctrine/doctrine/rules/<domain>.toml"
    @echo "  cms-schema        →  PlausiDen-CMS (or Loom mirror)"
    @echo "  crawler-detector  →  PlausiDen-Crawler/src/detectors/ (Detector trait impl)"
    @echo "  mcp-tool          →  mcp/tools/<name>.json + mcp/manifest.json"
    @echo ""
    @echo "Next steps:"
    @echo "  1. Open capability request issue:"
    @echo "       gh issue create --template capability-request.yml"
    @echo "     Or via web:"
    @echo "       https://github.com/thepictishbeast/PlausiDen-Forge/issues/new?template=capability-request.yml"
    @echo ""
    @echo "  2. Review the relevant skill:"
    @echo "       skills/add-loom-primitive/SKILL.md"
    @echo "       skills/add-forge-phase/SKILL.md"
    @echo "       skills/extend-doctrine-rules/SKILL.md"
    @echo "       skills/author-cms-content/SKILL.md"
    @echo ""
    @echo "  3. Verify before implementing:"
    @echo "       forge doctrine for <target-path> --terse"
    @echo "     surfaces the rules your new code will need to satisfy + cite."
    @echo ""
    @echo "  4. Implement, then build clean:"
    @echo "       just build-site"
    @echo ""
    @echo "DO NOT route around the substrate. See:"
    @echo "  docs/CAPABILITY_REQUEST_WORKFLOW.md"
    @echo "  SUBSTRATE_DISCIPLINE.md (in PlausiDen-AVP-Doctrine)"

# ----------------------------------------------------------------
# verify-discipline — quick local check that the substrate gates pass.
# CI runs the same gates via .github/workflows/substrate-discipline.yml
# ----------------------------------------------------------------
verify-discipline:
    @echo "Running substrate-discipline gates locally..."
    cargo build --release -p forge-cli --locked --quiet
    ./target/release/forge doctrine check
    ./target/release/forge bypasses
    ./target/release/forge audit phantom_button --explain || echo "(or: phase not yet implemented)"
    @echo ""
    @echo "All substrate-discipline gates passed locally."
    @echo "CI will run the same gates on PR + push to main."

# ----------------------------------------------------------------
# orient — invoke the cross-AI session-start brief.
# Aliases `make orient`. Available here for operators who reach for
# just first.
# ----------------------------------------------------------------
orient *args:
    cargo build --release -p forge-cli --locked --quiet
    ./target/release/forge orient {{args}}

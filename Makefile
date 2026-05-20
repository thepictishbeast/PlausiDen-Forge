# PlausiDen-Forge Makefile — discovery + common operations.
#
# Per AVP-Doctrine rule docs-007 (substrate updates AGENTS.md same
# commit) and `[[tool-starvation-anti-pattern]]`: `make help` is the
# cheapest, most-discoverable affordance for the tool surface. When
# Claude (or any contributor) lands in this repo and isn't sure what's
# available, `make help` prints the canonical list.
#
# This file is a thin orchestrator over `cargo` and `forge` — every
# target documents what to use directly instead, so contributors who
# want the raw tool can skip the Makefile.

.PHONY: help
help: ## Show this help.
	@printf '\n\033[1mPlausiDen-Forge — Makefile help\033[0m\n\n'
	@printf 'For the full surface see:\n'
	@printf '  AGENTS.md       — orientation for AI agents (read first)\n'
	@printf '  TOOLS.md        — canonical command index, categorized\n'
	@printf '  forge --help    — live CLI surface\n\n'
	@printf 'Common operations:\n\n'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z0-9_.-]+:.*?## / {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)
	@printf '\nDoctrine commands (subset; see TOOLS.md or `forge doctrine --help`):\n'
	@printf '  \033[36mdoctrine               \033[0m query/check/exceptions/for/render/lifecycle subcommands\n'
	@printf '\n'

# ----------------------------------------------------------------
# Build + test (delegates to cargo)
# ----------------------------------------------------------------

.PHONY: build
build: ## Build the entire workspace (debug profile).
	cargo build --workspace

.PHONY: release
release: ## Build the entire workspace, release profile.
	cargo build --workspace --release

.PHONY: test
test: ## Run every workspace test.
	cargo test --workspace

.PHONY: test-quick
test-quick: ## Run only the forge-cli + forge-core tests (fast iteration).
	cargo test -p forge-cli -p forge-core -p forge-phases

.PHONY: clippy
clippy: ## Run clippy across the workspace (lint pass).
	cargo clippy --workspace --all-targets -- -D warnings

.PHONY: fmt
fmt: ## Format the workspace (rustfmt).
	cargo fmt --all

.PHONY: fmt-check
fmt-check: ## Verify formatting without changing files (CI use).
	cargo fmt --all -- --check

# ----------------------------------------------------------------
# Forge pipeline (delegates to the release binary)
# ----------------------------------------------------------------

FORGE := ./target/release/forge

.PHONY: forge-build
forge-build: release ## Run a full Forge build of the current site (cms/*.json → static/).
	$(FORGE) build

.PHONY: forge-watch
forge-watch: release ## Continuous-build watch loop.
	$(FORGE) watch

.PHONY: forge-verify
forge-verify: release ## Verify the Merkle build-report chain.
	$(FORGE) verify

# ----------------------------------------------------------------
# Doctrine (delegates to forge subcommands)
# ----------------------------------------------------------------

.PHONY: doctrine-query
doctrine-query: release ## Query the doctrine rule database (pass FILTER=...).
	$(FORGE) doctrine query $(FILTER)

.PHONY: doctrine-check
doctrine-check: release ## Verify every rule citation in the workspace resolves.
	$(FORGE) doctrine check

.PHONY: doctrine-exceptions
doctrine-exceptions: release ## Lint inline DOCTRINE-EXCEPTION tags + register.
	$(FORGE) doctrine exceptions

.PHONY: doctrine-render
doctrine-render: release ## Render the full doctrine to docs/doctrine.md.
	@mkdir -p docs
	$(FORGE) doctrine render --out docs/doctrine.md

.PHONY: doctrine-lifecycle
doctrine-lifecycle: release ## Audit rule lifecycle (experimental / stable / deprecated).
	$(FORGE) doctrine lifecycle

# ----------------------------------------------------------------
# Substrate discipline gates
# ----------------------------------------------------------------

.PHONY: bypasses
bypasses: release ## Substrate-bypass register cross-reference.
	$(FORGE) bypasses

.PHONY: orient
orient: release ## Session-start meta-tool: affordances + Rule 0 + canonical defaults + scoped doctrine.
	$(FORGE) orient

.PHONY: mcp-list
mcp-list: ## List MCP tool definitions (cross-AI consumable JSON schemas in mcp/tools/).
	@printf '\n\033[1mPlausiDen MCP tool surface\033[0m\n\n'
	@printf 'Manifest:  mcp/manifest.json\n'
	@printf 'Tools:     mcp/tools/*.json\n'
	@printf 'README:    mcp/README.md\n\n'
	@printf 'Declared tools:\n'
	@for f in mcp/tools/*.json; do                                                  \
	    name=$$(node -e "console.log(JSON.parse(require('fs').readFileSync('$$f')).name)" 2>/dev/null); \
	    desc=$$(node -e "let d=JSON.parse(require('fs').readFileSync('$$f')).description; console.log(d.length>80?d.slice(0,77)+'...':d)" 2>/dev/null); \
	    printf '  \033[36m%-32s\033[0m %s\n' "$$name" "$$desc";                     \
	done
	@printf '\nCross-AI consumable: Claude / Gemini / other MCP clients read identical schemas.\n\n'

.PHONY: audit-secrets
audit-secrets: release ## Scan staged changes for credential leaks.
	$(FORGE) audit secrets --explain

# ----------------------------------------------------------------
# Maintenance + hygiene
# ----------------------------------------------------------------

.PHONY: clean
clean: ## Remove cargo build artifacts (target/).
	cargo clean

.PHONY: docs
docs: ## Generate workspace rustdoc (target/doc/).
	cargo doc --workspace --no-deps

# ----------------------------------------------------------------
# Composite targets — common workflows
# ----------------------------------------------------------------

.PHONY: ci
ci: fmt-check clippy test ## Run the gate set CI would run (fmt-check + clippy + test).

.PHONY: pre-commit
pre-commit: fmt clippy test-quick doctrine-check ## Quick local checks before committing.

# ----------------------------------------------------------------
# Pixel reproduction loop (Forge #218)
# ----------------------------------------------------------------
#
# Drives the dual-capture diff workflow documented in
# docs/PIXEL_REP_PROSPERITYCLUB.md. Captures the live URL + the
# local Forge mirror at 390/768/1280 px via Crawler's
# --capture-reference, lands the artifacts side-by-side under
# PlausiDen-Crawler/runs/<slug>/ and <slug>-forge/.
#
# Override variables on the command line:
#   make pixel-rep SLUG=stripe SITE_URL=https://stripe.com/
#
# CRAWLER_BIN env override points at a custom binary path (default
# resolves PlausiDen-Crawler/target/release/crawler).

PIXEL_REP_SLUG          ?= prosperityclub
PIXEL_REP_SITE_URL      ?= https://prosperityclub.com/
PIXEL_REP_PORT          ?= 8125
# Path on the local static server for the Forge mirror. Defaults to /
# (which is the prosperityclub mirror at cms/index.json). For other
# sites, override: FORGE_PATH=/stripe.html
PIXEL_REP_FORGE_PATH    ?= /
CRAWLER_REPO_DIR        ?= ../PlausiDen-Crawler
CRAWLER_BIN             ?= $(CRAWLER_REPO_DIR)/target/release/crawler

.PHONY: pixel-rep
pixel-rep: ## Capture live + Forge mirror at 3 viewports. Override SLUG / SITE_URL / PIXEL_REP_PORT.
	@test -x "$(CRAWLER_BIN)" || (echo "crawler binary not found at $(CRAWLER_BIN); run 'cd $(CRAWLER_REPO_DIR) && cargo build --release --bin crawler'" && exit 2)
	@printf '\n\033[1mpixel-rep $(PIXEL_REP_SLUG)\033[0m\n  live  = $(PIXEL_REP_SITE_URL)\n  forge = http://127.0.0.1:$(PIXEL_REP_PORT)/ (served from static/)\n\n'
	@echo "[1/3] capturing live $(PIXEL_REP_SITE_URL)..."
	@cd $(CRAWLER_REPO_DIR) && ./target/release/crawler \
	    --capture-reference $(PIXEL_REP_SITE_URL) \
	    --site-slug $(PIXEL_REP_SLUG) 2>&1 | tail -4
	@echo ""
	@echo "[2/3] starting static server on :$(PIXEL_REP_PORT)..."
	@cd static && nohup ruby -run -ehttpd . -p $(PIXEL_REP_PORT) > /tmp/forge-pixel-rep-server.log 2>&1 & echo $$! > /tmp/forge-pixel-rep.pid; sleep 2
	@test "$$(curl -sS -o /dev/null -w '%{http_code}' http://127.0.0.1:$(PIXEL_REP_PORT)/)" = "200" || (echo "static server did not come up; check /tmp/forge-pixel-rep-server.log" && kill $$(cat /tmp/forge-pixel-rep.pid 2>/dev/null) 2>/dev/null && exit 3)
	@echo "[3/3] capturing local Forge mirror at $(PIXEL_REP_FORGE_PATH)..."
	@cd $(CRAWLER_REPO_DIR) && ./target/release/crawler \
	    --capture-reference http://127.0.0.1:$(PIXEL_REP_PORT)$(PIXEL_REP_FORGE_PATH) \
	    --site-slug $(PIXEL_REP_SLUG)-forge 2>&1 | tail -4
	@kill $$(cat /tmp/forge-pixel-rep.pid 2>/dev/null) 2>/dev/null
	@rm -f /tmp/forge-pixel-rep.pid
	@printf '\n\033[1mcaptures landed:\033[0m\n'
	@printf '  live:  $(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)/\n'
	@printf '  forge: $(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-forge/\n\n'
	@ls -la $(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)/manifest.json $(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-forge/manifest.json 2>&1 | sed 's|^|  |'

.PHONY: pixel-rep-diff
pixel-rep-diff: ## Print a compact diff of two manifests written by pixel-rep. Requires SLUG.
	@printf '\n\033[1mpixel-rep diff $(PIXEL_REP_SLUG)\033[0m\n\n'
	@for vp in 390 768 1280; do \
	    live_png=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)/$$vp.png; \
	    forge_png=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-forge/$$vp.png; \
	    live_html=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)/$$vp.html; \
	    forge_html=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-forge/$$vp.html; \
	    if [ -f $$live_png ] && [ -f $$forge_png ]; then \
	        live_size=$$(stat -c%s $$live_png); \
	        forge_size=$$(stat -c%s $$forge_png); \
	        live_html_size=$$(stat -c%s $$live_html); \
	        forge_html_size=$$(stat -c%s $$forge_html); \
	        printf '  %s px  png live=%dB forge=%dB  |  html live=%dB forge=%dB\n' \
	            $$vp $$live_size $$forge_size $$live_html_size $$forge_html_size; \
	    else \
	        printf '  %s px  ⚠ missing captures (run make pixel-rep first)\n' $$vp; \
	    fi; \
	done
	@printf '\n'

PIXEL_REP_DIFF_OUT      ?= $(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-diff
PIXEL_REP_FUZZ          ?= 5%

.PHONY: pixel-rep-visual-diff
pixel-rep-visual-diff: ## Visual pixel-diff via ImageMagick. Emits diff PNGs + AE counts.
	@command -v magick >/dev/null 2>&1 || (echo "ImageMagick not installed (need 'magick' on PATH)" && exit 2)
	@mkdir -p $(PIXEL_REP_DIFF_OUT)
	@printf '\n\033[1mpixel-rep visual-diff %s\033[0m\n  fuzz: %s\n  out:  %s/\n\n' \
	    "$(PIXEL_REP_SLUG)" "$(PIXEL_REP_FUZZ)" "$(PIXEL_REP_DIFF_OUT)"
	@for vp in 390 768 1280; do \
	    live_png=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)/$$vp.png; \
	    forge_png=$(CRAWLER_REPO_DIR)/runs/$(PIXEL_REP_SLUG)-forge/$$vp.png; \
	    diff_png=$(PIXEL_REP_DIFF_OUT)/$$vp.diff.png; \
	    if [ ! -f $$live_png ] || [ ! -f $$forge_png ]; then \
	        printf '  %s px  ⚠ missing captures (run make pixel-rep first)\n' $$vp; \
	        continue; \
	    fi; \
	    live_dim=$$(magick identify -format '%wx%h' $$live_png); \
	    forge_dim=$$(magick identify -format '%wx%h' $$forge_png); \
	    forge_canvas=$(PIXEL_REP_DIFF_OUT)/$$vp.forge-canvas.png; \
	    live_target_h=$$(magick identify -format '%h' $$live_png); \
	    magick $$forge_png -gravity north -background white \
	        -extent $${vp}x$$live_target_h $$forge_canvas 2>/dev/null; \
	    ae=$$(magick compare -metric AE -fuzz $(PIXEL_REP_FUZZ) \
	          $$live_png $$forge_canvas $$diff_png 2>&1 | head -1); \
	    total=$$(awk "BEGIN { print $$vp * $$live_target_h }"); \
	    if [ -n "$$total" ] && [ "$$total" -gt 0 ]; then \
	        pct=$$(awk "BEGIN { printf \"%.1f\", $$ae * 100 / $$total }"); \
	    else \
	        pct="?"; \
	    fi; \
	    printf '  %s px  live=%s  forge=%s  →  diff=%s px (%s%% of live area)\n' \
	        $$vp $$live_dim $$forge_dim $$ae $$pct; \
	    rm -f $$forge_canvas; \
	done
	@printf '\nDiff PNGs (red overlay marks differing pixels):\n'
	@ls -la $(PIXEL_REP_DIFF_OUT)/*.diff.png 2>/dev/null | sed 's|^|  |' || echo '  (no diffs produced)'
	@printf '\n'

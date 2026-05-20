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

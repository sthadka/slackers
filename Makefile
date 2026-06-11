# ==============================================================================
#  slackers — Slack CLI
# ==============================================================================

.PHONY: build build-dev install check lint fmt fmt-check test ci clean run help

BINARY  := slackers
VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")

# Use system SQLite via pkg-config (avoids ranlib build failure on macOS)
export LIBSQLITE3_SYS_USE_PKG_CONFIG := 1

# ── Colors ────────────────────────────────────────────────────────────────────
RESET  := \033[0m
BOLD   := \033[1m
DIM    := \033[2m
GREEN  := \033[32m
YELLOW := \033[33m
CYAN   := \033[36m

.DEFAULT_GOAL := help

# ── Help ──────────────────────────────────────────────────────────────────────

help: ## Show this help
	@printf "\n  $(BOLD)slackers$(RESET) — Slack CLI\n\n"
	@printf "  $(CYAN)Usage:$(RESET) make $(DIM)<target>$(RESET)\n\n"
	@printf "  $(CYAN)Targets:$(RESET)\n"
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ \
	  { printf "    $(GREEN)%-20s$(RESET) %s\n", $$1, $$2 }' $(MAKEFILE_LIST)
	@printf "\n"

# ── Build ─────────────────────────────────────────────────────────────────────

build: ## Compile the release binary
	@printf "  $(CYAN)→$(RESET) Building $(BOLD)$(BINARY)$(RESET) $(DIM)$(VERSION)$(RESET)...\n"
	@cargo build --release && \
	  printf "  $(GREEN)✓$(RESET) $(BOLD)./target/release/$(BINARY)$(RESET) ready\n"

build-dev: ## Compile the debug binary (faster, no optimisations)
	@printf "  $(CYAN)→$(RESET) Building $(BOLD)$(BINARY)$(RESET) $(DIM)(debug)$(RESET)...\n"
	@cargo build && \
	  printf "  $(GREEN)✓$(RESET) $(BOLD)./target/debug/$(BINARY)$(RESET) ready\n"

install: ## Install to ~/.cargo/bin
	@printf "  $(CYAN)→$(RESET) Installing $(BOLD)$(BINARY)$(RESET)...\n"
	@cargo install --path . && \
	  printf "  $(GREEN)✓$(RESET) Installed to ~/.cargo/bin/$(BINARY)\n"

clean: ## Remove build artefacts
	@printf "  $(YELLOW)→$(RESET) Cleaning build artefacts\n"
	@cargo clean && \
	  printf "  $(GREEN)✓$(RESET) Clean\n"

run: build-dev ## Build (debug) then run (use ARGS="..." to pass flags)
	@./target/debug/$(BINARY) $(ARGS)

# ── Quality ───────────────────────────────────────────────────────────────────

check: ## Run cargo check + clippy
	@printf "  $(CYAN)→$(RESET) cargo check\n"
	@cargo check 2>&1 | grep -E "^error" || true
	@printf "  $(CYAN)→$(RESET) cargo clippy\n"
	@cargo clippy -- -D warnings && \
	  printf "  $(GREEN)✓$(RESET) check + clippy passed\n"

lint: ## Run clippy with all warnings as errors
	@printf "  $(CYAN)→$(RESET) cargo clippy -- -D warnings\n"
	@cargo clippy -- -D warnings

fmt: ## Format code with rustfmt
	@printf "  $(CYAN)→$(RESET) cargo fmt\n"
	@cargo fmt --all && printf "  $(GREEN)✓$(RESET) Formatted\n"

fmt-check: ## Check formatting without modifying files
	@printf "  $(CYAN)→$(RESET) cargo fmt -- --check\n"
	@cargo fmt --all -- --check && printf "  $(GREEN)✓$(RESET) Formatting OK\n"

test: ## Run all tests
	@printf "  $(CYAN)→$(RESET) cargo test\n"
	@cargo test && \
	  printf "  $(GREEN)✓$(RESET) All tests passed\n"

ci: fmt-check check test ## Run all CI checks (fmt + clippy + tests)

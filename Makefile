# Makefile for bash-ast development tasks
#
# Common targets for development, testing, and CI

.PHONY: help all build test lint clean coverage fuzz bench

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

all: lint test ## Run lint and test

build: ## Build the project
	cargo build

test: ## Run all tests
	cargo test

lint: ## Run linting (clippy + fmt check)
	cargo fmt -- --check
	cargo clippy -- -D warnings

fmt: ## Format code
	cargo fmt

clean: ## Clean build artifacts
	cargo clean
	rm -rf fuzz/artifacts fuzz/corpus/fuzz_parse/*.cur

coverage: ## Run coverage (requires cargo-llvm-cov, Linux CI)
	@echo "Running coverage analysis..."
	@echo "Note: Requires rustup-installed Rust with llvm-tools-preview"
	@echo "  rustup component add llvm-tools-preview"
	cargo llvm-cov --html --output-dir coverage -- --test-threads=1
	@echo "Coverage report: coverage/html/index.html"

coverage-text: ## Run coverage with text output
	cargo llvm-cov --text -- --test-threads=1

coverage-summary: ## Run coverage summary only
	cargo llvm-cov --summary-only -- --test-threads=1

fuzz: ## Run fuzz testing (requires nightly Rust)
	@echo "Running fuzz tests..."
	@echo "Note: Requires nightly Rust: rustup toolchain install nightly"
	@echo "Note: On first run, use 'make fuzz-setup' to patch bash object files"
	PATH="$$(rustup which --toolchain nightly rustc | xargs dirname):$$PATH" \
		cargo fuzz run fuzz_parse -- -max_total_time=60

fuzz-setup: ## Patch bash objects for fuzzing (run once after building bash)
	@echo "Patching bash object files to remove conflicting main symbols..."
	@OBJCOPY=$$(find ~/.rustup/toolchains/nightly-*/lib/rustlib/*/bin -name llvm-objcopy 2>/dev/null | head -1); \
		if [ -z "$$OBJCOPY" ]; then \
			echo "Error: llvm-objcopy not found. Run: rustup component add llvm-tools --toolchain nightly"; \
			exit 1; \
		fi; \
		for f in bash/shell.o bash/mksignames.o bash/builtins/mkbuiltins.o bash/support/man2html.o; do \
			if [ -f "$$f" ] && nm "$$f" 2>/dev/null | grep -q "T _main"; then \
				echo "  Patching $$f"; \
				$$OBJCOPY --redefine-sym _main=_bash_main_unused "$$f" "$${f}.tmp" && mv "$${f}.tmp" "$$f"; \
			fi; \
		done
	@echo "Done. Now run 'make fuzz'"

proptest: ## Run property-based tests only
	cargo test prop_

bench: ## Run benchmarks
	cargo bench
	@echo "Benchmark results: target/criterion/report/index.html"

check: ## Quick check (fast feedback loop)
	cargo check
	cargo test --lib

ci: lint test ## Full CI pipeline
	@echo "CI checks passed!"

# Local development for Partition Lab.
# Run `make` (or `make run`) to build the WebAssembly and serve the page.

WASM_BINDGEN_VERSION := 0.2.130
WASM_TARGET          := wasm32-unknown-unknown
WASM                 := target/$(WASM_TARGET)/release/partition_lab.wasm
PORT                 ?= 8000

.DEFAULT_GOAL := run
.PHONY: help setup test lint build run serve clean

help: ## Show this help
	@grep -E '^[a-z]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  %-8s %s\n", $$1, $$2}'

setup: ## Install the wasm target and the matching wasm-bindgen CLI
	rustup target add $(WASM_TARGET)
	@if wasm-bindgen --version 2>/dev/null | grep -q '$(WASM_BINDGEN_VERSION)'; then \
		echo "wasm-bindgen $(WASM_BINDGEN_VERSION) already installed"; \
	else \
		cargo install wasm-bindgen-cli --version $(WASM_BINDGEN_VERSION) --locked; \
	fi

test: ## Run the unit, scenario and property tests
	cargo test

lint: ## Run clippy the way CI does
	cargo clippy --all-targets -- -D warnings

build: ## Compile to WebAssembly and generate www/pkg
	@rustup target list --installed | grep -qx '$(WASM_TARGET)' && command -v wasm-bindgen >/dev/null \
		|| { echo "Missing the wasm target or wasm-bindgen. Run: make setup"; exit 1; }
	cargo build --release --target $(WASM_TARGET) --lib
	wasm-bindgen --target web --no-typescript --out-dir www/pkg $(WASM)

serve: ## Serve www/ on localhost (PORT=8000 by default)
	@echo "Open http://localhost:$(PORT)"
	python3 -m http.server -d www $(PORT)

run: build serve ## Build, then serve (default)

clean: ## Remove build output
	cargo clean
	rm -rf www/pkg

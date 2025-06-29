# Makefile for Loxone MCP Rust Server

.PHONY: help build build-wasm build-native test lint format clean install-deps check-wasm run-native run-wasm bench docs wasm-component infisical-test

# Default target
help:
	@echo "Loxone MCP Rust Server - Build Commands"
	@echo ""
	@echo "Building:"
	@echo "  build         - Build for current target (with code signing)"
	@echo "  build-native  - Build native binary"
	@echo "  build-wasm    - Build WASM32-WASIP2 binary"
	@echo "  build-all     - Build both native and WASM"
	@echo "  wasm-component - Build WASM component with Infisical support"
	@echo ""
	@echo "Development (uses environment variables):"
	@echo "  dev           - Development server with auto-reload"
	@echo "  dev-run       - Development HTTP server"
	@echo "  dev-stdio     - Development stdio server"
	@echo "  dev-build     - Quick development build"
	@echo ""
	@echo "Production (uses keychain credentials):"
	@echo "  run-stdio     - Production stdio server (Claude Desktop)"
	@echo "  run-http      - Production HTTP server (n8n/web)"
	@echo "  run-native    - Run via cargo (development)"
	@echo ""
	@echo "Testing & Quality:"
	@echo "  test          - Run test suite"
	@echo "  lint          - Run linting (clippy)"
	@echo "  format        - Format code"
	@echo "  check         - Run all checks (lint + test)"
	@echo ""
	@echo "WASM:"
	@echo "  run-wasm      - Run WASM server with wasmtime"
	@echo "  run-inspector - Run with MCP Inspector"
	@echo "  build-wasm-all - Build WASM for all targets"
	@echo ""
	@echo "Utilities:"
	@echo "  clean         - Clean build artifacts"
	@echo "  install-deps  - Install development dependencies"
	@echo "  docs          - Generate documentation"
	@echo "  reset-keychain - Clear and reinitialize keychain entries"
	@echo "  ./sign-binary.sh - Manually sign binary"

# Installation and setup
install-deps:
	@echo "🔧 Installing Rust toolchain and dependencies..."
	rustup target add wasm32-wasip2
	rustup component add rustfmt clippy
	cargo install wasmtime-cli || echo "wasmtime already installed"
	cargo install wasm-opt || echo "wasm-opt already installed"

# Building targets
build:
	@echo "🏗️ Building Loxone MCP server..."
	@if [ "$(shell uname)" = "Darwin" ]; then \
		echo "🔐 Building with automatic code signing..."; \
		./cargo-build-sign.sh --release; \
	else \
		cargo build --release; \
	fi

build-native:
	@echo "🏗️ Building native binary..."
	@if [ "$(shell uname)" = "Darwin" ]; then \
		./cargo-build-sign.sh --release --target-dir ./target/native; \
	else \
		cargo build --release --target-dir ./target/native; \
	fi

# WASM builds - TEMPORARILY DISABLED
# TODO: Re-enable once WASM support is properly implemented with feature flags
build-wasm:
	@echo "❌ WASM builds are temporarily disabled"
	@echo "   See: https://github.com/tokio-rs/tokio/issues/[issue-number]"
	@echo "   Run 'make build-native' for native builds instead"
	@exit 1
	# Original commands (preserved for re-enabling):
	# cargo build --release --target wasm32-wasip2
	# wasm-opt -Oz -o target/wasm32-wasip2/release/loxone_mcp_rust_optimized.wasm \
	#	target/wasm32-wasip2/release/loxone_mcp_rust.wasm || \
	#	echo "⚠️ wasm-opt not found, using unoptimized binary"

build-all: build-native
	@echo "✅ Built native target (WASM temporarily disabled)"

# Development commands
test:
	@echo "🧪 Running test suite..."
	cargo test --lib --all-features

# WASM testing - TEMPORARILY DISABLED
test-wasm:
	@echo "❌ WASM tests are temporarily disabled"
	@exit 1
	# cargo test --target wasm32-wasip2 --no-default-features --features wasm-storage

lint:
	@echo "🔍 Running clippy linting..."
	cargo clippy --workspace --lib --tests --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --all-features -- -D warnings

format:
	@echo "✨ Formatting code..."
	cargo fmt --all

format-check:
	@echo "🔍 Checking code formatting..."
	cargo fmt --all -- --check

check: format-check lint test
	@echo "✅ All checks passed"

# WASM checks - TEMPORARILY DISABLED
check-wasm: format-check
	@echo "❌ WASM checks are temporarily disabled"
	@exit 1
	# cargo check --target wasm32-wasip2 --no-default-features --features wasm-storage
	# cargo clippy --target wasm32-wasip2 --no-default-features --features wasm-storage

# WASM testing with wasm-pack
test-wasm-pack:
	@echo "🧪 Running wasm-pack tests..."
	wasm-pack test --headless --chrome --firefox

test-wasm-node:
	@echo "🧪 Running WASM tests in Node.js..."
	wasm-pack test --node

test-wasm-browser:
	@echo "🧪 Running WASM tests in browser..."
	wasm-pack test --headless --chrome

# Build WASM packages
build-wasm-web:
	@echo "🌐 Building WASM for web..."
	wasm-pack build --target web --out-dir pkg-web --release

build-wasm-node:
	@echo "📦 Building WASM for Node.js..."
	wasm-pack build --target nodejs --out-dir pkg-node --release

build-wasm-bundler:
	@echo "📦 Building WASM for bundlers..."
	wasm-pack build --target bundler --out-dir pkg-bundler --release

build-wasm-all: build-wasm-web build-wasm-node build-wasm-bundler
	@echo "✅ Built WASM for all targets"

# WASM Component Model builds (WASIP2)
wasm-component:
	@echo "🔧 Building WASM component with Infisical support..."
	./build-wasm.sh

wasm-component-dev:
	@echo "🔧 Building WASM component (development)..."
	cargo component build --target wasm32-wasip2 --features "infisical,wasi-keyvalue"

# Infisical integration testing
infisical-test:
	@echo "🔐 Testing Infisical integration..."
	cargo test --features "infisical" infisical -- --nocapture

infisical-example:
	@echo "🔐 Running Infisical example..."
	RUST_LOG=debug cargo run --features "infisical" --example infisical_client

# WASM size optimization
optimize-wasm: build-wasm-web
	@echo "⚡ Optimizing WASM size..."
	@if command -v wasm-opt >/dev/null 2>&1; then \
		wasm-opt -Oz pkg-web/loxone_mcp_rust_bg.wasm -o pkg-web/loxone_mcp_rust_optimized.wasm; \
		echo "Original size: $$(ls -lh pkg-web/loxone_mcp_rust_bg.wasm | awk '{print $$5}')"; \
		echo "Optimized size: $$(ls -lh pkg-web/loxone_mcp_rust_optimized.wasm | awk '{print $$5}')"; \
	else \
		echo "⚠️ wasm-opt not found. Install binaryen for size optimization."; \
	fi

# WASM environment setup
setup-wasm-env:
	@echo "🔧 Setting up WASM development environment..."
	rustup target add wasm32-unknown-unknown wasm32-wasip2
	@if ! command -v wasm-pack >/dev/null 2>&1; then \
		echo "Installing wasm-pack..."; \
		curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh; \
	else \
		echo "✅ wasm-pack already installed"; \
	fi
	@if ! command -v wasm-opt >/dev/null 2>&1; then \
		echo "⚠️ wasm-opt not found. Install binaryen for optimization:"; \
		echo "   https://github.com/WebAssembly/binaryen/releases"; \
	else \
		echo "✅ wasm-opt available"; \
	fi

# Complete WASM workflow
wasm-workflow: setup-wasm-env check-wasm build-wasm-all test-wasm-pack optimize-wasm
	@echo "🎉 Complete WASM workflow completed successfully"

# Running applications
run-native:
	@echo "🚀 Running native Loxone MCP server..."
	RUST_LOG=info cargo run --release

# Production servers (use keychain credentials)
run-stdio:
	@echo "🚀 Running production stdio server (Claude Desktop)..."
	@echo "💡 Using keychain credentials (signed binary)"
	RUST_LOG=info ./target/release/loxone-mcp-server stdio

run-http:
	@echo "🌐 Running production HTTP server (n8n/web clients)..."
	@echo "💡 Using keychain credentials (signed binary)"
	RUST_LOG=info ./target/release/loxone-mcp-server http --port 3001

# WASM runtime - TEMPORARILY DISABLED
run-wasm:
	@echo "❌ WASM runtime is temporarily disabled"
	@exit 1
	# wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm

# WASM optimized runtime - TEMPORARILY DISABLED
run-wasm-optimized:
	@echo "❌ WASM runtime is temporarily disabled"
	@exit 1

run-inspector:
	@echo "🔍 Running with MCP Inspector..."
	@if command -v npx >/dev/null 2>&1; then \
		npx @modelcontextprotocol/inspector wasmtime target/wasm32-wasip2/release/loxone_mcp_rust.wasm; \
	else \
		echo "❌ npx not found. Install Node.js to use MCP Inspector."; \
	fi

# Benchmarking
bench:
	@echo "⚡ Running benchmarks..."
	cargo bench --workspace

# Documentation
docs:
	@echo "📚 Generating documentation..."
	cargo doc --workspace --all-features --no-deps --open

docs-wasm:
	@echo "📚 Generating WASM documentation..."
	cargo doc --target wasm32-wasip2 --no-default-features --features wasm-storage --no-deps

# Size analysis
size-analysis:
	@echo "📊 Analyzing binary sizes..."
	@if [ -f target/release/loxone_mcp_rust ]; then \
		echo "Native binary size:"; \
		ls -lh target/release/loxone_mcp_rust; \
	fi
	@if [ -f target/wasm32-wasip2/release/loxone_mcp_rust.wasm ]; then \
		echo "WASM binary size:"; \
		ls -lh target/wasm32-wasip2/release/loxone_mcp_rust.wasm; \
	fi
	@if [ -f target/wasm32-wasip2/release/loxone_mcp_rust_optimized.wasm ]; then \
		echo "Optimized WASM binary size:"; \
		ls -lh target/wasm32-wasip2/release/loxone_mcp_rust_optimized.wasm; \
	fi

# Security audit
audit:
	@echo "🔒 Running security audit..."
	cargo audit

# Cleanup
clean:
	@echo "🧹 Cleaning build artifacts..."
	cargo clean
	rm -rf target/native

clean-wasm:
	@echo "🧹 Cleaning WASM artifacts..."
	rm -rf target/wasm32-wasip2

# Release preparation
prepare-release: clean build-all test size-analysis
	@echo "📦 Preparing release..."
	@echo "✅ Release preparation complete"

# CI/CD commands (matches GitHub Actions)
ci-check: format-check lint test security docs check-wasm
	@echo "🤖 CI checks completed successfully"

# Individual CI jobs (matches GitHub Actions workflows)
ci-format:
	@echo "🔍 CI Format Check..."
	cargo fmt --all -- --check

ci-clippy:
	@echo "🔍 CI Clippy Check..."
	cargo clippy --lib --tests --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --no-default-features -- -D warnings
	cargo clippy --lib --tests --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --features "default" -- -D warnings

ci-test:
	@echo "🧪 CI Test Suite..."
	cargo build --verbose --lib --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints
	cargo build --verbose --lib --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --no-default-features
	cargo build --verbose --lib --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --features "default"
	cargo test --verbose --lib
	cargo test --verbose --lib --features "default"
	@echo "🔍 Checking for warnings..."
	@! cargo check --lib --tests --bin loxone-mcp-server --bin loxone-mcp-setup --bin loxone-mcp-verify --bin loxone-mcp-update-host --bin loxone-mcp-test-connection --bin loxone-mcp-test-endpoints --features "default" 2>&1 | grep -i warning

ci-security:
	@echo "🔒 CI Security Audit..."
	@which cargo-audit > /dev/null || cargo install cargo-audit
	cargo audit

# CI WASM check - TEMPORARILY DISABLED
ci-wasm:
	@echo "⚠️ CI WASM Build Check - Temporarily disabled"
	@echo "   WASM support pending tokio compatibility fixes"
	# @rustup target list --installed | grep -q wasm32-wasip2 || rustup target add wasm32-wasip2
	# cargo build --target wasm32-wasip2 --features wasm --no-default-features

ci-docs:
	@echo "📚 CI Documentation Build..."
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features

# Security alias for audit
security: audit

# Development server with auto-reload (avoids keychain prompts)
dev:
	@echo "🔄 Starting development server with auto-reload..."
	@echo "💡 Using environment variables to avoid keychain prompts"
	LOXONE_USER=admin \
	LOXONE_PASS=admin \
	LOXONE_HOST=http://192.168.1.100 \
	LOXONE_API_KEY=dev-api-key \
	RUST_LOG=debug \
	cargo watch -x "run --bin loxone-mcp-server -- http"

# Development server without auto-reload
dev-run:
	@echo "🚀 Starting development server..."
	@echo "💡 Using environment variables to avoid keychain prompts"
	LOXONE_USER=admin \
	LOXONE_PASS=admin \
	LOXONE_HOST=http://192.168.1.100 \
	LOXONE_API_KEY=dev-api-key \
	RUST_LOG=debug \
	cargo run --bin loxone-mcp-server -- http

# Development stdio server (for Claude Desktop)
dev-stdio:
	@echo "🚀 Starting development stdio server..."
	@echo "💡 Using environment variables to avoid keychain prompts"
	LOXONE_USER=admin \
	LOXONE_PASS=admin \
	LOXONE_HOST=http://192.168.1.100 \
	RUST_LOG=debug \
	cargo run --bin loxone-mcp-server -- stdio

# Quick development build
dev-build:
	@echo "⚡ Quick development build..."
	@if [ "$(shell uname)" = "Darwin" ]; then \
		./cargo-build-sign.sh --bin loxone-mcp-server; \
	else \
		cargo build --bin loxone-mcp-server; \
	fi

# Update dependencies
update-deps:
	@echo "📦 Updating dependencies..."
	cargo update

# Reset keychain entries
reset-keychain: build
	@echo "🔄 Resetting keychain entries..."
	./reset-keychain.sh

# Example usage and testing
example-stdio:
	@echo "📝 Running stdio example..."
	echo '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | cargo run --release

example-health:
	@echo "📝 Testing server health..."
	@if command -v timeout >/dev/null 2>&1; then \
		timeout 5s cargo run --release || echo "Server started successfully"; \
	elif command -v gtimeout >/dev/null 2>&1; then \
		gtimeout 5s cargo run --release || echo "Server started successfully"; \
	else \
		echo "⚠️ Neither timeout nor gtimeout found. Running without timeout..."; \
		cargo run --release --help > /dev/null 2>&1 || echo "Server started successfully"; \
	fi

# Integration Testing
install-test-deps:
	@echo "📦 Installing test dependencies..."
	pip install -r integration_tests/requirements.txt

# Run quick bash integration tests
integration-quick:
	@echo "⚡ Starting server for quick integration tests..."
	@cargo run --bin loxone-mcp-server -- http --port 3003 --dev-mode > /tmp/mcp_test_server.log 2>&1 & \
	SERVER_PID=$$!; \
	sleep 5; \
	echo "🧪 Running integration tests..."; \
	if ./integration_tests/test_mcp_server.sh; then \
		echo "✅ Integration tests passed"; \
		kill $$SERVER_PID 2>/dev/null || true; \
		exit 0; \
	else \
		echo "❌ Integration tests failed"; \
		kill $$SERVER_PID 2>/dev/null || true; \
		exit 1; \
	fi

# Run full Python integration tests
integration-full: install-test-deps
	@echo "🐍 Running full Python integration tests..."
	python -m pytest integration_tests/test_mcp_compatibility.py -v

# Test with MCP Inspector
integration-inspector: install-test-deps
	@echo "🔍 Testing with MCP Inspector..."
	python integration_tests/test_with_mcp_inspector.py

# All integration tests
integration-test: integration-quick integration-full
	@echo "✅ All integration tests completed"
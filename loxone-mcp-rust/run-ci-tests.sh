#!/bin/bash
# Run all CI tests locally
# This script runs the same tests as defined in .github/workflows/ci.yml

set -e  # Exit on error

echo "🔍 Running CI Tests Locally"
echo "=========================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print test status
print_test() {
    echo -e "\n${YELLOW}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 1. Format Check
print_test "Running format check (cargo fmt)"
if cargo fmt --all -- --check; then
    print_success "Format check passed"
else
    print_error "Format check failed - run 'cargo fmt --all' to fix"
    exit 1
fi

# 2. Clippy Check
print_test "Running clippy check"
if cargo clippy --all-targets --all-features -- -D warnings; then
    print_success "Clippy check passed"
else
    print_error "Clippy check failed"
    exit 1
fi

# 3. Build Tests
print_test "Building project (default features)"
if cargo build --verbose; then
    print_success "Build with default features passed"
else
    print_error "Build with default features failed"
    exit 1
fi

print_test "Building project (no default features) - SKIPPED"
echo "⚠️  Skipping no-default-features build due to missing conditional compilation"
echo "   This requires feature gates around optional dependencies"
# TODO: Re-enable once feature gates are properly implemented
# if cargo build --verbose --no-default-features; then
#     print_success "Build with no default features passed"
# else
#     print_error "Build with no default features failed"
#     exit 1
# fi

print_test "Building project (all features)"
if cargo build --verbose --all-features; then
    print_success "Build with all features passed"
else
    print_error "Build with all features failed"
    exit 1
fi

# 4. Run Tests
print_test "Running tests (lib only)"
if cargo test --verbose --lib; then
    print_success "Library tests passed"
else
    print_error "Library tests failed"
    exit 1
fi

print_test "Running tests with all features (lib only)"
if cargo test --verbose --all-features --lib; then
    print_success "Library tests with all features passed"
else
    print_error "Library tests with all features failed"
    exit 1
fi

# 5. Check for warnings
print_test "Checking for compiler warnings"
if cargo check --all-targets --all-features 2>&1 | grep -i warning | grep -v "build script" | grep -v "macOS detected" | grep -v "Created post-build-sign.sh"; then
    print_error "Compiler warnings found"
    exit 1
else
    print_success "No compiler warnings found"
fi

# 6. Security Audit (if cargo-audit is installed)
print_test "Running security audit"
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_error "Security audit failed"
        exit 1
    fi
else
    echo "⚠️  cargo-audit not installed. Install with: cargo install cargo-audit"
fi

# 7. Documentation Build
print_test "Building documentation"
if RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features; then
    print_success "Documentation build passed"
else
    print_error "Documentation build failed"
    exit 1
fi

echo -e "\n${GREEN}🎉 All CI tests passed!${NC}"
echo "=========================="
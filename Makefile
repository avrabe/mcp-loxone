.PHONY: help install install-dev test lint format type-check security clean build setup run

# Default target
help:
	@echo "Loxone MCP Server - Development Commands"
	@echo ""
	@echo "Setup & Installation:"
	@echo "  make install       Install production dependencies"
	@echo "  make install-dev   Install development dependencies"
	@echo "  make setup         Configure Loxone credentials"
	@echo ""
	@echo "Development:"
	@echo "  make lint          Run code linting (ruff)"
	@echo "  make format        Format code automatically"
	@echo "  make type-check    Run type checking (mypy)"
	@echo "  make test          Run tests"
	@echo "  make security      Run security checks"
	@echo ""
	@echo "Build & Run:"
	@echo "  make build         Build distribution packages"
	@echo "  make run           Run the MCP server"
	@echo "  make clean         Clean build artifacts"

# Install production dependencies
install:
	uv venv
	uv pip install -r requirements.txt

# Install development dependencies
install-dev: install
	uv pip install -e ".[dev]"
	pre-commit install

# Configure Loxone credentials
setup:
	uv run python -m loxone_mcp.secrets setup

# Run linting
lint:
	uv run ruff check src/ tests/

# Format code
format:
	uv run ruff format src/ tests/

# Run type checking
type-check:
	uv run mypy src/

# Run tests
test:
	uv run pytest tests/ -v --cov=src/loxone_mcp --cov-report=term-missing

# Run security checks
security:
	uv run bandit -r src/ -ll
	uv run pip-audit
	@echo "Checking for hardcoded secrets..."
	@! grep -rE "(password|passwd|pwd|secret|api_key|apikey|token|credential)\s*=\s*[\"'][^\"']+[\"']" src/ --include="*.py" || (echo "Potential hardcoded secrets found!" && exit 1)

# Clean build artifacts
clean:
	rm -rf build/
	rm -rf dist/
	rm -rf *.egg-info
	rm -rf .coverage
	rm -rf htmlcov/
	rm -rf .pytest_cache/
	rm -rf .mypy_cache/
	rm -rf .ruff_cache/
	find . -type d -name __pycache__ -exec rm -rf {} +
	find . -type f -name "*.pyc" -delete

# Build distribution packages
build: clean
	uv run python -m build

# Run the MCP server
run:
	uv run mcp dev src/loxone_mcp/server.py

# Run all checks (useful before committing)
check: lint type-check security test
	@echo "All checks passed!"

# Development server with auto-reload
dev:
	uv run mcp dev src/loxone_mcp/server.py --reload

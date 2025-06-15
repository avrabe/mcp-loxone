# Contributing to Loxone MCP Server

Thank you for your interest in contributing to the Loxone MCP Server! This document provides guidelines and instructions for contributing.

## Code of Conduct

By participating in this project, you agree to abide by our Code of Conduct:
- Be respectful and inclusive
- Welcome newcomers and help them get started
- Focus on constructive criticism
- Respect differing viewpoints and experiences

## Getting Started

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/avrabe/mcp-loxone.git
   cd mcp-loxone
   ```

3. Set up the development environment:
   ```bash
   # Install uv if you haven't already
   curl -LsSf https://astral.sh/uv/install.sh | sh
   
   # Create virtual environment and install dependencies
   uv venv
   source .venv/bin/activate  # On Windows: .venv\Scripts\activate
   uv pip install -r requirements.txt
   uv pip install -e ".[dev]"
   ```

4. Set up pre-commit hooks:
   ```bash
   pre-commit install
   ```

## Development Workflow

1. Create a new branch for your feature or fix:
   ```bash
   git checkout -b feature/your-feature-name
   # or
   git checkout -b fix/your-bug-fix
   ```

2. Make your changes following the coding standards

3. Run tests and linting:
   ```bash
   # Run linting
   ruff check src/
   ruff format src/
   
   # Run type checking
   mypy src/
   
   # Run tests (when available)
   pytest tests/
   ```

4. Commit your changes using conventional commits:
   ```bash
   git commit -m "feat: add new control for climate devices"
   # or
   git commit -m "fix: correct rolladen position calculation"
   ```

## Conventional Commits

We use [Conventional Commits](https://www.conventionalcommits.org/) for our commit messages:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `style:` Code style changes (formatting, etc.)
- `refactor:` Code refactoring
- `perf:` Performance improvements
- `test:` Test additions or modifications
- `chore:` Maintenance tasks
- `ci:` CI/CD changes

## Pull Request Process

1. Update documentation if needed
2. Add tests for new functionality
3. Ensure all tests pass and code is properly formatted
4. Update the CHANGELOG.md with your changes
5. Create a pull request with a clear description
6. Wait for code review and address feedback

## Code Standards

### Python Style
- Follow PEP 8 with a line length of 100 characters
- Use type hints where possible
- Document functions and classes with docstrings
- Keep functions focused and small

### Security
- Never commit credentials or sensitive data
- Use the keychain for credential storage
- Validate all user inputs
- Follow security best practices

### Testing
- Write tests for new functionality
- Maintain or increase code coverage
- Test with actual Loxone hardware when possible

## Project Structure

```
mcp-loxone/
├── src/
│   └── loxone_mcp/
│       ├── __init__.py
│       ├── server.py       # Main MCP server
│       ├── secrets.py      # Credential management
│       ├── loxone_client.py    # WebSocket client
│       └── loxone_http_client.py # HTTP client
├── tests/              # Test files
├── docs/               # Documentation
└── examples/           # Usage examples
```

## Adding New Features

When adding new device controls:

1. Add the control function in `server.py`
2. Use the `@mcp.tool()` decorator
3. Include proper type hints and docstring
4. Handle errors gracefully
5. Add corresponding tests

Example:
```python
@mcp.tool()
async def control_new_device(
    room: str,
    device: Optional[str] = None,
    action: str = "default"
) -> Dict[str, Any]:
    """
    Control a new type of device.
    
    Args:
        room: Room name (partial match)
        device: Specific device name (optional)
        action: Action to perform
    
    Returns:
        Result of the control operation
    """
    # Implementation here
```

## Questions?

If you have questions:
1. Check existing issues and discussions
2. Read the documentation
3. Create a new issue with the question label

Thank you for contributing!

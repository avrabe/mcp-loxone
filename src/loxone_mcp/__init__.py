"""Loxone MCP Server - Control Loxone Gen 1 systems via Model Context Protocol."""

from .server import mcp

__version__ = "0.1.0"
__all__ = ["mcp"]


def run() -> None:
    """Main entry point for loxone-mcp command with subcommands."""
    import sys

    if len(sys.argv) < 2:
        print("Usage: loxone-mcp <command>")
        print("Commands:")
        print("  setup    - Configure Loxone credentials")
        print("  verify   - Validate existing credentials")
        print("  clear    - Remove all stored credentials")
        print("  server   - Run MCP server (stdio mode)")
        print("  sse      - Run SSE server for web integrations")
        sys.exit(1)

    command = sys.argv[1]

    if command == "setup":
        from .credentials import LoxoneSecrets

        LoxoneSecrets.setup()
    elif command == "verify":
        from .credentials import LoxoneSecrets

        if LoxoneSecrets.validate():
            print("✅ All credentials are configured")
        else:
            sys.exit(1)
    elif command == "clear":
        from .credentials import LoxoneSecrets

        LoxoneSecrets.clear_all()
    elif command == "server":
        # Run stdio MCP server
        import asyncio

        asyncio.run(mcp.run_stdio_async())
    elif command == "sse":
        # Run SSE server
        from .sse_server import main

        main()
    else:
        print(f"Unknown command: {command}")
        print("Available commands: setup, verify, clear, server, sse")
        sys.exit(1)

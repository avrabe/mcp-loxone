"""Loxone MCP Server - Control Loxone Gen 1 systems via Model Context Protocol."""

from .server import mcp

__version__ = "0.1.0"
__all__ = ["mcp"]


def run() -> None:
    """Run the MCP server."""
    import uvicorn

    uvicorn.run(mcp, host="127.0.0.1", port=8000)

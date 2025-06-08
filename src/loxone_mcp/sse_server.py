#!/usr/bin/env python3
"""SSE (Server-Sent Events) server for Loxone MCP.

This module provides FastMCP-based SSE server for the Loxone MCP server.
"""

import asyncio
import logging
import os
import sys

# Set up logging
logging.basicConfig(
    level=os.getenv("LOXONE_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# SSE configuration - These are not used by FastMCP but kept for compatibility
SSE_PORT = int(os.getenv("LOXONE_SSE_PORT", "8000"))  # FastMCP default port
SSE_HOST = os.getenv("LOXONE_SSE_HOST", "127.0.0.1")  # Localhost only for security


async def run_sse_server() -> None:
    """Run the SSE server using FastMCP's built-in SSE support."""
    logger.info("Starting FastMCP SSE server...")

    # Import the FastMCP server instance from the main server module
    from loxone_mcp.server import mcp

    # Run FastMCP's built-in SSE server
    logger.info("✅ Starting FastMCP SSE server...")
    logger.info("🔌 SSE endpoint will be available at the default FastMCP port")
    logger.info("📨 Use FastMCP's standard SSE endpoints")

    await mcp.run_sse_async()


def main() -> None:
    """Main entry point."""
    from loxone_mcp.credentials import LoxoneSecrets

    # Validate credentials first
    if not LoxoneSecrets.validate():
        print("❌ Missing Loxone credentials. Run 'uvx --from . loxone-mcp setup' first.")
        sys.exit(1)

    # Run the server
    try:
        asyncio.run(run_sse_server())
    except KeyboardInterrupt:
        logger.info("Server stopped by user")
    except Exception as e:
        logger.error(f"Server error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()


#!/usr/bin/env python3
"""Quick test to start server with minimal setup time."""

import asyncio
import signal
import sys
from loxone_mcp.server import mcp

async def main():
    """Start server for quick test."""
    print("Starting MCP server for quick test...")
    
    # Set up signal handler for graceful shutdown
    def signal_handler(signum, frame):
        print("\nShutting down server...")
        sys.exit(0)
    
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    try:
        import uvicorn
        config = uvicorn.Config(
            mcp, 
            host="127.0.0.1", 
            port=8001,  # Use different port to avoid conflicts
            log_level="info"
        )
        server = uvicorn.Server(config)
        
        print("Server starting on http://127.0.0.1:8001")
        print("Press Ctrl+C to stop")
        
        await server.serve()
        
    except KeyboardInterrupt:
        print("\nServer stopped by user")
    except Exception as e:
        print(f"Server error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    asyncio.run(main())
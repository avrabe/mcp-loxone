#!/usr/bin/env python3
"""Test script to verify server startup."""

import asyncio
import logging
import sys

async def test_server_startup():
    """Test that the server can start up without errors."""
    try:
        # Import the server
        from loxone_mcp.server import mcp
        print("✅ Server imported successfully")
        
        # Test that the mcp object exists and has expected attributes
        if hasattr(mcp, '_mcp_server'):
            print("✅ MCP server object is properly initialized")
        else:
            print("❌ MCP server object missing expected attributes")
            return False
            
        print("✅ Server startup test passed")
        return True
        
    except Exception as e:
        print(f"❌ Server startup failed: {e}")
        logging.exception("Server startup error")
        return False

if __name__ == "__main__":
    # Set up logging
    logging.basicConfig(level=logging.INFO)
    
    # Run the test
    success = asyncio.run(test_server_startup())
    sys.exit(0 if success else 1)
#!/usr/bin/env python3
"""
MCP server validation script that works without requiring Loxone credentials.

This script validates that the MCP server is properly structured and can be imported
without triggering the credential validation that would fail in CI.
"""

import inspect
import sys
from typing import Any, Dict, List


def validate_mcp_server() -> None:
    """Validate MCP server implementation without requiring credentials."""
    print("Checking MCP server implementation...")
    
    # Import the server module
    try:
        from loxone_mcp import server
        print("✅ Successfully imported server module")
    except ImportError as e:
        print(f"❌ Failed to import server module: {e}")
        sys.exit(1)
    
    # Check that FastMCP instance exists
    if not hasattr(server, 'mcp'):
        print("❌ No 'mcp' FastMCP instance found")
        sys.exit(1)
    print("✅ Found FastMCP instance")
    
    # Count decorated functions by examining the source
    tools = []
    prompts = []
    resources = []
    
    # Get all functions in the server module
    for name, obj in inspect.getmembers(server):
        if inspect.isfunction(obj) and not name.startswith('_'):
            # Check function source for decorators
            try:
                source = inspect.getsource(obj)
                if '@mcp.tool(' in source or '@mcp.tool()' in source:
                    tools.append(name)
                elif '@mcp.prompt(' in source:
                    prompts.append(name)
                elif '@mcp.resource(' in source:
                    resources.append(name)
            except (OSError, TypeError):
                # Can't get source, skip
                continue
    
    print(f"Found {len(tools)} tools: {tools}")
    print(f"Found {len(prompts)} prompts: {prompts}")
    print(f"Found {len(resources)} resources: {resources}")
    
    # Validate we have the expected tools
    expected_tools = [
        'list_rooms',
        'get_room_devices', 
        'control_rolladen',
        'control_room_rolladen',
        'control_light',
        'control_room_lights',
        'get_device_status',
        'get_all_devices'
    ]
    
    for tool in expected_tools:
        if tool not in tools:
            print(f"❌ Missing expected tool: {tool}")
            sys.exit(1)
    
    print("✅ All expected tools found")
    
    # Basic validation
    if len(tools) == 0:
        print("❌ No MCP tools found!")
        sys.exit(1)
        
    if len(prompts) == 0:
        print("❌ No MCP prompts found!")
        sys.exit(1)
    
    print("✅ MCP server validation passed!")
    print(f"Server provides {len(tools)} tools, {len(prompts)} prompts, and {len(resources)} resources")


if __name__ == "__main__":
    validate_mcp_server()
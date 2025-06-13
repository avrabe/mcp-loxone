#!/usr/bin/env python3
"""
Example: Basic Loxone Control Script

This example demonstrates how to use the Loxone MCP client
directly in Python scripts (outside of Claude).
"""

import asyncio
import os
import sys
from pathlib import Path

# Add parent directory to path to import loxone_mcp
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from loxone_mcp.loxone_http_client import LoxoneHTTPClient
from loxone_mcp.secrets import LoxoneSecrets


async def main():
    """Main example function."""
    # Check credentials
    if not LoxoneSecrets.validate():
        print("❌ No credentials found. Run 'loxone-mcp setup' first.")
        return
    
    # Get credentials
    host = LoxoneSecrets.get(LoxoneSecrets.HOST_KEY)
    username = LoxoneSecrets.get(LoxoneSecrets.USER_KEY)
    password = LoxoneSecrets.get(LoxoneSecrets.PASS_KEY)
    
    print(f"🔌 Connecting to Loxone at {host}...")
    
    # Create client
    client = LoxoneHTTPClient(host, username, password)
    
    try:
        # Connect
        await client.connect()
        print("✅ Connected successfully!")
        
        # Get structure
        structure = await client.get_structure_file()
        
        # List rooms
        print("\n📍 Rooms:")
        for uuid, room in structure.get('rooms', {}).items():
            print(f"  - {room.get('name', 'Unknown')} ({uuid})")
        
        # List some controls
        print("\n🎮 Sample Controls:")
        count = 0
        for uuid, control in structure.get('controls', {}).items():
            if count >= 5:  # Just show first 5
                break
            
            room_uuid = control.get('room', '')
            room_name = structure.get('rooms', {}).get(room_uuid, {}).get('name', 'Unknown')
            
            print(f"  - {control.get('name', 'Unknown')} ({control.get('type', 'Unknown')})")
            print(f"    Room: {room_name}")
            print(f"    UUID: {uuid}")
            count += 1
        
        # Example: Turn on a specific light (you need to know its UUID)
        # Uncomment and replace with actual UUID to test:
        # light_uuid = "your-light-uuid-here"
        # print(f"\n💡 Turning on light {light_uuid}...")
        # await client.send_command(f"jdev/sps/io/{light_uuid}/On")
        # print("✅ Light turned on!")
        
    except Exception as e:
        print(f"❌ Error: {e}")
    finally:
        await client.close()
        print("\n👋 Disconnected")


if __name__ == "__main__":
    asyncio.run(main())

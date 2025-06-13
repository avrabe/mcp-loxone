#!/usr/bin/env python3
"""
Example: Room Control Scenarios

This example shows common room control scenarios like
movie mode, goodnight routine, etc.
"""

import asyncio
import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from loxone_mcp.loxone_http_client import LoxoneHTTPClient
from loxone_mcp.secrets import LoxoneSecrets


class RoomController:
    """Helper class for room control scenarios."""
    
    def __init__(self, client: LoxoneHTTPClient, structure: dict):
        self.client = client
        self.structure = structure
        self.rooms = structure.get('rooms', {})
        self.controls = structure.get('controls', {})
    
    def find_room_uuid(self, room_name: str) -> str:
        """Find room UUID by partial name match."""
        room_lower = room_name.lower()
        for uuid, room in self.rooms.items():
            if room_lower in room.get('name', '').lower():
                return uuid
        return None
    
    def get_room_controls(self, room_uuid: str, control_type: str = None) -> list:
        """Get all controls in a room, optionally filtered by type."""
        controls = []
        for uuid, control in self.controls.items():
            if control.get('room') == room_uuid:
                if control_type is None or control.get('type') == control_type:
                    controls.append((uuid, control))
        return controls
    
    async def control_room_lights(self, room_name: str, action: str):
        """Control all lights in a room."""
        room_uuid = self.find_room_uuid(room_name)
        if not room_uuid:
            print(f"❌ Room '{room_name}' not found")
            return
        
        room_full_name = self.rooms[room_uuid].get('name', room_name)
        lights = self.get_room_controls(room_uuid, 'Light')
        
        if not lights:
            print(f"No lights found in {room_full_name}")
            return
        
        print(f"\n💡 Controlling {len(lights)} lights in {room_full_name}...")
        
        for uuid, light in lights:
            try:
                command = f"jdev/sps/io/{uuid}/{action}"
                await self.client.send_command(command)
                print(f"  ✓ {light.get('name', 'Unknown')}: {action}")
            except Exception as e:
                print(f"  ✗ {light.get('name', 'Unknown')}: {e}")
    
    async def control_room_blinds(self, room_name: str, position: int):
        """Control all blinds in a room."""
        room_uuid = self.find_room_uuid(room_name)
        if not room_uuid:
            print(f"❌ Room '{room_name}' not found")
            return
        
        room_full_name = self.rooms[room_uuid].get('name', room_name)
        blinds = self.get_room_controls(room_uuid, 'Jalousie')
        
        if not blinds:
            print(f"No blinds found in {room_full_name}")
            return
        
        print(f"\n🪟 Setting {len(blinds)} blinds in {room_full_name} to {position}%...")
        
        for uuid, blind in blinds:
            try:
                if position == 0:
                    command = f"jdev/sps/io/{uuid}/FullDown"
                elif position == 100:
                    command = f"jdev/sps/io/{uuid}/FullUp"
                else:
                    command = f"jdev/sps/io/{uuid}/moveToPosition/{position}"
                
                await self.client.send_command(command)
                print(f"  ✓ {blind.get('name', 'Unknown')}: {position}%")
            except Exception as e:
                print(f"  ✗ {blind.get('name', 'Unknown')}: {e}")
    
    async def movie_mode(self, room_name: str):
        """Set up room for movie watching."""
        print(f"\n🎬 Setting up {room_name} for movie mode...")
        
        # Close blinds
        await self.control_room_blinds(room_name, 0)
        
        # Turn off lights (or dim them)
        await self.control_room_lights(room_name, "Off")
        
        print(f"\n✅ {room_name} is ready for movie watching!")
    
    async def goodnight_routine(self, room_name: str = None):
        """Goodnight routine for a room or whole house."""
        if room_name:
            print(f"\n🌙 Goodnight routine for {room_name}...")
            await self.control_room_lights(room_name, "Off")
            await self.control_room_blinds(room_name, 0)
        else:
            print("\n🌙 Goodnight routine for entire house...")
            for room_uuid, room in self.rooms.items():
                room_name = room.get('name', 'Unknown')
                print(f"\nProcessing {room_name}...")
                await self.control_room_lights(room_name, "Off")
                await self.control_room_blinds(room_name, 0)
        
        print("\n✅ Goodnight routine complete! Sleep well! 😴")


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
        # Connect and get structure
        await client.connect()
        structure = await client.get_structure_file()
        
        # Create controller
        controller = RoomController(client, structure)
        
        # List available rooms
        print("\n📍 Available rooms:")
        for uuid, room in controller.rooms.items():
            print(f"  - {room.get('name', 'Unknown')}")
        
        # Example scenarios (uncomment to test):
        
        # 1. Movie mode in living room
        # await controller.movie_mode("Living")
        
        # 2. Turn off all lights in bedroom
        # await controller.control_room_lights("Bedroom", "Off")
        
        # 3. Open all blinds in kitchen
        # await controller.control_room_blinds("Kitchen", 100)
        
        # 4. Goodnight routine for whole house
        # await controller.goodnight_routine()
        
        print("\n💡 Uncomment examples in the code to test different scenarios!")
        
    except Exception as e:
        print(f"❌ Error: {e}")
    finally:
        await client.close()


if __name__ == "__main__":
    asyncio.run(main())

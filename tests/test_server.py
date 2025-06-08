"""Basic tests for Loxone MCP Server."""

from unittest.mock import AsyncMock, patch

import pytest

from loxone_mcp.server import (
    LoxoneDevice,
    ServerContext,
    get_room_devices,
    list_rooms,
)


@pytest.fixture
def mock_context() -> ServerContext:
    """Create a mock server context for testing."""
    # Create mock Loxone client
    mock_loxone = AsyncMock()

    # Create test structure
    test_structure = {
        "rooms": {
            "uuid-room-1": {"name": "Living Room"},
            "uuid-room-2": {"name": "Bedroom"},
            "uuid-room-3": {"name": "Kitchen"},
        },
        "controls": {
            "uuid-light-1": {
                "name": "Ceiling Light",
                "type": "Light",
                "room": "uuid-room-1",
                "states": {"value": "uuid-state-1"},
            },
            "uuid-rolladen-1": {
                "name": "Window Blind",
                "type": "Jalousie",
                "room": "uuid-room-1",
                "states": {"position": "uuid-state-2"},
            },
            "uuid-light-2": {
                "name": "Bedside Lamp",
                "type": "Light",
                "room": "uuid-room-2",
                "states": {"value": "uuid-state-3"},
            },
        },
    }

    # Create devices
    devices = {
        "uuid-light-1": LoxoneDevice(
            uuid="uuid-light-1",
            name="Ceiling Light",
            type="Light",
            room="Living Room",
            room_uuid="uuid-room-1",
            states={"value": "uuid-state-1"},
        ),
        "uuid-rolladen-1": LoxoneDevice(
            uuid="uuid-rolladen-1",
            name="Window Blind",
            type="Jalousie",
            room="Living Room",
            room_uuid="uuid-room-1",
            states={"position": "uuid-state-2"},
        ),
        "uuid-light-2": LoxoneDevice(
            uuid="uuid-light-2",
            name="Bedside Lamp",
            type="Light",
            room="Bedroom",
            room_uuid="uuid-room-2",
            states={"value": "uuid-state-3"},
        ),
    }

    # Create rooms mapping
    rooms = {
        "uuid-room-1": "Living Room",
        "uuid-room-2": "Bedroom",
        "uuid-room-3": "Kitchen",
    }

    # Create context
    context = ServerContext(
        loxone=mock_loxone, structure=test_structure, devices=devices, rooms=rooms
    )

    return context


class TestRoomManagement:
    """Test room management functions."""

    @pytest.mark.asyncio
    async def test_list_rooms(
        self, mock_context: ServerContext, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Test listing all rooms."""
        # Monkeypatch the global context
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", mock_context)

        # Call function
        rooms = await list_rooms()

        # Verify results
        assert len(rooms) == 3
        assert {"uuid": "uuid-room-1", "name": "Living Room"} in rooms
        assert {"uuid": "uuid-room-2", "name": "Bedroom"} in rooms
        assert {"uuid": "uuid-room-3", "name": "Kitchen"} in rooms

    @pytest.mark.asyncio
    async def test_list_rooms_no_context(self, monkeypatch: pytest.MonkeyPatch) -> None:
        """Test listing rooms when not connected."""
        # Monkeypatch the global context to None
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", None)

        # Call function
        rooms = await list_rooms()

        # Verify error response
        assert rooms == [{"error": "Not connected to Loxone"}]

    @pytest.mark.asyncio
    async def test_get_room_devices(
        self, mock_context: ServerContext, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Test getting devices in a specific room."""
        # Monkeypatch the global context
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", mock_context)

        # Get all devices in Living Room
        devices = await get_room_devices("Living Room")

        # Verify results
        assert len(devices) == 2
        device_names = [d["name"] for d in devices]
        assert "Ceiling Light" in device_names
        assert "Window Blind" in device_names

    @pytest.mark.asyncio
    async def test_get_room_devices_with_type_filter(
        self, mock_context: ServerContext, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Test getting devices with type filter."""
        # Monkeypatch the global context
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", mock_context)

        # Get only lights in Living Room
        devices = await get_room_devices("Living Room", device_type="Light")

        # Verify results
        assert len(devices) == 1
        assert devices[0]["name"] == "Ceiling Light"
        assert devices[0]["type"] == "Light"

    @pytest.mark.asyncio
    async def test_get_room_devices_partial_match(
        self, mock_context: ServerContext, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Test room name partial matching."""
        # Monkeypatch the global context
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", mock_context)

        # Use partial room name
        devices = await get_room_devices("living")

        # Should still find Living Room devices
        assert len(devices) == 2

    @pytest.mark.asyncio
    async def test_get_room_devices_no_match(
        self, mock_context: ServerContext, monkeypatch: pytest.MonkeyPatch
    ) -> None:
        """Test when no room matches."""
        # Monkeypatch the global context
        import loxone_mcp.server

        monkeypatch.setattr(loxone_mcp.server, "_context", mock_context)

        # Non-existent room
        devices = await get_room_devices("Garage")

        # Should return empty list
        assert devices == []


class TestSecrets:
    """Test credential management."""

    def test_secrets_import(self) -> None:
        """Test that secrets module can be imported."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Verify class exists and has expected methods
        assert hasattr(LoxoneSecrets, "get")
        assert hasattr(LoxoneSecrets, "set")
        assert hasattr(LoxoneSecrets, "validate")

    @patch.dict("os.environ", {"LOXONE_HOST": "192.168.1.100"})
    def test_get_from_environment(self) -> None:
        """Test getting credentials from environment variables."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Should get from environment
        host = LoxoneSecrets.get(LoxoneSecrets.HOST_KEY)
        assert host == "192.168.1.100"


class TestMCPServer:
    """Test MCP server setup."""

    def test_mcp_import(self) -> None:
        """Test that MCP server can be imported."""
        from loxone_mcp import mcp, run

        # Verify exports
        assert mcp is not None
        assert callable(run)

    def test_server_tools_exist(self) -> None:
        """Test that server tools are defined."""
        from loxone_mcp.server import mcp

        # Check that MCP instance exists
        assert mcp is not None
        assert hasattr(mcp, "tool")
        assert hasattr(mcp, "prompt")
        assert hasattr(mcp, "resource")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])

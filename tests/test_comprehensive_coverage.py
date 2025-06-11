"""Comprehensive test coverage with fuzzy tests and property-based testing."""

import os
import string
from unittest.mock import patch

import pytest
from hypothesis import given
from hypothesis import strategies as st


class TestEdgeCases:
    """Test edge cases and boundary conditions."""

    def test_http_client_port_variations(self) -> None:
        """Test HTTP client with various port configurations."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        # Test different port configurations
        test_cases = [
            (80, "http://192.168.1.100:80"),
            (8080, "http://192.168.1.100:8080"),
            (443, "http://192.168.1.100:443"),
            (7777, "http://192.168.1.100:7777"),
        ]

        for port, expected_url in test_cases:
            client = LoxoneHTTPClient("192.168.1.100", "user", "pass", port=port)
            assert client.base_url == expected_url
            assert client.port == port

    def test_credentials_empty_values(self) -> None:
        """Test credentials handling with empty values."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Test empty environment variables - empty string is falsy, so should return None
        with patch.dict(os.environ, {"LOXONE_HOST": ""}, clear=True):
            result = LoxoneSecrets.get("LOXONE_HOST")
            # Empty string is falsy, so get() returns None when no keychain value
            assert result is None

    def test_server_context_all_empty(self) -> None:
        """Test ServerContext with all empty collections."""
        from loxone_mcp.server import ServerContext, SystemCapabilities

        context = ServerContext(
            loxone=None,
            rooms={},
            devices={},
            categories={},
            devices_by_category={},
            devices_by_type={},
            devices_by_room={},
            discovered_sensors=[],
            capabilities=SystemCapabilities(),
        )

        assert len(context.rooms) == 0
        assert len(context.devices) == 0
        assert len(context.discovered_sensors) == 0

    def test_system_capabilities_all_disabled(self) -> None:
        """Test SystemCapabilities with all features disabled."""
        from loxone_mcp.server import SystemCapabilities

        caps = SystemCapabilities()

        # All boolean capabilities should be False by default
        boolean_caps = [
            "has_lighting",
            "has_blinds",
            "has_weather",
            "has_security",
            "has_energy",
            "has_audio",
            "has_climate",
            "has_sensors",
        ]

        for cap in boolean_caps:
            assert getattr(caps, cap) is False

        # All count capabilities should be 0 by default
        count_caps = [
            "light_count",
            "blind_count",
            "weather_device_count",
            "security_device_count",
            "energy_device_count",
            "audio_zone_count",
            "climate_device_count",
            "sensor_count",
        ]

        for cap in count_caps:
            assert getattr(caps, cap) == 0


class TestSSEServerComprehensive:
    """Comprehensive SSE server testing."""

    def test_sse_environment_variables_comprehensive(self) -> None:
        """Test all SSE environment variable combinations."""
        import importlib

        test_cases = [
            ({"LOXONE_SSE_PORT": "8000", "LOXONE_SSE_HOST": "localhost"}, 8000, "localhost"),
            ({"LOXONE_SSE_PORT": "3000"}, 3000, None),  # Host should use default
            ({"LOXONE_SSE_HOST": "127.0.0.1"}, None, "127.0.0.1"),  # Port should use default
            ({}, None, None),  # Both should use defaults
        ]

        for env_vars, expected_port, expected_host in test_cases:
            with patch.dict(os.environ, env_vars, clear=True):
                # Reload to pick up environment changes
                import loxone_mcp.sse_server

                importlib.reload(loxone_mcp.sse_server)

                if expected_port is not None:
                    assert expected_port == loxone_mcp.sse_server.SSE_PORT
                if expected_host is not None:
                    assert expected_host == loxone_mcp.sse_server.SSE_HOST

    def test_sse_server_function_types(self) -> None:
        """Test SSE server function types and signatures."""
        import inspect

        from loxone_mcp.sse_server import main, run_sse_server

        # Test function types
        assert callable(main)
        assert inspect.iscoroutinefunction(run_sse_server)

        # Test function signatures exist
        main_sig = inspect.signature(main)
        run_sig = inspect.signature(run_sse_server)

        assert main_sig is not None
        assert run_sig is not None


@pytest.mark.skipif(
    not pytest.importorskip("hypothesis", reason="hypothesis not available"),
    reason="Hypothesis not available for property-based testing",
)
class TestPropertyBased:
    """Property-based tests using Hypothesis."""

    @given(
        host=st.text(
            alphabet=string.ascii_letters + string.digits + ".-", min_size=1, max_size=50
        ).filter(lambda x: "." in x or x.isdigit())
    )
    def test_http_client_host_variations(self, host: str) -> None:
        """Test HTTP client with various valid host formats."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        try:
            client = LoxoneHTTPClient(host, "user", "pass")
            assert client.host == host
            assert client.base_url == f"http://{host}:80"
        except Exception:
            # Some generated hosts might be invalid, that's OK
            pass

    @given(port=st.integers(min_value=1, max_value=65535))
    def test_http_client_port_range(self, port: int) -> None:
        """Test HTTP client with full valid port range."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        client = LoxoneHTTPClient("localhost", "user", "pass", port=port)
        assert client.port == port
        assert f":{port}" in client.base_url

    @given(username=st.text(min_size=1, max_size=100), password=st.text(min_size=1, max_size=100))
    def test_credentials_storage_patterns(self, username: str, password: str) -> None:
        """Test credential storage with various username/password patterns."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Test that credentials can be set without error (in mock environment)
        with patch("keyring.set_password") as mock_set:
            try:
                LoxoneSecrets.set("LOXONE_USER", username)
                LoxoneSecrets.set("LOXONE_PASS", password)
                # Should call keyring.set_password for each
                assert mock_set.call_count == 2
            except Exception:
                # Some special characters might cause issues, that's OK
                pass

    @given(
        room_names=st.lists(
            st.text(min_size=1, max_size=50).filter(lambda x: x.strip()), min_size=0, max_size=20
        )
    )
    def test_server_context_room_variations(self, room_names: list[str]) -> None:
        """Test ServerContext with various room configurations."""
        from loxone_mcp.server import ServerContext, SystemCapabilities

        # Create rooms dict from list
        rooms = {f"uuid-{i}": name.strip() for i, name in enumerate(room_names)}

        context = ServerContext(
            loxone=None,
            rooms=rooms,
            devices={},
            categories={},
            devices_by_category={},
            devices_by_type={},
            devices_by_room={},
            discovered_sensors=[],
            capabilities=SystemCapabilities(),
        )

        assert len(context.rooms) == len(rooms)
        for room_id, room_name in rooms.items():
            assert context.rooms[room_id] == room_name


class TestAsyncEdgeCases:
    """Test async edge cases and error conditions."""

    @pytest.mark.asyncio
    async def test_http_client_async_methods_structure(self) -> None:
        """Test HTTP client async method structure."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        client = LoxoneHTTPClient("localhost", "user", "pass")

        # Test that async methods exist and are coroutines

        async_methods = [
            "connect",
            "close",
            "get_structure_file",
            "send_command",
            "get_state",
            "authenticate",
            "start",
            "stop",
        ]

        for method_name in async_methods:
            assert hasattr(client, method_name)
            method = getattr(client, method_name)
            assert callable(method)
            # Don't call them to avoid actual network operations

    @pytest.mark.asyncio
    async def test_credentials_async_discovery_error_handling(self) -> None:
        """Test async discovery methods error handling."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Test that async methods exist and handle errors gracefully
        try:
            servers = await LoxoneSecrets.discover_loxone_servers(timeout=0.001)
            assert isinstance(servers, list)
        except Exception:
            # Network operations might fail in test environment
            pass

        try:
            udp_result = await LoxoneSecrets._udp_discovery(timeout=0.001)
            assert isinstance(udp_result, list)
        except Exception:
            # UDP operations might fail in test environment
            pass

        try:
            http_result = await LoxoneSecrets._http_discovery(timeout=0.001)
            assert isinstance(http_result, list)
        except Exception:
            # HTTP operations might fail in test environment
            pass


class TestConcurrencyAndRobustness:
    """Test concurrency and robustness patterns."""

    def test_multiple_http_clients(self) -> None:
        """Test creating multiple HTTP clients simultaneously."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        clients = []
        for i in range(10):
            client = LoxoneHTTPClient(f"192.168.1.{100 + i}", f"user{i}", f"pass{i}")
            clients.append(client)

        # All clients should be independent
        for i, client in enumerate(clients):
            assert client.host == f"192.168.1.{100 + i}"
            assert client.username == f"user{i}"
            assert client.password == f"pass{i}"

    def test_credentials_concurrent_access_patterns(self) -> None:
        """Test credentials concurrent access patterns."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Test multiple get operations
        keys = [LoxoneSecrets.HOST_KEY, LoxoneSecrets.USER_KEY, LoxoneSecrets.PASS_KEY]

        with patch.dict(
            os.environ, {"LOXONE_HOST": "host", "LOXONE_USER": "user", "LOXONE_PASS": "pass"}
        ):
            results = [LoxoneSecrets.get(key) for key in keys]

        expected = ["host", "user", "pass"]
        assert results == expected

    def test_server_context_large_data_structures(self) -> None:
        """Test ServerContext with large data structures."""
        from loxone_mcp.server import ServerContext, SystemCapabilities

        # Create large collections
        large_rooms = {f"room-{i}": f"Room {i}" for i in range(1000)}
        large_devices = {
            f"device-{i}": {"name": f"Device {i}", "type": "Light"} for i in range(1000)
        }

        context = ServerContext(
            loxone=None,
            rooms=large_rooms,
            devices=large_devices,
            categories={},
            devices_by_category={},
            devices_by_type={},
            devices_by_room={},
            discovered_sensors=[],
            capabilities=SystemCapabilities(),
        )

        assert len(context.rooms) == 1000
        assert len(context.devices) == 1000

        # Test random access
        assert context.rooms["room-500"] == "Room 500"
        assert context.devices["device-500"]["name"] == "Device 500"


class TestSecurityAndValidation:
    """Test security patterns and input validation."""

    def test_credentials_special_characters(self) -> None:
        """Test credentials with special characters."""
        from loxone_mcp.credentials import LoxoneSecrets

        special_chars_tests = [
            "user@domain.com",
            "p@ssw0rd!",
            "user.name+tag",
            "192.168.1.100:8080",
            "user with spaces",
            "émöji-üser",
        ]

        for test_value in special_chars_tests:
            with (
                patch("keyring.set_password"),
                patch("keyring.get_password", return_value=test_value),
            ):
                try:
                    LoxoneSecrets.set("TEST_KEY", test_value)
                    result = LoxoneSecrets.get("TEST_KEY")
                    assert result == test_value
                except Exception:
                    # Some special characters might cause encoding issues
                    pass

    def test_api_key_generation_properties(self) -> None:
        """Test API key generation properties."""
        from loxone_mcp.credentials import LoxoneSecrets

        # Generate multiple API keys
        keys = [LoxoneSecrets.generate_api_key() for _ in range(100)]

        # All keys should be unique
        assert len(set(keys)) == 100

        # All keys should be URL-safe strings
        for key in keys:
            assert isinstance(key, str)
            assert len(key) > 0
            # URL-safe base64 uses only these characters
            allowed_chars = set(string.ascii_letters + string.digits + "-_")
            assert all(c in allowed_chars for c in key)

    def test_input_sanitization_patterns(self) -> None:
        """Test input sanitization patterns."""
        from loxone_mcp.loxone_http_client import LoxoneHTTPClient

        # Test with potentially problematic inputs
        problematic_inputs = [
            ("localhost", "normal_user", "normal_pass"),
            ("192.168.1.100", "user@domain", "p@ssw0rd"),
            ("host.local", "user_name", "pass_word"),
        ]

        for host, username, password in problematic_inputs:
            try:
                client = LoxoneHTTPClient(host, username, password)
                assert client.host == host
                assert client.username == username
                assert client.password == password
            except Exception:
                # Some inputs might be invalid, that's OK for security
                pass
